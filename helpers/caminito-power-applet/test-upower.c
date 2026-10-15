/*
 * UPower as the applet reads it (262), against python-dbusmock's upower template on a
 * private bus that stands for the system bus: the display device on battery, charging and
 * full, with no battery, and upowerd stopping and starting again.
 *
 *   test-upower <python>
 */

#include <gio/gio.h>

#include "pw-upower.h"

#define UPOWER_NAME "org.freedesktop.UPower"
#define UPOWER_PATH "/org/freedesktop/UPower"
#define DBUSMOCK    "org.freedesktop.DBus.Mock"

static const char *python;

typedef struct
{
  GTestDBus       *bus;
  GDBusConnection *connection;
  GSubprocess     *mock;
  PwUpower        *upower;
  guint            changes;
} Fixture;

static void
on_changed (PwUpower *upower G_GNUC_UNUSED, gpointer data)
{
  Fixture *fixture = data;

  fixture->changes++;
}

static void
start_mock (Fixture *fixture)
{
  g_autoptr (GError) error = NULL;

  fixture->mock = g_subprocess_new (G_SUBPROCESS_FLAGS_STDOUT_SILENCE, &error, python, "-m",
                                    "dbusmock", "--system", "--template", "upower", NULL);
  g_assert_no_error (error);
  /* The mock owns org.freedesktop.UPower once it serves the template's objects. */
  for (int i = 0; i < 100; i++)
    {
      g_autoptr (GVariant) has = NULL;
      gboolean owned;

      has = g_dbus_connection_call_sync (fixture->connection, "org.freedesktop.DBus",
                                         "/org/freedesktop/DBus", "org.freedesktop.DBus",
                                         "NameHasOwner", g_variant_new ("(s)", UPOWER_NAME),
                                         G_VARIANT_TYPE ("(b)"), G_DBUS_CALL_FLAGS_NONE, -1,
                                         NULL, &error);
      g_assert_no_error (error);
      g_variant_get (has, "(b)", &owned);
      if (owned)
        return;
      g_usleep (100 * 1000);
    }
  g_error ("The mock never owned %s", UPOWER_NAME);
}

static void
stop_mock (Fixture *fixture)
{
  g_subprocess_force_exit (fixture->mock);
  g_subprocess_wait (fixture->mock, NULL, NULL);
  g_clear_object (&fixture->mock);
}

/* SetupDisplayDevice: the type, state, percentage, energy, energy full, energy rate, time
   to empty, time to full, is present, icon name, warning level. */
static void
setup_display (Fixture *fixture, guint state, double percentage, gint64 to_empty,
               gint64 to_full, gboolean present)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = NULL;

  reply = g_dbus_connection_call_sync (
    fixture->connection, UPOWER_NAME, UPOWER_PATH, DBUSMOCK, "SetupDisplayDevice",
    g_variant_new ("(uuddddxxbsu)", 2, state, percentage, percentage, 100.0, 10.0, to_empty,
                   to_full, present, "", 1),
    NULL, G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);
  g_assert_no_error (error);
}

typedef gboolean (*Condition) (Fixture *fixture, gconstpointer expected);

/* Runs the main loop until condition holds, or fails after 5 s. */
static void
wait_until (Fixture *fixture, Condition condition, gconstpointer expected)
{
  gint64 deadline = g_get_monotonic_time () + 5 * G_USEC_PER_SEC;

  while (!condition (fixture, expected))
    {
      g_assert_cmpint (g_get_monotonic_time (), <, deadline);
      g_main_context_iteration (NULL, FALSE);
      g_usleep (10 * 1000);
    }
}

static gboolean
tooltip_is (Fixture *fixture, gconstpointer expected)
{
  PwDisplay display;
  g_autofree char *tooltip = NULL;

  pw_upower_display (fixture->upower, &display);
  tooltip = pw_state_tooltip (&display);
  return g_strcmp0 (tooltip, expected) == 0;
}

static gboolean
has_changed (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return fixture->changes > 0;
}

