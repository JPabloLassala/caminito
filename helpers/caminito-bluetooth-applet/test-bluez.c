/*
 * BlueZ as the applet reads it (259), against python-dbusmock's bluez5 template on a
 * private bus that stands for the system bus: adapters and their power, a connected
 * device and its battery, and bluetoothd stopping and starting again.
 *
 *   test-bluez <python> <bluez-mock.py>
 */

#include <gio/gio.h>

#include "bt-bluez.h"

#define MOCK_INTERFACE  "org.bluez.Mock"
#define DBUSMOCK        "org.freedesktop.DBus.Mock"
#define HCI0            "/org/bluez/hci0"
#define HEADPHONES      "11:22:33:44:55:66"
#define HEADPHONES_PATH HCI0 "/dev_11_22_33_44_55_66"

static const char *python;
static const char *template;

typedef struct
{
  GTestDBus       *bus;
  GDBusConnection *connection;
  GSubprocess     *mock;
  BtBluez         *bluez;
  guint            changes;
} Fixture;

static void
on_changed (BtBluez *bluez G_GNUC_UNUSED, gpointer data)
{
  Fixture *fixture = data;

  fixture->changes++;
}

static void
start_mock (Fixture *fixture)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = NULL;

  fixture->mock = g_subprocess_new (G_SUBPROCESS_FLAGS_STDOUT_SILENCE, &error,
                                    python, "-m", "dbusmock", "--system", "--template",
                                    template, NULL);
  g_assert_no_error (error);
  /* The mock owns org.bluez once it serves the template's objects. */
  for (int i = 0; i < 100; i++)
    {
      g_autoptr (GVariant) has = NULL;
      gboolean owned;

      has = g_dbus_connection_call_sync (fixture->connection,
                                         "org.freedesktop.DBus", "/org/freedesktop/DBus",
                                         "org.freedesktop.DBus", "NameHasOwner",
                                         g_variant_new ("(s)", "org.bluez"),
                                         G_VARIANT_TYPE ("(b)"), G_DBUS_CALL_FLAGS_NONE, -1,
                                         NULL, &error);
      g_assert_no_error (error);
      g_variant_get (has, "(b)", &owned);
      if (owned)
        return;
      g_usleep (100 * 1000);
    }
  g_error ("The mock never owned org.bluez");
}

static void
stop_mock (Fixture *fixture)
{
  g_subprocess_force_exit (fixture->mock);
  g_subprocess_wait (fixture->mock, NULL, NULL);
  g_clear_object (&fixture->mock);
}

static GVariant *
call (Fixture *fixture, const char *path, const char *interface, const char *method,
      GVariant *parameters)
{
  g_autoptr (GError) error = NULL;
  GVariant *reply;

  reply = g_dbus_connection_call_sync (fixture->connection, "org.bluez", path, interface,
                                       method, parameters, NULL, G_DBUS_CALL_FLAGS_NONE, -1,
                                       NULL, &error);
  g_assert_no_error (error);
  return reply;
}

static void
mock (Fixture *fixture, const char *method, GVariant *parameters)
{
  g_variant_unref (call (fixture, "/", MOCK_INTERFACE, method, parameters));
}

