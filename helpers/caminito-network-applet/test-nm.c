/*
 * NetworkManager as the applet reads it (CMN-193), against python-dbusmock's networkmanager
 * template on a private bus that stands for the system bus: wired, Wi-Fi at each step of the
 * signal, a connection coming up, limited connectivity, a VPN, airplane mode, nothing up,
 * and NetworkManager stopping and starting again. Each state is checked as the panel shows
 * it: the tooltip and the icon's first name. Then the popup's tiles and card (CMN-196) as
 * they follow changes made elsewhere, and what their clicks do to the mock; and the Networks
 * tab's list and what joining a network does (CMN-200). The test's bus is the session bus
 * too, where the test stands for caminito-network's notifier, which the applet tells of the
 * user's activations (ADR 0044).
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
/* NMDeviceState. */
#define DEVICE_UNAVAILABLE  20
#define DEVICE_DISCONNECTED 30
#define DEVICE_ACTIVATED    100
/* The template's own methods the tests add, on an interface of their own. */
#define TEST_IFACE "org.mate.Caminito.Test"
/* NM80211ApSecurityFlags. */
#define SEC_NONE       0x000
#define SEC_PSK        0x100
#define SEC_ENTERPRISE 0x200

/* caminito-network's notifier (ADR 0044). */
#define NOTIFIER_NAME  "org.mate.Caminito.Network"
#define NOTIFIER_PATH  "/org/mate/Caminito/Network/Notifier"
#define NOTIFIER_IFACE "org.mate.Caminito.Network.Notifier"

static const char *python;

typedef struct
{
  GTestDBus       *bus;
  GDBusConnection *connection;
  /* The same bus as the session bus, where the test is the notifier. */
  GDBusConnection *session;
  GSubprocess     *mock;
  NwNm            *nm;
  guint            changes;
  /* The UUIDs the applet said the user is activating, as "uuid;uuid;". */
  GString         *activating;
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

/* A saved connection, as text: "{'connection': {'id': <'Office'>, …}}": returns its
   path. */
static char *
add_connection (Fixture *fixture, const char *settings_text)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) settings = g_variant_parse (G_VARIANT_TYPE ("a{sa{sv}}"), settings_text,
                                                   NULL, NULL, &error);
  g_autoptr (GVariant) connection = NULL;
  char *connection_path;

  g_assert_no_error (error);
  connection = call (fixture, NM_PATH "/Settings", "org.freedesktop.NetworkManager.Settings",
                     "AddConnection", g_variant_new_tuple (&settings, 1));
  g_variant_get (connection, "(o)", &connection_path);
  return connection_path;
}

/* An ethernet device "eth0" in the device state, with the connection id saved for it, as
   NetworkManager has it in AvailableConnections: returns the connection's path. */
static char *
add_ethernet (Fixture *fixture, const char *id, guint device_state)
{
  g_autoptr (GVariant) device = NULL;
  g_autofree char *settings = g_strdup_printf (
    "{'connection': {'id': <'%s'>, 'type': <'802-3-ethernet'>}}", id);
  g_autofree char *available = NULL;
  const char *device_path;
  char *connection_path;

  device = call (fixture, NM_PATH, DBUSMOCK, "AddEthernetDevice",
                 g_variant_new ("(ssi)", "eth0", "eth0", device_state));
  g_variant_get (device, "(&s)", &device_path);
  /* NetworkManager's, which the template hasn't: the device's connection goes down. */
  mock_call (fixture, device_path, "AddMethod",
             g_variant_new ("(sssss)", NM_NAME ".Device", "Disconnect", "", "",
                            "objects['" NM_PATH "'].RemoveActiveConnection(self.path,"
                            " self.Get('" NM_NAME ".Device', 'ActiveConnection'))"));
  connection_path = add_connection (fixture, settings);
  available = g_strdup_printf ("{'AvailableConnections': <[objectpath '%s']>}",
                               connection_path);
  update (fixture, device_path, NM_NAME ".Device", available);
  return connection_path;
}

