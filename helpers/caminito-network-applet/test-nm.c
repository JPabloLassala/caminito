/*
 * NetworkManager as the applet reads it (CMN-193), against python-dbusmock's networkmanager
 * template on a private bus that stands for the system bus: wired, Wi-Fi at each step of the
 * signal, a connection coming up, limited connectivity, a VPN, airplane mode, nothing up,
 * and NetworkManager stopping and starting again. Each state is checked as the panel shows
 * it: the tooltip and the icon's first name.
 *
 *   test-nm <python>
 */

#include <gio/gio.h>

#include "nw-nm.h"

#define NM_NAME     "org.freedesktop.NetworkManager"
#define NM_PATH     "/org/freedesktop/NetworkManager"
#define DBUSMOCK    "org.freedesktop.DBus.Mock"
#define DEVICES     NM_PATH "/Devices/"
#define ACTIVE      NM_PATH "/ActiveConnection/"
#define AP          NM_PATH "/AccessPoint/"

/* NMActiveConnectionState and NMConnectivityState. */
#define ACTIVATING 1
#define ACTIVATED  2
#define PORTAL     2
#define LIMITED    3
#define FULL       4

static const char *python;

typedef struct
{
  GTestDBus       *bus;
  GDBusConnection *connection;
  GSubprocess     *mock;
  NwNm            *nm;
  guint            changes;
} Fixture;

static void
on_changed (NwNm *nm G_GNUC_UNUSED, gpointer data)
{
  Fixture *fixture = data;

  fixture->changes++;
}

static GVariant *
call (Fixture *fixture, const char *path, const char *interface, const char *method,
      GVariant *parameters)
{
  g_autoptr (GError) error = NULL;
  GVariant *reply = g_dbus_connection_call_sync (fixture->connection, NM_NAME, path, interface,
                                                 method, parameters, NULL,
                                                 G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);

  g_assert_no_error (error);
  return reply;
}

static void
mock_call (Fixture *fixture, const char *path, const char *method, GVariant *parameters)
{
  g_variant_unref (call (fixture, path, DBUSMOCK, method, parameters));
}

/* Sets properties of the object at path, as text: "{'State': <uint32 2>}", with
   PropertiesChanged, as NetworkManager does. */
static void
update (Fixture *fixture, const char *path, const char *interface, const char *properties)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) values = g_variant_parse (G_VARIANT_TYPE ("a{sv}"), properties, NULL,
                                                 NULL, &error);
  GVariantIter iter;
  const char *name;
  GVariant *value;

  g_assert_no_error (error);
  /* The template's SetProperty, one at a time: dbusmock's UpdateProperties can't take an
     object path. */
  g_variant_iter_init (&iter, values);
  while (g_variant_iter_loop (&iter, "{&sv}", &name, &value))
    mock_call (fixture, NM_PATH, "SetProperty",
               g_variant_new ("(sssv)", path, interface, name, value));
}

static void
start_mock (Fixture *fixture)
{
  g_autoptr (GError) error = NULL;

  fixture->mock = g_subprocess_new (G_SUBPROCESS_FLAGS_STDOUT_SILENCE, &error, python, "-m",
                                    "dbusmock", "--system", "--template", "networkmanager",
                                    NULL);
  g_assert_no_error (error);
  for (int i = 0; i < 100; i++)
    {
      g_autoptr (GVariant) has = NULL;
      gboolean owned;

      has = g_dbus_connection_call_sync (fixture->connection, "org.freedesktop.DBus",
                                         "/org/freedesktop/DBus", "org.freedesktop.DBus",
                                         "NameHasOwner", g_variant_new ("(s)", NM_NAME),
                                         G_VARIANT_TYPE ("(b)"), G_DBUS_CALL_FLAGS_NONE, -1,
                                         NULL, &error);
      g_assert_no_error (error);
      g_variant_get (has, "(b)", &owned);
      if (owned)
        {
          /* NetworkManager's, which the template hasn't. */
          mock_call (fixture, NM_PATH, "AddProperty",
                     g_variant_new ("(ssv)", NM_NAME, "PrimaryConnection",
                                    g_variant_new_object_path ("/")));
          return;
        }
      g_usleep (100 * 1000);
    }
  g_error ("The mock never owned %s", NM_NAME);
}

static void
stop_mock (Fixture *fixture)
{
  g_subprocess_force_exit (fixture->mock);
  g_subprocess_wait (fixture->mock, NULL, NULL);
  g_clear_object (&fixture->mock);
}

