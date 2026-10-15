/*
 * UPower as the applet reads it (262), against python-dbusmock's upower template on a
 * private bus that stands for the system bus: the display device on battery, charging and
 * full, with no battery, and upowerd stopping and starting again; and (263) the other
 * devices as they come and go, and the charge limit.
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
  /* The charge limit's answers: how many, and the last one's error. */
  guint            answers;
  char            *answer_path;
  char            *answer_error;
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

#define DEVICES "/org/freedesktop/UPower/devices/"
#define DEVICE  "org.freedesktop.UPower.Device"

static GVariant *
mock_call (Fixture *fixture, const char *path, const char *interface, const char *method,
           GVariant *parameters)
{
  g_autoptr (GError) error = NULL;
  GVariant *reply = g_dbus_connection_call_sync (fixture->connection, UPOWER_NAME, path,
                                                 interface, method, parameters, NULL,
                                                 G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);

  g_assert_no_error (error);
  return reply;
}

/* A battery of the laptop, "BAT0" for the path DEVICES "BAT0". */
static void
add_battery (Fixture *fixture, const char *name, double percentage)
{
  g_variant_unref (mock_call (fixture, UPOWER_PATH, DBUSMOCK, "AddDischargingBattery",
                              g_variant_new ("(ssdx)", name, "Model", percentage, 3600)));
}

/* The properties of a device the template made, as a{sv} in text: "{'Type': <uint32 5>}". */
static void
set_properties (Fixture *fixture, const char *name, const char *properties)
{
  g_autofree char *path = g_strconcat (DEVICES, name, NULL);
  g_autoptr (GError) error = NULL;
  GVariant *value = g_variant_parse (G_VARIANT_TYPE ("a{sv}"), properties, NULL, NULL, &error);

  g_assert_no_error (error);
  g_variant_unref (mock_call (fixture, UPOWER_PATH, DBUSMOCK, "SetDeviceProperties",
                              g_variant_new ("(o@a{sv})", path, value)));
}

static void
add_mouse (Fixture *fixture, const char *name, double percentage)
{
  g_autofree char *properties = NULL;

  add_battery (fixture, name, percentage);
  properties = g_strdup_printf ("{'Type': <uint32 5>, 'PowerSupply': <false>, "
                                "'Model': <'MX Master 3S'>, 'Percentage': <%f>}", percentage);
  set_properties (fixture, name, properties);
}

static void
remove_device (Fixture *fixture, const char *name)
{
  g_autofree char *path = g_strconcat (DEVICES, name, NULL);

  g_variant_unref (mock_call (fixture, UPOWER_PATH, DBUSMOCK, "RemoveDevice",
                              g_variant_new ("(o)", path)));
}

/* The charge limit on a battery, as UPower has it: its properties, and a method that does
   what UPower's does, or that polkit refuses, or that is slow. */
static void
add_threshold (Fixture *fixture, const char *name, const char *code)
{
  g_autofree char *path = g_strconcat (DEVICES, name, NULL);

  g_variant_unref (mock_call (fixture, path, DBUSMOCK, "AddProperties",
                              g_variant_new_parsed ("(%s, {'ChargeThresholdSupported': <true>, "
                                                    "'ChargeThresholdEnabled': <false>, "
                                                    "'ChargeEndThreshold': <uint32 100>, "
                                                    "'Capacity': <58.0>})", DEVICE)));
  g_variant_unref (mock_call (fixture, path, DBUSMOCK, "AddMethod",
                              g_variant_new ("(sssss)", DEVICE, "EnableChargeThreshold", "b",
                                             "", code)));
}

#define UPOWERS_OWN \
  "self.Set('org.freedesktop.UPower.Device', 'ChargeThresholdEnabled', args[0]); " \
  "self.Set('org.freedesktop.UPower.Device', 'ChargeEndThreshold', " \
  "dbus.UInt32(80 if args[0] else 100))"
#define REFUSED \
  "raise dbus.exceptions.DBusException('Not authorized', " \
  "name='org.freedesktop.DBus.Error.AccessDenied')"