/* An ethernet device "eth0" with the connection id up on it, as active connection
   "wired": returns its path. */
static char *
add_wired (Fixture *fixture, const char *id, guint state)
{
  g_autoptr (GVariant) active = NULL;
  g_autofree char *connection_path = add_ethernet (fixture, id, DEVICE_ACTIVATED);
  char *active_path;

  active = call (fixture, NM_PATH, DBUSMOCK, "AddActiveConnection",
                 g_variant_new_parsed ("([%o], %o, '/', 'wired', %u)", DEVICES "eth0",
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

/* A VPN saved, not up, last activated at the timestamp: returns its path. */
static char *
add_saved_vpn (Fixture *fixture, const char *id, guint64 timestamp)
{
  g_autofree char *settings = g_strdup_printf (
    "{'connection': {'id': <'%s'>, 'type': <'vpn'>, 'timestamp': <uint64 %" G_GUINT64_FORMAT
    ">}}", id, timestamp);

  return add_connection (fixture, settings);
}

/* The IPv4 address on the active connection at path, as NetworkManager gives it: an
   IP4Config object, which the template hasn't. */
static void
set_address (Fixture *fixture, const char *active_path, const char *address)
{
  g_autofree char *properties = NULL;

  /* The mock's AddObject over D-Bus doesn't tell the ObjectManager, which libnm follows. */
  mock_call (fixture, NM_PATH, "AddMethod",
             g_variant_new ("(sssss)", TEST_IFACE, "AddIP4Config", "ss", "",
                            "self.AddObject(args[0], 'org.freedesktop.NetworkManager.IP4Config',"
                            " {'AddressData': dbus.Array([{'address': args[1],"
                            " 'prefix': dbus.UInt32(24, variant_level=1)}],"
                            " signature='a{sv}'), 'Gateway': ''}, [])\n"
                            "self.object_manager_emit_added(args[0])"));
  g_variant_unref (call (fixture, NM_PATH, TEST_IFACE, "AddIP4Config",
                         g_variant_new ("(ss)", NM_PATH "/IP4Config/1", address)));
  mock_call (fixture, active_path, "AddProperty",
             g_variant_new ("(ssv)", NM_NAME ".Connection.Active", "Ip4Config",
                            g_variant_new_object_path ("/")));
  properties = g_strdup_printf ("{'Ip4Config': <objectpath '%s'>}", NM_PATH "/IP4Config/1");
  update (fixture, active_path, NM_NAME ".Connection.Active", properties);
}

static gboolean
get_boolean (Fixture *fixture, const char *name)
{
  g_autoptr (GVariant) reply = call (fixture, NM_PATH, "org.freedesktop.DBus.Properties", "Get",
                                     g_variant_new ("(ss)", NM_NAME, name));
  g_autoptr (GVariant) value = NULL;

  g_variant_get (reply, "(v)", &value);
  return g_variant_get_boolean (value);
}

/* The four tiles, "on Home, dim Unplugged, off Off, off Off": "dim" for an insensitive
   tile, which is never on. */
static char *
tiles_of (const NwStatus *status)
{
  GString *tiles = g_string_new (NULL);

  for (NwTile tile = 0; tile < NW_N_TILES; tile++)
    {
      NwTileState state = nw_state_tile (status, tile);

      g_string_append_printf (tiles, "%s%s %s", tile == 0 ? "" : ", ",
                              !state.sensitive ? "dim" : state.on ? "on" : "off", state.text);
    }
  return g_string_free (tiles, FALSE);
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
          g_autofree char *tiles = tiles_of (nw_nm_status (fixture->nm));

          g_error ("expected %s, the applet shows %s|%s, with the tiles %s",
                   (const char *) expected, tooltip,
                   nw_state_icon_names (nw_nm_status (fixture->nm))[0], tiles);
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
tiles_show (Fixture *fixture, gconstpointer expected)
{
  g_autofree char *tiles = tiles_of (nw_nm_status (fixture->nm));

  return g_strcmp0 (tiles, expected) == 0;
}

/* The card, "title|detail|address", "-" for what it hasn't. */
static gboolean
card_shows (Fixture *fixture, gconstpointer expected)
{
  const NwStatus *status = nw_nm_status (fixture->nm);
  g_autofree char *title = nw_state_card_title (status);
  g_autofree char *detail = nw_state_card_detail (status);
  g_autofree char *shown = g_strdup_printf ("%s|%s|%s", title, detail ? detail : "-",
                                            status->address ? status->address : "-");

  return g_strcmp0 (shown, expected) == 0;
}

static gboolean
connection_gone (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return fixture->connection == NULL && fixture->session == NULL;
}

static void
on_notifier_call (GDBusConnection *connection G_GNUC_UNUSED, const char *sender G_GNUC_UNUSED,
                  const char *path G_GNUC_UNUSED, const char *interface G_GNUC_UNUSED,
                  const char *method G_GNUC_UNUSED, GVariant *parameters,
                  GDBusMethodInvocation *invocation, gpointer data)
{
  Fixture *fixture = data;
  const char *uuid;

  g_variant_get (parameters, "(&s)", &uuid);
  g_string_append_printf (fixture->activating, "%s;", uuid);
  g_dbus_method_invocation_return_value (invocation, NULL);
}

/* The notifier's UserActivating, which records what it's told. */
static void
be_the_notifier (Fixture *fixture)
{
  static const GDBusInterfaceVTable vtable = { .method_call = on_notifier_call };
  g_autoptr (GDBusNodeInfo) node = NULL;
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = NULL;

  node = g_dbus_node_info_new_for_xml (
    "<node><interface name='" NOTIFIER_IFACE "'><method name='UserActivating'>"
    "<arg name='uuid' type='s' direction='in'/></method></interface></node>", &error);
  g_assert_no_error (error);
  g_dbus_connection_register_object (fixture->session, NOTIFIER_PATH, node->interfaces[0],
                                     &vtable, fixture, NULL, &error);
  g_assert_no_error (error);
  reply = g_dbus_connection_call_sync (fixture->session, "org.freedesktop.DBus",
                                       "/org/freedesktop/DBus", "org.freedesktop.DBus",
                                       "RequestName", g_variant_new ("(su)", NOTIFIER_NAME, 0),
                                       G_VARIANT_TYPE ("(u)"), G_DBUS_CALL_FLAGS_NONE, -1, NULL,
                                       &error);
  g_assert_no_error (error);
}

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GError) error = NULL;

  fixture->bus = g_test_dbus_new (G_TEST_DBUS_NONE);
  /* It sets the session bus's address too. */
  g_test_dbus_up (fixture->bus);
  /* GDBus reads the system bus's address from here, and so does dbusmock. */
  g_setenv ("DBUS_SYSTEM_BUS_ADDRESS", g_test_dbus_get_bus_address (fixture->bus), TRUE);
  fixture->connection = g_bus_get_sync (G_BUS_TYPE_SYSTEM, NULL, &error);
  g_assert_no_error (error);
  fixture->session = g_bus_get_sync (G_BUS_TYPE_SESSION, NULL, &error);
  g_assert_no_error (error);
  /* The bus goes down at the end of each test, which mustn't end the test program. */
  g_dbus_connection_set_exit_on_close (fixture->connection, FALSE);
  g_dbus_connection_set_exit_on_close (fixture->session, FALSE);
  fixture->activating = g_string_new (NULL);
  be_the_notifier (fixture);
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_clear_pointer (&fixture->nm, nw_nm_free);
  if (fixture->mock != NULL)
    stop_mock (fixture);
  /* GDBus keeps the buses' connections while anything holds them, and the next test's bus
     is a new one: wait for these to go before taking the bus down. */
  g_object_add_weak_pointer (G_OBJECT (fixture->connection), (gpointer *) &fixture->connection);
  g_object_add_weak_pointer (G_OBJECT (fixture->session), (gpointer *) &fixture->session);
  g_object_unref (fixture->connection);
  g_object_unref (fixture->session);
  wait_until (fixture, connection_gone, NULL);
  g_test_dbus_down (fixture->bus);
  g_clear_object (&fixture->bus);
  g_string_free (fixture->activating, TRUE);
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

/* NetworkManager's WwanEnabled is true unless the user turned it off, which the template's
   isn't: without it, Wi-Fi off would be airplane mode. */
static void
start_mock_with_wwan (Fixture *fixture)
{
  start_mock (fixture);
  update (fixture, NM_PATH, NM_NAME, "{'WwanEnabled': <true>}");
}

static void
test_tiles_follow_changes (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *connection = NULL;
  g_autofree char *wifi = NULL;

  start_mock_with_wwan (fixture);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, tiles_show, "on On, dim Unplugged, dim Off, off Off");
  connection = add_ethernet (fixture, "Office", DEVICE_UNAVAILABLE);
  wait_until (fixture, tiles_show, "on On, dim Unplugged, dim Off, off Off");
  /* The cable in. libnm reads the state from StateReason. */
  update (fixture, DEVICES "eth0", NM_NAME ".Device",
          "{'State': <uint32 30>, 'StateReason': <(uint32 30, uint32 0)>}");
  wait_until (fixture, tiles_show, "on On, off Disconnected, dim Off, off Off");
  wifi = add_wifi (fixture, 70, ACTIVATED);
  set_primary (fixture, wifi);
  wait_until (fixture, tiles_show, "on Home, off Disconnected, dim Off, off Off");
  /* nmcli radio wifi off. */
  update (fixture, NM_PATH, NM_NAME, "{'WirelessEnabled': <false>}");
  wait_until (fixture, tiles_show, "off Off, off Disconnected, dim Off, off Off");
  update (fixture, NM_PATH, NM_NAME, "{'WirelessEnabled': <true>}");
  wait_until (fixture, tiles_show, "on Home, off Disconnected, dim Off, off Off");
  /* The hardware switch. */
  update (fixture, NM_PATH, NM_NAME, "{'WirelessHardwareEnabled': <false>}");
  wait_until (fixture, tiles_show, "dim Off, off Disconnected, dim Off, off Off");
  update (fixture, NM_PATH, NM_NAME, "{'WirelessHardwareEnabled': <true>}");
  g_free (add_saved_vpn (fixture, "Work", 1000));
  wait_until (fixture, tiles_show, "on Home, off Disconnected, off Off, off Off");
  update (fixture, NM_PATH, NM_NAME, "{'WirelessEnabled': <false>, 'WwanEnabled': <false>}");
  wait_until (fixture, tiles_show, "dim Off, off Disconnected, off Off, on On");
}

static void
test_wifi_tile (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock_with_wwan (fixture);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, tiles_show, "on On, dim Unplugged, dim Off, off Off");
  nw_nm_set_tile (fixture->nm, NW_TILE_WIFI, FALSE);
  wait_until (fixture, tiles_show, "off Off, dim Unplugged, dim Off, off Off");
  g_assert_false (get_boolean (fixture, "WirelessEnabled"));
  nw_nm_set_tile (fixture->nm, NW_TILE_WIFI, TRUE);
  wait_until (fixture, tiles_show, "on On, dim Unplugged, dim Off, off Off");
  g_assert_true (get_boolean (fixture, "WirelessEnabled"));
}

static void
test_wired_tile (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock_with_wwan (fixture);
  g_free (add_ethernet (fixture, "Office", DEVICE_DISCONNECTED));
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, tiles_show, "on On, off Disconnected, dim Off, off Off");
  nw_nm_set_tile (fixture->nm, NW_TILE_WIRED, TRUE);
  wait_until (fixture, tiles_show, "on On, on Connected, dim Off, off Off");
  nw_nm_set_tile (fixture->nm, NW_TILE_WIRED, FALSE);
  wait_until (fixture, tiles_show, "on On, off Disconnected, dim Off, off Off");
}