static void
set_primary (Fixture *fixture, const char *path)
{
  g_autofree char *properties = g_strdup_printf ("{'PrimaryConnection': <objectpath '%s'>}",
                                                 path);

  update (fixture, NM_PATH, NM_NAME, properties);
}

static void
set_connectivity (Fixture *fixture, guint connectivity)
{
  g_autofree char *properties = g_strdup_printf ("{'Connectivity': <uint32 %u>}", connectivity);

  update (fixture, NM_PATH, NM_NAME, properties);
}

/* An ethernet device "eth0" with the connection id up on it, as active connection
   "wired": returns its path. */
static char *
add_wired (Fixture *fixture, const char *id, guint state)
{
  g_autoptr (GVariant) device = NULL;
  g_autoptr (GVariant) connection = NULL;
  g_autoptr (GVariant) active = NULL;
  g_autoptr (GError) error = NULL;
  g_autofree char *settings_text = g_strdup_printf (
    "({'connection': {'id': <'%s'>, 'type': <'802-3-ethernet'>}},)", id);
  g_autoptr (GVariant) settings = g_variant_parse (NULL, settings_text, NULL, NULL, &error);
  const char *device_path;
  const char *connection_path;
  char *active_path;

  g_assert_no_error (error);
  device = call (fixture, NM_PATH, DBUSMOCK, "AddEthernetDevice",
                 g_variant_new ("(ssi)", "eth0", "eth0", 100));
  g_variant_get (device, "(&s)", &device_path);
  connection = call (fixture, NM_PATH "/Settings", "org.freedesktop.NetworkManager.Settings",
                     "AddConnection", settings);
  g_variant_get (connection, "(&o)", &connection_path);
  active = call (fixture, NM_PATH, DBUSMOCK, "AddActiveConnection",
                 g_variant_new_parsed ("([%s], %s, '/', 'wired', %u)", device_path,
                                       connection_path, state));
  g_variant_get (active, "(s)", &active_path);
  return active_path;
}

/* A Wi-Fi device "wlan0" on an access point "Home" with the strength, the connection up on
   it as active connection "wifi" in state: returns its path. */
static char *
add_wifi (Fixture *fixture, guint strength, guint state)
{
  g_autoptr (GVariant) device = NULL;
  g_autoptr (GVariant) ap = NULL;
  g_autoptr (GVariant) connection = NULL;
  g_autoptr (GVariant) active = NULL;
  const char *device_path;
  const char *ap_path;
  const char *connection_path;
  g_autofree char *properties = NULL;
  char *active_path;

  device = call (fixture, NM_PATH, DBUSMOCK, "AddWiFiDevice",
                 g_variant_new ("(ssi)", "wlan0", "wlan0", 100));
  g_variant_get (device, "(&s)", &device_path);
  /* NetworkManager's, which the template hasn't. */
  mock_call (fixture, device_path, "AddProperty",
             g_variant_new ("(ssv)", NM_NAME ".Device.Wireless", "ActiveAccessPoint",
                            g_variant_new_object_path ("/")));
  ap = call (fixture, NM_PATH, DBUSMOCK, "AddAccessPoint",
             g_variant_new ("(ssssuuuyu)", device_path, "home", "Home", "00:11:22:33:44:55", 2,
                            2437, 54000, (guchar) strength, 0x100));
  g_variant_get (ap, "(&s)", &ap_path);
  connection = call (fixture, NM_PATH, DBUSMOCK, "AddWiFiConnection",
                     g_variant_new ("(ssss)", device_path, "home", "Home", "wpa-psk"));
  g_variant_get (connection, "(&s)", &connection_path);
  active = call (fixture, NM_PATH, DBUSMOCK, "AddActiveConnection",
                 g_variant_new_parsed ("([%s], %s, %s, 'wifi', %u)", device_path,
                                       connection_path, ap_path, state));
  g_variant_get (active, "(s)", &active_path);
  properties = g_strdup_printf ("{'ActiveAccessPoint': <objectpath '%s'>}", ap_path);
  update (fixture, device_path, NM_NAME ".Device.Wireless", properties);
  return active_path;
}

static void
set_strength (Fixture *fixture, guint strength)
{
  g_autofree char *properties = g_strdup_printf ("{'Strength': <byte %u>}", strength);

  update (fixture, AP "home", NM_NAME ".AccessPoint", properties);
}