static gboolean
connection_gone (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return fixture->connection == NULL;
}

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GError) error = NULL;

  fixture->bus = g_test_dbus_new (G_TEST_DBUS_NONE);
  g_test_dbus_up (fixture->bus);
  /* GDBus reads the system bus's address from here, and so does dbusmock. */
  g_setenv ("DBUS_SYSTEM_BUS_ADDRESS", g_test_dbus_get_bus_address (fixture->bus), TRUE);
  fixture->connection = g_bus_get_sync (G_BUS_TYPE_SYSTEM, NULL, &error);
  g_assert_no_error (error);
  /* The bus goes down at the end of each test, which mustn't end the test program. */
  g_dbus_connection_set_exit_on_close (fixture->connection, FALSE);
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_clear_pointer (&fixture->upower, pw_upower_free);
  if (fixture->mock != NULL)
    stop_mock (fixture);
  /* GDBus keeps the system bus's connection while anything holds it, and the next test's
     bus is a new one: wait for this one to go before taking the bus down. */
  g_object_add_weak_pointer (G_OBJECT (fixture->connection), (gpointer *) &fixture->connection);
  g_object_unref (fixture->connection);
  wait_until (fixture, connection_gone, NULL);
  g_test_dbus_down (fixture->bus);
  g_clear_object (&fixture->bus);
}

static void
test_states (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->upower = pw_upower_new (on_changed, fixture);

  /* The template's display device has no battery. */
  wait_until (fixture, tooltip_is, "Plugged in");

  setup_display (fixture, 2, 78.0, 3 * 3600 + 40 * 60, 0, TRUE);
  wait_until (fixture, tooltip_is, "78%\n3 h 40 min left");
  setup_display (fixture, 1, 42.0, 0, 50 * 60, TRUE);
  wait_until (fixture, tooltip_is, "42%\nFull in 50 min");
  setup_display (fixture, 4, 100.0, 0, 0, TRUE);
  wait_until (fixture, tooltip_is, "100%\nPlugged in");
  setup_display (fixture, 0, 0.0, 0, 0, FALSE);
  wait_until (fixture, tooltip_is, "Plugged in");
  /* The applet is told, from the main loop. */
  wait_until (fixture, has_changed, NULL);
}

static void
test_restart (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->upower = pw_upower_new (on_changed, fixture);
  setup_display (fixture, 2, 78.0, 3 * 3600 + 40 * 60, 0, TRUE);
  wait_until (fixture, tooltip_is, "78%\n3 h 40 min left");

  /* upowerd gone: no battery, as far as anyone can tell. */
  stop_mock (fixture);
  wait_until (fixture, tooltip_is, "Plugged in");

  start_mock (fixture);
  setup_display (fixture, 1, 55.0, 0, 20 * 60, TRUE);
  wait_until (fixture, tooltip_is, "55%\nFull in 20 min");
}

static void
test_late_start (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  /* The applet is loaded before upowerd runs. */
  fixture->upower = pw_upower_new (on_changed, fixture);
  wait_until (fixture, tooltip_is, "Plugged in");
  start_mock (fixture);
  setup_display (fixture, 2, 90.0, 2 * 3600, 0, TRUE);
  wait_until (fixture, tooltip_is, "90%\n2 h left");
}

static void
test_free_early (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->upower = pw_upower_new (on_changed, fixture);
  /* Freed before the proxy is up: nothing may call into it afterwards. */
  g_clear_pointer (&fixture->upower, pw_upower_free);
  /* The proxy's reply, if it came, would come within a few milliseconds on a private bus:
     half a second of the loop gives a stray callback ample time to crash the test. */
  for (int i = 0; i < 50; i++)
    {
      g_main_context_iteration (NULL, FALSE);
      g_usleep (10 * 1000);
    }
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  if (argc != 2)
    g_error ("usage: test-upower <python>");
  python = argv[1];
  g_test_add ("/upower/the-display-device-is-followed", Fixture, NULL, setup, test_states, teardown);
  g_test_add ("/upower/upowerd-stopping-and-starting-again-is-followed", Fixture, NULL, setup, test_restart, teardown);
  g_test_add ("/upower/upowerd-starting-after-the-applet-is-followed", Fixture, NULL, setup, test_late_start, teardown);
  g_test_add ("/upower/freeing-while-starting-calls-nothing", Fixture, NULL, setup, test_free_early, teardown);
  return g_test_run ();
}