static void
test_vpn_tile (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *wired = NULL;

  start_mock_with_wwan (fixture);
  wired = add_wired (fixture, "Office", ACTIVATED);
  set_primary (fixture, wired);
  g_free (add_saved_vpn (fixture, "Old", 1000));
  g_free (add_saved_vpn (fixture, "Work", 3000));
  g_free (add_saved_vpn (fixture, "Older", 500));
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, tiles_show, "on On, on Connected, off Off, off Off");
  g_assert_cmpstr (nw_nm_status (fixture->nm)->last_vpn, ==, "Work");
  nw_nm_set_tile (fixture->nm, NW_TILE_VPN, TRUE);
  wait_until (fixture, shows, "Office\nVPN: Work|network-wired");
  g_assert_true (tiles_show (fixture, "on On, on Connected, on Work, off Off"));
  nw_nm_set_tile (fixture->nm, NW_TILE_VPN, FALSE);
  /* The template's DeactivateConnection takes every device down with the VPN. */
  wait_until (fixture, shows, "Office|network-wired");
  g_assert_cmpstr (nw_nm_status (fixture->nm)->vpn, ==, NULL);
}

static void
test_airplane_tile (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock_with_wwan (fixture);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, tiles_show, "on On, dim Unplugged, dim Off, off Off");
  nw_nm_set_tile (fixture->nm, NW_TILE_AIRPLANE, TRUE);
  wait_until (fixture, tiles_show, "dim Off, dim Unplugged, dim Off, on On");
  g_assert_false (get_boolean (fixture, "WirelessEnabled"));
  g_assert_false (get_boolean (fixture, "WwanEnabled"));
  nw_nm_set_tile (fixture->nm, NW_TILE_AIRPLANE, FALSE);
  wait_until (fixture, tiles_show, "on On, dim Unplugged, dim Off, off Off");
  g_assert_true (get_boolean (fixture, "WirelessEnabled"));
  g_assert_true (get_boolean (fixture, "WwanEnabled"));

  /* Wi-Fi off before: it stays off after. */
  update (fixture, NM_PATH, NM_NAME, "{'WirelessEnabled': <false>}");
  wait_until (fixture, tiles_show, "off Off, dim Unplugged, dim Off, off Off");
  nw_nm_set_tile (fixture->nm, NW_TILE_AIRPLANE, TRUE);
  wait_until (fixture, tiles_show, "dim Off, dim Unplugged, dim Off, on On");
  nw_nm_set_tile (fixture->nm, NW_TILE_AIRPLANE, FALSE);
  wait_until (fixture, tiles_show, "off Off, dim Unplugged, dim Off, off Off");
  g_assert_false (get_boolean (fixture, "WirelessEnabled"));
  g_assert_true (get_boolean (fixture, "WwanEnabled"));

  /* Airplane mode from elsewhere, then off here: Wi-Fi comes back. */
  update (fixture, NM_PATH, NM_NAME, "{'WwanEnabled': <false>}");
  wait_until (fixture, tiles_show, "dim Off, dim Unplugged, dim Off, on On");
  nw_nm_set_tile (fixture->nm, NW_TILE_AIRPLANE, FALSE);
  wait_until (fixture, tiles_show, "on On, dim Unplugged, dim Off, off Off");
}