static void
set_state (Fixture *fixture, const char *active_path, guint state)
{
  g_autofree char *properties = g_strdup_printf ("{'State': <uint32 %u>}", state);

  update (fixture, active_path, NM_NAME ".Connection.Active", properties);
}

/* A VPN up over the active connection at base, or "/" for a WireGuard connection, whose
   specific object NetworkManager leaves empty: returns its path. type is the connection's,
   "vpn" or "wireguard". */
static char *
add_vpn (Fixture *fixture, const char *id, const char *type, const char *base)
{
  g_autoptr (GVariant) connection = NULL;
  g_autoptr (GVariant) active = NULL;
  g_autoptr (GError) error = NULL;
  g_autofree char *settings_text = g_strdup_printf (
    "({'connection': {'id': <'%s'>, 'type': <'%s'>}},)", id, type);
  g_autoptr (GVariant) settings = g_variant_parse (NULL, settings_text, NULL, NULL, &error);
  const char *connection_path;
  char *active_path;

  g_assert_no_error (error);
  connection = call (fixture, NM_PATH "/Settings", "org.freedesktop.NetworkManager.Settings",
                     "AddConnection", settings);
  g_variant_get (connection, "(&o)", &connection_path);
  /* A VPN's specific object is the active connection it goes over. */
  active = call (fixture, NM_PATH, DBUSMOCK, "AddActiveConnection",
                 g_variant_new_parsed ("(@as [], %s, %s, %s, %u)", connection_path, base, id,
                                       ACTIVATED));
  g_variant_get (active, "(s)", &active_path);
  return active_path;
}

typedef gboolean (*Condition) (Fixture *fixture, gconstpointer expected);

/* Runs the main loop until condition holds, or fails after 5 s. */
static void
wait_until (Fixture *fixture, Condition condition, gconstpointer expected)
{
  gint64 deadline = g_get_monotonic_time () + 5 * G_USEC_PER_SEC;

  while (!condition (fixture, expected))
    {
      if (g_get_monotonic_time () >= deadline && fixture->nm != NULL)
        {
          g_autofree char *tooltip = nw_state_tooltip (nw_nm_status (fixture->nm));

          g_error ("expected %s, the applet shows %s|%s", (const char *) expected, tooltip,
                   nw_state_icon_names (nw_nm_status (fixture->nm))[0]);
        }
      g_assert_cmpint (g_get_monotonic_time (), <, deadline);
      g_main_context_iteration (NULL, FALSE);
      g_usleep (10 * 1000);
    }
}

/* "tooltip|first icon name". */
static gboolean
shows (Fixture *fixture, gconstpointer expected)
{
  const NwStatus *status = nw_nm_status (fixture->nm);
  g_autofree char *tooltip = nw_state_tooltip (status);
  g_autofree char *shown = g_strdup_printf ("%s|%s", tooltip, nw_state_icon_names (status)[0]);

  return g_strcmp0 (shown, expected) == 0;
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
  g_clear_pointer (&fixture->nm, nw_nm_free);
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
test_nothing_up (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, shows, "Not connected|network-offline");
}

static void
test_wired (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *wired = NULL;

  start_mock (fixture);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, shows, "Not connected|network-offline");
  wired = add_wired (fixture, "Wired connection 1", ACTIVATED);
  set_primary (fixture, wired);
  wait_until (fixture, shows, "Wired connection 1|network-wired");
  set_connectivity (fixture, LIMITED);
  wait_until (fixture, shows, "Wired connection 1 · no internet|network-wired-no-route");
  set_connectivity (fixture, FULL);
  wait_until (fixture, shows, "Wired connection 1|network-wired");
  set_connectivity (fixture, PORTAL);
  wait_until (fixture, shows, "Wired connection 1 · no internet|network-wired-no-route");
}

static void
test_wifi_steps (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *wifi = NULL;

  start_mock (fixture);
  wifi = add_wifi (fixture, 20, ACTIVATED);
  set_primary (fixture, wifi);
  /* Already up when the applet starts. */
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, shows, "Home · 20%|network-wireless-signal-weak");
  set_strength (fixture, 45);
  wait_until (fixture, shows, "Home · 45%|network-wireless-signal-ok");
  set_strength (fixture, 70);
  wait_until (fixture, shows, "Home · 70%|network-wireless-signal-good");
  set_strength (fixture, 95);
  wait_until (fixture, shows, "Home · 95%|network-wireless-signal-excellent");
}