/* The table as text, a row to each ";": "Battery 1:73%:Discharging|MX Master 3S:55%:Mouse". */
static char *
table_text (Fixture *fixture)
{
  g_autoptr (GPtrArray) devices = pw_upower_devices (fixture->upower);
  g_autoptr (GPtrArray) rows = pw_state_rows (devices);
  GString *text = g_string_new (NULL);

  for (guint i = 0; i < rows->len; i++)
    {
      const PwRow *row = g_ptr_array_index (rows, i);

      g_string_append_printf (text, "%s%s:%s:%s%s", i > 0 ? "|" : "", row->name, row->percentage,
                              row->state, row->limit_enabled ? ":limit" : "");
    }
  return g_string_free (text, FALSE);
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
table_is (Fixture *fixture, gconstpointer expected)
{
  g_autofree char *text = table_text (fixture);

  return g_strcmp0 (text, expected) == 0;
}

static gboolean
has_answered (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return fixture->answers > 0;
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
  g_clear_pointer (&fixture->answer_path, g_free);
  g_clear_pointer (&fixture->answer_error, g_free);
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

static void
test_devices_come_and_go (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->upower = pw_upower_new (on_changed, fixture);
  wait_until (fixture, table_is, "");

  add_battery (fixture, "BAT0", 73);
  wait_until (fixture, table_is, "Battery:73%:Discharging");
  add_battery (fixture, "BAT1", 80);
  wait_until (fixture, table_is, "Battery 1:73%:Discharging|Battery 2:80%:Discharging");
  add_mouse (fixture, "mouse0", 55);
  wait_until (fixture, table_is,
              "Battery 1:73%:Discharging|Battery 2:80%:Discharging|MX Master 3S:55%:Discharging");

  /* A change of a device's own. */
  set_properties (fixture, "mouse0", "{'Percentage': <18.0>}");
  wait_until (fixture, table_is,
              "Battery 1:73%:Discharging|Battery 2:80%:Discharging|MX Master 3S:18%:Discharging");
  /* A mouse that is turned off is gone, or isn't present. */
  set_properties (fixture, "mouse0", "{'IsPresent': <false>}");
  wait_until (fixture, table_is, "Battery 1:73%:Discharging|Battery 2:80%:Discharging");
  set_properties (fixture, "mouse0", "{'IsPresent': <true>}");
  wait_until (fixture, table_is,
              "Battery 1:73%:Discharging|Battery 2:80%:Discharging|MX Master 3S:18%:Discharging");
  remove_device (fixture, "mouse0");
  wait_until (fixture, table_is, "Battery 1:73%:Discharging|Battery 2:80%:Discharging");
  remove_device (fixture, "BAT1");
  wait_until (fixture, table_is, "Battery:73%:Discharging");
}

static void
test_devices_there_at_the_start (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  add_battery (fixture, "BAT0", 73);
  add_mouse (fixture, "mouse0", 55);
  fixture->upower = pw_upower_new (on_changed, fixture);
  wait_until (fixture, table_is, "Battery:73%:Discharging|MX Master 3S:55%:Discharging");
}

static void
test_devices_follow_upowerd (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  add_battery (fixture, "BAT0", 73);
  fixture->upower = pw_upower_new (on_changed, fixture);
  wait_until (fixture, table_is, "Battery:73%:Discharging");

  stop_mock (fixture);
  wait_until (fixture, table_is, "");

  start_mock (fixture);
  add_battery (fixture, "BAT0", 40);
  wait_until (fixture, table_is, "Battery:40%:Discharging");
}

static void
on_answer (const char *path, const char *error, gpointer data)
{
  Fixture *fixture = data;

  fixture->answers++;
  g_free (fixture->answer_path);
  g_free (fixture->answer_error);
  fixture->answer_path = g_strdup (path);
  fixture->answer_error = g_strdup (error);
}

static void
test_charge_threshold (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  add_battery (fixture, "BAT0", 73);
  add_threshold (fixture, "BAT0", UPOWERS_OWN);
  fixture->upower = pw_upower_new (on_changed, fixture);
  wait_until (fixture, table_is, "Battery:73%:Discharging");

  /* On: UPower's own change follows. */
  pw_upower_enable_charge_threshold (fixture->upower, DEVICES "BAT0", TRUE, on_answer, fixture);
  wait_until (fixture, has_answered, NULL);
  g_assert_cmpstr (fixture->answer_path, ==, DEVICES "BAT0");
  g_assert_null (fixture->answer_error);
  wait_until (fixture, table_is, "Battery:73%:Discharging · limit 80%:limit");

  fixture->answers = 0;
  pw_upower_enable_charge_threshold (fixture->upower, DEVICES "BAT0", FALSE, on_answer, fixture);
  wait_until (fixture, has_answered, NULL);
  g_assert_null (fixture->answer_error);
  wait_until (fixture, table_is, "Battery:73%:Discharging");
}

static void
test_charge_threshold_refused (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  add_battery (fixture, "BAT0", 73);
  add_threshold (fixture, "BAT0", REFUSED);
  fixture->upower = pw_upower_new (on_changed, fixture);
  wait_until (fixture, table_is, "Battery:73%:Discharging");

  pw_upower_enable_charge_threshold (fixture->upower, DEVICES "BAT0", TRUE, on_answer, fixture);
  wait_until (fixture, has_answered, NULL);
  /* Polkit's refusal, said without D-Bus's name for it, and nothing changed. */
  g_assert_cmpstr (fixture->answer_error, ==, "Not authorized");
  g_assert_true (table_is (fixture, "Battery:73%:Discharging"));
}

static void
test_charge_threshold_of_a_battery_that_is_gone (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->upower = pw_upower_new (on_changed, fixture);
  wait_until (fixture, table_is, "");

  pw_upower_enable_charge_threshold (fixture->upower, DEVICES "BAT0", TRUE, on_answer, fixture);
  /* Never from inside the call. */
  g_assert_cmpuint (fixture->answers, ==, 0);
  wait_until (fixture, has_answered, NULL);
  g_assert_nonnull (fixture->answer_error);
}

static void
test_free_with_a_call_under_way (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  add_battery (fixture, "BAT0", 73);
  add_threshold (fixture, "BAT0", "import time; time.sleep(0.3); " UPOWERS_OWN);
  fixture->upower = pw_upower_new (on_changed, fixture);
  wait_until (fixture, table_is, "Battery:73%:Discharging");

  pw_upower_enable_charge_threshold (fixture->upower, DEVICES "BAT0", TRUE, on_answer, fixture);
  /* Freed before the answer, as the panel does when the applet is removed, and again
     with the call for a battery that isn't there: neither answers a freed applet. */
  pw_upower_enable_charge_threshold (fixture->upower, DEVICES "nothing", TRUE, on_answer, fixture);
  g_clear_pointer (&fixture->upower, pw_upower_free);
  for (int i = 0; i < 100; i++)
    {
      g_main_context_iteration (NULL, FALSE);
      g_usleep (10 * 1000);
    }
  g_assert_cmpuint (fixture->answers, ==, 0);
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
  g_test_add ("/upower/devices-come-and-go-and-change", Fixture, NULL, setup, test_devices_come_and_go, teardown);
  g_test_add ("/upower/devices-there-before-the-applet-are-listed", Fixture, NULL, setup, test_devices_there_at_the_start, teardown);
  g_test_add ("/upower/devices-follow-upowerd-stopping-and-starting-again", Fixture, NULL, setup, test_devices_follow_upowerd, teardown);
  g_test_add ("/upower/the-charge-limit-is-enabled-and-disabled", Fixture, NULL, setup, test_charge_threshold, teardown);
  g_test_add ("/upower/a-refused-charge-limit-says-so-and-changes-nothing", Fixture, NULL, setup, test_charge_threshold_refused, teardown);
  g_test_add ("/upower/a-charge-limit-of-a-missing-battery-answers-later", Fixture, NULL, setup, test_charge_threshold_of_a_battery_that_is_gone, teardown);
  g_test_add ("/upower/freeing-with-a-call-under-way-answers-nothing", Fixture, NULL, setup, test_free_with_a_call_under_way, teardown);
  g_test_add ("/upower/freeing-while-starting-calls-nothing", Fixture, NULL, setup, test_free_early, teardown);
  return g_test_run ();
}