static void
test_card (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *wired = NULL;

  start_mock_with_wwan (fixture);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, card_shows, "Not connected|-|-");
  wired = add_wired (fixture, "Office", ACTIVATED);
  set_primary (fixture, wired);
  wait_until (fixture, card_shows, "Office|Connected|-");
  set_address (fixture, wired, "192.168.1.20");
  update (fixture, DEVICES "eth0", NM_NAME ".Device.Wired", "{'Speed': <uint32 1000>}");
  wait_until (fixture, card_shows, "Office|Connected · 1 Gb/s|192.168.1.20");
  set_connectivity (fixture, LIMITED);
  wait_until (fixture, card_shows, "Office|Connected · No internet · 1 Gb/s|192.168.1.20");
  nw_nm_disconnect (fixture->nm);
  wait_until (fixture, card_shows, "Not connected|-|-");
}

/* A Wi-Fi device "wlan0" with nothing up on it: returns its path. */
static char *
add_wifi_device (Fixture *fixture)
{
  g_autoptr (GVariant) device = call (fixture, NM_PATH, DBUSMOCK, "AddWiFiDevice",
                                      g_variant_new ("(ssi)", "wlan0", "wlan0",
                                                     DEVICE_DISCONNECTED));
  char *device_path;

  g_variant_get (device, "(s)", &device_path);
  /* NetworkManager's, which the template hasn't. */
  mock_call (fixture, device_path, "AddProperty",
             g_variant_new ("(ssv)", NM_NAME ".Device.Wireless", "ActiveAccessPoint",
                            g_variant_new_object_path ("/")));
  return device_path;
}