static void
test_connecting (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *wifi = NULL;

  start_mock (fixture);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, shows, "Not connected|network-offline");
  wifi = add_wifi (fixture, 70, ACTIVATING);
  wait_until (fixture, shows, "Connecting to Home…|network-wireless-acquiring");
  set_state (fixture, wifi, ACTIVATED);
  set_primary (fixture, wifi);
  wait_until (fixture, shows, "Home · 70%|network-wireless-signal-good");
}

static void
test_vpn (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *wired = NULL;
  g_autofree char *vpn = NULL;
  g_autofree char *properties = NULL;

  start_mock (fixture);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wired = add_wired (fixture, "Office", ACTIVATED);
  set_primary (fixture, wired);
  wait_until (fixture, shows, "Office|network-wired");
  vpn = add_vpn (fixture, "Work", "vpn", wired);
  wait_until (fixture, shows, "Office\nVPN: Work|network-wired");
  /* A VPN that takes the default route is the primary connection: the icon is still the
     connection it goes over. */
  set_primary (fixture, vpn);
  wait_until (fixture, shows, "Office\nVPN: Work|network-wired");
  /* The VPN going down. The template's RemoveActiveConnection disconnects a device, which a
     VPN hasn't. */
  set_primary (fixture, wired);
  properties = g_strdup_printf ("{'ActiveConnections': <[objectpath '%s']>}", wired);
  update (fixture, NM_PATH, NM_NAME, properties);
  wait_until (fixture, shows, "Office|network-wired");
}

static void
test_wireguard (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *wired = NULL;
  g_autofree char *wireguard = NULL;

  start_mock (fixture);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wired = add_wired (fixture, "Office", ACTIVATED);
  wireguard = add_vpn (fixture, "wg0", "wireguard", "/");
  /* WireGuard taking the default route: the icon is the connection that's up under it. */
  set_primary (fixture, wireguard);
  wait_until (fixture, shows, "Office\nVPN: wg0|network-wired");
}

static void
test_airplane_mode (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, shows, "Not connected|network-offline");
  /* The template's WwanEnabled is already false. */
  update (fixture, NM_PATH, NM_NAME, "{'WirelessEnabled': <false>}");
  wait_until (fixture, shows, "Airplane mode|airplane-mode");
  update (fixture, NM_PATH, NM_NAME, "{'WwanEnabled': <true>}");
  wait_until (fixture, shows, "Not connected|network-offline");
}

static void
test_restart (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *wired = NULL;
  guint changes;

  start_mock (fixture);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wired = add_wired (fixture, "Office", ACTIVATED);
  set_primary (fixture, wired);
  wait_until (fixture, shows, "Office|network-wired");
  stop_mock (fixture);
  wait_until (fixture, shows, "NetworkManager isn't running|network-offline");
  changes = fixture->changes;
  start_mock (fixture);
  g_clear_pointer (&wired, g_free);
  wired = add_wired (fixture, "Office", ACTIVATED);
  set_primary (fixture, wired);
  wait_until (fixture, shows, "Office|network-wired");
  g_assert_cmpuint (fixture->changes, >, changes);
}

static void
test_late_start (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, shows, "NetworkManager isn't running|network-offline");
  start_mock (fixture);
  wait_until (fixture, shows, "Not connected|network-offline");
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  if (argc != 2)
    g_error ("usage: test-nm <python>");
  python = argv[1];
  g_test_add ("/nm/nothing-up-is-not-connected", Fixture, NULL, setup, test_nothing_up, teardown);
  g_test_add ("/nm/wired-and-its-connectivity-are-followed", Fixture, NULL, setup, test_wired, teardown);
  g_test_add ("/nm/wi-fi-follows-the-signal", Fixture, NULL, setup, test_wifi_steps, teardown);
  g_test_add ("/nm/a-connection-coming-up-then-up", Fixture, NULL, setup, test_connecting, teardown);
  g_test_add ("/nm/a-vpn-is-shown-over-its-connection", Fixture, NULL, setup, test_vpn, teardown);
  g_test_add ("/nm/wireguard-is-a-vpn-over-the-connection-up", Fixture, NULL, setup, test_wireguard, teardown);
  g_test_add ("/nm/airplane-mode-is-every-radio-off", Fixture, NULL, setup, test_airplane_mode, teardown);
  g_test_add ("/nm/networkmanager-stopping-and-starting-again-is-followed", Fixture, NULL, setup, test_restart, teardown);
  g_test_add ("/nm/networkmanager-starting-after-the-applet-is-followed", Fixture, NULL, setup, test_late_start, teardown);
  return g_test_run ();
}