static void
set_powered (Fixture *fixture, gboolean powered)
{
  GVariantBuilder properties;

  g_variant_builder_init (&properties, G_VARIANT_TYPE_VARDICT);
  g_variant_builder_add (&properties, "{sv}", "Powered", g_variant_new_boolean (powered));
  g_variant_unref (call (fixture, HCI0, DBUSMOCK, "UpdateProperties",
                         g_variant_new ("(sa{sv})", "org.bluez.Adapter1", &properties)));
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
icon_is (Fixture *fixture, gconstpointer expected)
{
  g_autoptr (BtSnapshot) snapshot = bt_bluez_snapshot (fixture->bluez);

  return bt_state_icon (snapshot) == (BtIconState) GPOINTER_TO_INT (expected);
}

static gboolean
tooltip_is (Fixture *fixture, gconstpointer expected)
{
  g_autoptr (BtSnapshot) snapshot = bt_bluez_snapshot (fixture->bluez);
  g_autofree char *tooltip = bt_state_tooltip (snapshot);

  return g_strcmp0 (tooltip, expected) == 0;
}

static gboolean
connection_gone (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return fixture->connection == NULL;
}

static void
wait_for_icon (Fixture *fixture, BtIconState expected)
{
  wait_until (fixture, icon_is, GINT_TO_POINTER (expected));
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
  g_clear_pointer (&fixture->bluez, bt_bluez_free);
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
  fixture->bluez = bt_bluez_new (on_changed, fixture);
  /* The template starts with no adapter. */
  while (fixture->changes == 0)
    g_main_context_iteration (NULL, TRUE);
  wait_for_icon (fixture, BT_ICON_NONE);

  mock (fixture, "AddAdapter", g_variant_new ("(ss)", "hci0", "my-computer"));
  wait_for_icon (fixture, BT_ICON_ON);
  set_powered (fixture, FALSE);
  wait_for_icon (fixture, BT_ICON_OFF);
  set_powered (fixture, TRUE);
  wait_for_icon (fixture, BT_ICON_ON);

  mock (fixture, "AddDevice", g_variant_new ("(sss)", "hci0", HEADPHONES, "Headphones"));
  mock (fixture, "PairDevice", g_variant_new ("(ss)", "hci0", HEADPHONES));
  /* The device's own methods, which the popup will call: the template's ConnectDevice
     leaves Disconnect thinking it isn't connected. */
  g_variant_unref (call (fixture, HEADPHONES_PATH, "org.bluez.Device1", "Connect", NULL));
  wait_for_icon (fixture, BT_ICON_CONNECTED);
  mock (fixture, "AddBattery", g_variant_new ("(oy)", HEADPHONES_PATH, 80));
  wait_until (fixture, tooltip_is, "Bluetooth is on\nHeadphones: 80%");

  g_variant_unref (call (fixture, HEADPHONES_PATH, "org.bluez.Device1", "Disconnect", NULL));
  wait_for_icon (fixture, BT_ICON_ON);
}

static void
test_restart (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->bluez = bt_bluez_new (on_changed, fixture);
  mock (fixture, "AddAdapter", g_variant_new ("(ss)", "hci0", "my-computer"));
  wait_for_icon (fixture, BT_ICON_ON);

  stop_mock (fixture);
  wait_for_icon (fixture, BT_ICON_NONE);

  start_mock (fixture);
  mock (fixture, "AddAdapter", g_variant_new ("(ss)", "hci0", "my-computer"));
  wait_for_icon (fixture, BT_ICON_ON);
}

/* bluetoothd isn't running when the applet starts, as on a computer that has no
   Bluetooth until a dongle is plugged in. */
static void
test_late_start (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  fixture->bluez = bt_bluez_new (on_changed, fixture);
  while (fixture->changes == 0)
    g_main_context_iteration (NULL, TRUE);
  wait_for_icon (fixture, BT_ICON_NONE);

  start_mock (fixture);
  mock (fixture, "AddAdapter", g_variant_new ("(ss)", "hci0", "my-computer"));
  wait_for_icon (fixture, BT_ICON_ON);
}

static void
test_set_powered (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->bluez = bt_bluez_new (on_changed, fixture);
  mock (fixture, "AddAdapter", g_variant_new ("(ss)", "hci0", "my-computer"));
  mock (fixture, "AddAdapter", g_variant_new ("(ss)", "hci1", "my-computer"));
  wait_for_icon (fixture, BT_ICON_ON);

  bt_bluez_set_powered (fixture->bluez, FALSE);
  wait_for_icon (fixture, BT_ICON_OFF);
  bt_bluez_set_powered (fixture->bluez, TRUE);
  wait_for_icon (fixture, BT_ICON_ON);
}

/* The applet removed from the panel while bluez is still starting. */
static void
test_free_early (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->bluez = bt_bluez_new (on_changed, fixture);
  g_clear_pointer (&fixture->bluez, bt_bluez_free);
  /* That nothing is called: half a second is many times what the mock takes to answer
     GetManagedObjects on a private bus, a few milliseconds. */
  for (int i = 0; i < 50; i++)
    {
      g_main_context_iteration (NULL, FALSE);
      g_usleep (10 * 1000);
    }
  g_assert_cmpuint (fixture->changes, ==, 0);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  if (argc != 3)
    {
      g_printerr ("usage: %s <python> <bluez-mock.py>\n", argv[0]);
      return 2;
    }
  python = argv[1];
  template = argv[2];

  g_test_add ("/bluez/the-icon-follows-adapters-devices-and-batteries", Fixture, NULL, setup, test_states, teardown);
  g_test_add ("/bluez/bluetoothd-stopping-and-starting-again-is-followed", Fixture, NULL, setup, test_restart, teardown);
  g_test_add ("/bluez/bluetoothd-starting-after-the-applet-is-followed", Fixture, NULL, setup, test_late_start, teardown);
  g_test_add ("/bluez/powering-sets-every-adapter", Fixture, NULL, setup, test_set_powered, teardown);
  g_test_add ("/bluez/freeing-while-starting-calls-nothing", Fixture, NULL, setup, test_free_early, teardown);
  return g_test_run ();
}