/* An access point of wlan0 named name, with the SSID, the strength and the security, as
   NM80211ApSecurityFlags. */
static void
add_ap (Fixture *fixture, const char *name, const char *ssid, guint strength, guint security)
{
  g_autoptr (GVariant) ap = call (fixture, NM_PATH, DBUSMOCK, "AddAccessPoint",
                                  g_variant_new ("(ssssuuuyu)", DEVICES "wlan0", name, ssid,
                                                 "00:11:22:33:44:66", 2, 2437, 54000,
                                                 (guchar) strength, security));

  (void) ap;
}

/* WEP: the privacy flag with neither WPA nor RSN, which the template's own can't make. */
static void
add_wep_ap (Fixture *fixture, const char *name, const char *ssid, guint strength)
{
  g_autofree char *path = g_strconcat (AP, name, NULL);

  add_ap (fixture, name, ssid, strength, SEC_NONE);
  update (fixture, path, NM_NAME ".AccessPoint", "{'Flags': <uint32 1>}");
}

/* The rows of the Networks tab: "Cafe 40%, Home lock 70% saved", with "lock" for a secured
   network, and "saved", "active" or "connecting". */
static char *
networks_of (const NwStatus *status)
{
  GString *networks = g_string_new (NULL);

  for (guint i = 0; i < status->n_networks; i++)
    {
      const NwNetwork *network = &status->networks[i];

      g_string_append_printf (networks, "%s%s", i == 0 ? "" : ", ", network->name);
      if (network->security != NW_SECURITY_NONE)
        g_string_append (networks, " lock");
      if (network->strength >= 0)
        g_string_append_printf (networks, " %d%%", network->strength);
      if (network->connecting)
        g_string_append (networks, " connecting");
      else if (network->active)
        g_string_append (networks, " active");
      else if (network->saved)
        g_string_append (networks, " saved");
    }
  return g_string_free (networks, FALSE);
}

static gboolean
networks_show (Fixture *fixture, gconstpointer expected)
{
  g_autofree char *networks = networks_of (nw_nm_status (fixture->nm));

  if (g_strcmp0 (networks, expected) != 0 && g_test_verbose ())
    g_printerr ("networks: %s\n", networks);
  return g_strcmp0 (networks, expected) == 0;
}

static void
test_networks (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *wifi = NULL;

  start_mock_with_wwan (fixture);
  g_free (add_ethernet (fixture, "Dock", DEVICE_DISCONNECTED));
  /* Home, up and primary: the card's, not the list's. */
  wifi = add_wifi (fixture, 70, ACTIVATED);
  set_primary (fixture, wifi);
  add_ap (fixture, "library", "Library", 50, SEC_PSK);
  add_ap (fixture, "library2", "Library", 80, SEC_PSK);
  g_variant_unref (call (fixture, NM_PATH, DBUSMOCK, "AddWiFiConnection",
                         g_variant_new ("(ssss)", DEVICES "wlan0", "library", "Library",
                                        "wpa-psk")));
  add_ap (fixture, "cafe", "Cafe", 40, SEC_NONE);
  add_ap (fixture, "eduroam", "eduroam", 60, SEC_ENTERPRISE);
  add_wep_ap (fixture, "old", "Old", 20);
  /* A hidden network, which "Connect to hidden network…" is for. */
  add_ap (fixture, "hidden", "", 90, SEC_PSK);
  g_free (add_saved_vpn (fixture, "Work", 1000));
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, networks_show,
              "Work saved, Library lock 80% saved, Dock saved, eduroam lock 60%, Cafe 40%,"
              " Old lock 20%");
  /* The cable out. */
  update (fixture, DEVICES "eth0", NM_NAME ".Device",
          "{'State': <uint32 20>, 'StateReason': <(uint32 20, uint32 0)>}");
  wait_until (fixture, networks_show,
              "Work saved, Library lock 80% saved, eduroam lock 60%, Cafe 40%, Old lock 20%");
  update (fixture, NM_PATH, NM_NAME, "{'WirelessEnabled': <false>}");
  wait_until (fixture, networks_show, "Work saved");
}

/* The settings of the connection at path, from the mock. */
static GVariant *
settings_of (Fixture *fixture, const char *path)
{
  g_autoptr (GVariant) reply = call (fixture, path, NM_NAME ".Settings.Connection",
                                     "GetSettings", NULL);

  return g_variant_get_child_value (reply, 0);
}

static char *
setting_string (GVariant *settings, const char *setting, const char *key)
{
  g_autoptr (GVariant) values = g_variant_lookup_value (settings, setting,
                                                        G_VARIANT_TYPE_VARDICT);
  char *value = NULL;

  if (values != NULL)
    g_variant_lookup (values, key, "s", &value);
  return value;
}

static gboolean
activated_one (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return fixture->activating->len > 0;
}

static void
test_join_new_network (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *device = NULL;
  g_autoptr (GVariant) connections = NULL;
  g_autoptr (GVariant) settings = NULL;
  g_autoptr (GVariant) security = NULL;
  g_autoptr (GVariant) connection = NULL;
  g_autoptr (GVariant) permissions = NULL;
  g_autofree const char **paths = NULL;
  g_autofree char *uuid = NULL;
  g_autofree char *key_mgmt = NULL;
  g_autofree char *psk = NULL;
  g_autofree char *said = NULL;
  guint32 flags = 0;

  start_mock_with_wwan (fixture);
  device = add_wifi_device (fixture);
  add_ap (fixture, "cafe", "Cafe", 60, SEC_PSK);
  /* The template's drops what it's given: the connection is saved as the applet sent it,
     then activated. */
  mock_call (fixture, NM_PATH, "AddMethod",
             g_variant_new ("(sssss)", NM_NAME, "AddAndActivateConnection", "a{sa{sv}}oo",
                            "oo",
                            "connection = self.SettingsAddConnection(args[0])\n"
                            "ret = (connection,"
                            " self.activate_connection(self, connection, args[1], args[2]))"));
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, networks_show, "Cafe lock 60%");
  nw_nm_join (fixture->nm, "ssid:Cafe", "correct horse");
  wait_until (fixture, networks_show, "Cafe lock 60% active");

  connections = call (fixture, NM_PATH "/Settings", NM_NAME ".Settings", "ListConnections",
                      NULL);
  g_variant_get (connections, "(^a&o)", &paths);
  g_assert_cmpuint (g_strv_length ((char **) paths), ==, 1);
  settings = settings_of (fixture, paths[0]);
  uuid = setting_string (settings, "connection", "uuid");
  key_mgmt = setting_string (settings, "802-11-wireless-security", "key-mgmt");
  psk = setting_string (settings, "802-11-wireless-security", "psk");
  g_assert_cmpstr (key_mgmt, ==, "wpa-psk");
  g_assert_cmpstr (psk, ==, "correct horse");
  /* The secret is NetworkManager's to keep: no flags, which libnm leaves out. */
  security = g_variant_lookup_value (settings, "802-11-wireless-security",
                                     G_VARIANT_TYPE_VARDICT);
  g_variant_lookup (security, "psk-flags", "u", &flags);
  g_assert_cmpuint (flags, ==, 0);
  /* For every user: no permissions, which libnm sends as an empty list. */
  connection = g_variant_lookup_value (settings, "connection", G_VARIANT_TYPE_VARDICT);
  permissions = g_variant_lookup_value (connection, "permissions", G_VARIANT_TYPE_STRING_ARRAY);
  g_assert_true (permissions == NULL || g_variant_n_children (permissions) == 0);
  /* caminito-network was told first, with the UUID the applet gave it. */
  wait_until (fixture, activated_one, NULL);
  said = g_strdup_printf ("%s;", uuid);
  g_assert_cmpstr (fixture->activating->str, ==, said);
}

static void
test_join_saved (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *device = NULL;
  g_autofree char *vpn = NULL;
  g_autoptr (GVariant) settings = NULL;
  g_autofree char *uuid = NULL;
  g_autofree char *said = NULL;

  start_mock_with_wwan (fixture);
  device = add_wifi_device (fixture);
  add_ap (fixture, "library", "Library", 80, SEC_PSK);
  g_variant_unref (call (fixture, NM_PATH, DBUSMOCK, "AddWiFiConnection",
                         g_variant_new ("(ssss)", DEVICES "wlan0", "library", "Library",
                                        "wpa-psk")));
  vpn = add_saved_vpn (fixture, "Work", 1000);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, networks_show, "Work saved, Library lock 80% saved");
  nw_nm_join (fixture->nm, "ssid:Library", NULL);
  wait_until (fixture, networks_show, "Library lock 80% active, Work saved");
  settings = settings_of (fixture, vpn);
  uuid = setting_string (settings, "connection", "uuid");
  said = g_strdup_printf ("uuid:%s", uuid);
  nw_nm_join (fixture->nm, said, NULL);
  wait_until (fixture, networks_show, "Library lock 80% active, Work active");
}

static void
test_tiles_tell_the_notifier (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *vpn = NULL;
  g_autoptr (GVariant) settings = NULL;
  g_autofree char *uuid = NULL;
  g_autofree char *said = NULL;

  start_mock_with_wwan (fixture);
  vpn = add_saved_vpn (fixture, "Work", 1000);
  fixture->nm = nw_nm_new (on_changed, fixture);
  wait_until (fixture, tiles_show, "on On, dim Unplugged, off Off, off Off");
  nw_nm_set_tile (fixture->nm, NW_TILE_VPN, TRUE);
  wait_until (fixture, activated_one, NULL);
  settings = settings_of (fixture, vpn);
  uuid = setting_string (settings, "connection", "uuid");
  said = g_strdup_printf ("%s;", uuid);
  g_assert_cmpstr (fixture->activating->str, ==, said);
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
  g_test_add ("/nm/the-tiles-follow-changes-made-elsewhere", Fixture, NULL, setup, test_tiles_follow_changes, teardown);
  g_test_add ("/nm/the-wi-fi-tile-turns-wireless-off-and-on", Fixture, NULL, setup, test_wifi_tile, teardown);
  g_test_add ("/nm/the-wired-tile-connects-and-disconnects-the-device", Fixture, NULL, setup, test_wired_tile, teardown);
  g_test_add ("/nm/the-vpn-tile-turns-the-vpn-used-last-on-and-off", Fixture, NULL, setup, test_vpn_tile, teardown);
  g_test_add ("/nm/airplane-mode-off-restores-the-radios-as-they-were", Fixture, NULL, setup, test_airplane_tile, teardown);
  g_test_add ("/nm/the-card-shows-the-connection-and-disconnects-it", Fixture, NULL, setup, test_card, teardown);
  g_test_add ("/nm/the-networks-are-listed-without-the-primary-one", Fixture, NULL, setup, test_networks, teardown);
  g_test_add ("/nm/joining-a-new-network-saves-its-password-for-every-user", Fixture, NULL, setup, test_join_new_network, teardown);
  g_test_add ("/nm/joining-a-saved-network-or-a-vpn-activates-it", Fixture, NULL, setup, test_join_saved, teardown);
  g_test_add ("/nm/the-tiles-tell-caminito-network-of-the-user-s-activations", Fixture, NULL, setup, test_tiles_tell_the_notifier, teardown);
  return g_test_run ();
}
