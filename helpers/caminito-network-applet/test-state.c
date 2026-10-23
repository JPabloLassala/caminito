/*
 * The icon's names, the lock and the tooltip (CMN-193), the popup's tiles and card
 * (CMN-196), and the Networks tab's order, search and bar (CMN-200), from statuses built by
 * hand.
 */

#include "nw-state.h"

static NwStatus
connected (NwKind kind, const char *name, int strength)
{
  return (NwStatus) {
    .running = TRUE,
    .phase = NW_PHASE_CONNECTED,
    .kind = kind,
    .name = (char *) name,
    .strength = strength,
  };
}

static void
test_not_running (void)
{
  NwStatus status = { .running = FALSE };
  const char *const *names = nw_state_icon_names (&status);
  g_autofree char *tooltip = nw_state_tooltip (&status);

  g_assert_cmpstr (names[0], ==, "network-offline");
  g_assert_cmpstr (names[1], ==, "network-offline-symbolic");
  g_assert_null (names[2]);
  g_assert_cmpstr (tooltip, ==, "NetworkManager isn't running");
}

static void
test_not_connected (void)
{
  NwStatus status = { .running = TRUE, .phase = NW_PHASE_NONE };
  const char *const *names = nw_state_icon_names (&status);
  g_autofree char *tooltip = nw_state_tooltip (&status);

  g_assert_cmpstr (names[0], ==, "network-offline");
  g_assert_cmpstr (tooltip, ==, "Not connected");
}

static void
test_airplane_mode (void)
{
  NwStatus status = { .running = TRUE, .airplane = TRUE, .phase = NW_PHASE_NONE };
  const char *const *names = nw_state_icon_names (&status);
  g_autofree char *tooltip = nw_state_tooltip (&status);

  /* Breeze's last, which has neither of the others. */
  g_assert_cmpstr (names[0], ==, "airplane-mode");
  g_assert_cmpstr (names[1], ==, "airplane-mode-symbolic");
  g_assert_cmpstr (names[2], ==, "network-flightmode-on");
  g_assert_cmpstr (tooltip, ==, "Airplane mode");
}

static void
test_wired_wins_over_airplane_mode (void)
{
  NwStatus status = connected (NW_KIND_WIRED, "Wired connection 1", -1);
  g_autofree char *tooltip = NULL;

  status.airplane = TRUE;
  tooltip = nw_state_tooltip (&status);
  g_assert_cmpstr (nw_state_icon_names (&status)[0], ==, "network-wired");
  g_assert_cmpstr (nw_state_icon_names (&status)[1], ==, "network-wired-symbolic");
  g_assert_cmpstr (tooltip, ==, "Wired connection 1");
}

static void
test_wifi_steps (void)
{
  struct { int strength; const char *name; } cases[] = {
    { 0, "network-wireless-signal-weak" },
    { 30, "network-wireless-signal-weak" },
    { 31, "network-wireless-signal-ok" },
    { 55, "network-wireless-signal-ok" },
    { 56, "network-wireless-signal-good" },
    { 80, "network-wireless-signal-good" },
    { 81, "network-wireless-signal-excellent" },
    { 100, "network-wireless-signal-excellent" },
    /* No access point known yet. */
    { -1, "network-wireless-signal-excellent" },
  };

  for (guint i = 0; i < G_N_ELEMENTS (cases); i++)
    {
      NwStatus status = connected (NW_KIND_WIFI, "Home", cases[i].strength);
      const char *const *names = nw_state_icon_names (&status);
      g_autofree char *symbolic = g_strconcat (cases[i].name, "-symbolic", NULL);

      g_assert_cmpstr (names[0], ==, cases[i].name);
      g_assert_cmpstr (names[1], ==, symbolic);
    }
}

static void
test_wifi_tooltip (void)
{
  NwStatus status = connected (NW_KIND_WIFI, "Home", 72);
  NwStatus unknown = connected (NW_KIND_WIFI, "Home", -1);
  g_autofree char *tooltip = nw_state_tooltip (&status);
  g_autofree char *without = nw_state_tooltip (&unknown);

  g_assert_cmpstr (tooltip, ==, "Home · 72%");
  g_assert_cmpstr (without, ==, "Home");
}

static void
test_limited (void)
{
  NwStatus wifi = connected (NW_KIND_WIFI, "Café", 45);
  NwStatus wired = connected (NW_KIND_WIRED, "Office", -1);
  g_autofree char *wifi_tooltip = NULL;
  g_autofree char *wired_tooltip = NULL;

  wifi.limited = wired.limited = TRUE;
  wifi_tooltip = nw_state_tooltip (&wifi);
  wired_tooltip = nw_state_tooltip (&wired);
  g_assert_cmpstr (nw_state_icon_names (&wifi)[0], ==, "network-wireless-no-route");
  g_assert_cmpstr (nw_state_icon_names (&wifi)[1], ==, "network-wireless-no-route-symbolic");
  /* Breeze's has the signal in it. */
  g_assert_cmpstr (nw_state_icon_names (&wifi)[2], ==, "network-wireless-40-limited");
  g_assert_cmpstr (nw_state_icon_names (&wired)[0], ==, "network-wired-no-route");
  g_assert_cmpstr (nw_state_icon_names (&wired)[2], ==, "network-wired-activated-limited");
  g_assert_cmpstr (wifi_tooltip, ==, "Café · 45% · no internet");
  g_assert_cmpstr (wired_tooltip, ==, "Office · no internet");
}

static void
test_connecting (void)
{
  NwStatus wifi = { .running = TRUE, .phase = NW_PHASE_CONNECTING, .kind = NW_KIND_WIFI,
                    .name = "Home", .strength = -1 };
  NwStatus wired = { .running = TRUE, .phase = NW_PHASE_CONNECTING, .kind = NW_KIND_WIRED,
                     .name = "Office", .strength = -1 };
  g_autofree char *tooltip = nw_state_tooltip (&wifi);

  g_assert_cmpstr (nw_state_icon_names (&wifi)[0], ==, "network-wireless-acquiring");
  g_assert_cmpstr (nw_state_icon_names (&wifi)[1], ==, "network-wireless-acquiring-symbolic");
  g_assert_cmpstr (nw_state_icon_names (&wired)[0], ==, "network-wired-acquiring");
  g_assert_cmpstr (nw_state_icon_names (&wired)[1], ==, "network-wired-acquiring-symbolic");
  g_assert_cmpstr (tooltip, ==, "Connecting to Home…");
}

static void
test_vpn (void)
{
  NwStatus status = connected (NW_KIND_WIFI, "Home", 72);
  g_autofree char *tooltip = NULL;

  status.vpn = "Work";
  tooltip = nw_state_tooltip (&status);
  g_assert_cmpstr (tooltip, ==, "Home · 72%\nVPN: Work");
  /* The icon is the connection's: the lock goes over it. */
  g_assert_cmpstr (nw_state_icon_names (&status)[0], ==, "network-wireless-signal-good");
  g_assert_cmpstr (nw_state_lock_names ()[0], ==, "changes-prevent");
}

static void
test_equal (void)
{
  NwStatus status = connected (NW_KIND_WIFI, "Home", 72);
  NwStatus other = connected (NW_KIND_WIFI, g_strdup ("Home"), 72);

  status.vpn = "Work";
  other.vpn = g_strdup ("Work");
  g_assert_true (nw_status_equal (&other, &status));
  status.strength = 73;
  g_assert_false (nw_status_equal (&other, &status));
  status.strength = 72;
  other.wifi_network = g_strdup ("Home");
  g_assert_false (nw_status_equal (&other, &status));
  status.wifi_network = "Home";
  g_assert_true (nw_status_equal (&other, &status));
  other.address = g_strdup ("192.168.1.20");
  g_assert_false (nw_status_equal (&other, &status));
  status.address = "192.168.1.20";
  other.last_vpn = g_strdup ("Work");
  g_assert_false (nw_status_equal (&other, &status));
  status.last_vpn = "Work";
  status.wired = NW_WIRED_UNPLUGGED;
  g_assert_false (nw_status_equal (&other, &status));
  status.wired = other.wired;
  status.speed = 1000;
  g_assert_false (nw_status_equal (&other, &status));
  status.speed = other.speed;
  status.wifi_enabled = TRUE;
  g_assert_false (nw_status_equal (&other, &status));
  status.wifi_enabled = FALSE;
  status.wifi_blocked = TRUE;
  g_assert_false (nw_status_equal (&other, &status));
  status.wifi_blocked = FALSE;
  other.n_networks = 1;
  other.networks = g_new0 (NwNetwork, 1);
  other.networks[0] = (NwNetwork) { .key = g_strdup ("ssid:Cafe"), .name = g_strdup ("Cafe"),
                                    .kind = NW_NETWORK_WIFI, .strength = 40 };
  g_assert_false (nw_status_equal (&other, &status));
  status.n_networks = 1;
  status.networks = &(NwNetwork) { .key = "ssid:Cafe", .name = "Cafe", .kind = NW_NETWORK_WIFI,
                                   .strength = 40 };
  g_assert_true (nw_status_equal (&other, &status));
  status.networks[0].strength = 41;
  g_assert_false (nw_status_equal (&other, &status));
  status.networks[0].strength = 40;
  status.networks[0].connecting = TRUE;
  g_assert_false (nw_status_equal (&other, &status));
  nw_status_clear (&other);
  g_assert_null (other.networks);
  g_assert_cmpuint (other.n_networks, ==, 0);
  g_assert_null (other.name);
  g_assert_null (other.wifi_network);
  g_assert_null (other.address);
  g_assert_null (other.last_vpn);
}

static void
assert_tile (const NwStatus *status, NwTile tile, gboolean on, gboolean sensitive,
             const char *text)
{
  NwTileState state = nw_state_tile (status, tile);

  g_assert_cmpint (state.on, ==, on);
  g_assert_cmpint (state.sensitive, ==, sensitive);
  g_assert_cmpstr (state.text, ==, text);
}

static void
test_wifi_tile (void)
{
  NwStatus status = connected (NW_KIND_WIFI, "Home", 72);

  status.wifi_enabled = TRUE;
  status.wifi_network = "Home";
  assert_tile (&status, NW_TILE_WIFI, TRUE, TRUE, "Home");
  status = (NwStatus) { .running = TRUE, .wifi_enabled = TRUE };
  assert_tile (&status, NW_TILE_WIFI, TRUE, TRUE, "On");
  status.wifi_enabled = FALSE;
  assert_tile (&status, NW_TILE_WIFI, FALSE, TRUE, "Off");
  status.airplane = TRUE;
  assert_tile (&status, NW_TILE_WIFI, FALSE, FALSE, "Off");
  /* The hardware switch off: WirelessEnabled stays as the user left it. */
  status = (NwStatus) { .running = TRUE, .wifi_enabled = TRUE, .wifi_blocked = TRUE };
  assert_tile (&status, NW_TILE_WIFI, FALSE, FALSE, "Off");
}

static void
test_wired_tile (void)
{
  NwStatus status = { .running = TRUE, .wired = NW_WIRED_CONNECTED };

  assert_tile (&status, NW_TILE_WIRED, TRUE, TRUE, "Connected");
  status.wired = NW_WIRED_CONNECTING;
  assert_tile (&status, NW_TILE_WIRED, TRUE, TRUE, "Connecting…");
  status.wired = NW_WIRED_DISCONNECTED;
  assert_tile (&status, NW_TILE_WIRED, FALSE, TRUE, "Disconnected");
  status.wired = NW_WIRED_UNPLUGGED;
  assert_tile (&status, NW_TILE_WIRED, FALSE, FALSE, "Unplugged");
  /* No wired device at all is a cable never plugged in. */
  status.wired = NW_WIRED_ABSENT;
  assert_tile (&status, NW_TILE_WIRED, FALSE, FALSE, "Unplugged");
}

static void
test_vpn_tile (void)
{
  NwStatus status = { .running = TRUE, .vpn = "Work", .last_vpn = "Work" };

  assert_tile (&status, NW_TILE_VPN, TRUE, TRUE, "Work");
  status.vpn = NULL;
  assert_tile (&status, NW_TILE_VPN, FALSE, TRUE, "Off");
  /* No VPN set up: nothing to turn on. */
  status.last_vpn = NULL;
  assert_tile (&status, NW_TILE_VPN, FALSE, FALSE, "Off");
}

static void
test_airplane_tile (void)
{
  NwStatus status = { .running = TRUE, .airplane = TRUE };

  assert_tile (&status, NW_TILE_AIRPLANE, TRUE, TRUE, "On");
  status.airplane = FALSE;
  assert_tile (&status, NW_TILE_AIRPLANE, FALSE, TRUE, "Off");
}

static void
test_card (void)
{
  NwStatus status = connected (NW_KIND_WIRED, "Office", -1);
  g_autofree char *title = nw_state_card_title (&status);
  g_autofree char *detail = NULL;

  g_assert_cmpstr (title, ==, "Office");
  detail = nw_state_card_detail (&status);
  g_assert_cmpstr (detail, ==, "Connected");
  g_clear_pointer (&detail, g_free);
  status.speed = 1000;
  detail = nw_state_card_detail (&status);
  g_assert_cmpstr (detail, ==, "Connected · 1 Gb/s");
  g_clear_pointer (&detail, g_free);
  status.speed = 2500;
  status.limited = TRUE;
  detail = nw_state_card_detail (&status);
  g_assert_cmpstr (detail, ==, "Connected · No internet · 2.5 Gb/s");
  g_clear_pointer (&detail, g_free);
  status.speed = 100;
  status.limited = FALSE;
  detail = nw_state_card_detail (&status);
  g_assert_cmpstr (detail, ==, "Connected · 100 Mb/s");
  g_clear_pointer (&detail, g_free);

  /* Wi-Fi has no link speed on the card, which the Details tab has (CMN-201). */
  status = connected (NW_KIND_WIFI, "Home", 72);
  status.speed = 300;
  status.limited = TRUE;
  detail = nw_state_card_detail (&status);
  g_assert_cmpstr (detail, ==, "Connected · No internet");
}

static void
test_card_without_a_connection (void)
{
  NwStatus status = { .running = TRUE, .phase = NW_PHASE_CONNECTING, .kind = NW_KIND_WIFI,
                      .name = "Home", .strength = 70 };
  g_autofree char *title = nw_state_card_title (&status);
  g_autofree char *detail = nw_state_card_detail (&status);

  g_assert_cmpstr (title, ==, "Connecting to Home…");
  g_assert_null (detail);
  g_clear_pointer (&title, g_free);
  status = (NwStatus) { .running = TRUE, .airplane = TRUE };
  title = nw_state_card_title (&status);
  g_assert_cmpstr (title, ==, "Not connected");
  g_assert_null (nw_state_card_detail (&status));
}

static NwNetwork
network (NwNetworkKind kind, const char *name, int strength)
{
  return (NwNetwork) { .key = (char *) name, .name = (char *) name, .kind = kind,
                       .strength = strength };
}

static void
test_order (void)
{
  NwNetwork networks[] = {
    network (NW_NETWORK_WIFI, "Cafe", 40),
    network (NW_NETWORK_VPN, "Work", -1),
    network (NW_NETWORK_WIFI, "Airport", 40),
    network (NW_NETWORK_WIFI, "Library", 80),
    network (NW_NETWORK_WIFI, "Neighbour", 90),
    network (NW_NETWORK_WIRED, "Dock", -1),
    network (NW_NETWORK_VPN, "Office VPN", -1),
    network (NW_NETWORK_WIFI, "Phone", 30),
    network (NW_NETWORK_VPN, "Home VPN", -1),
  };
  g_autoptr (GString) order = g_string_new (NULL);

  networks[2].saved = networks[3].saved = networks[5].saved = TRUE;
  networks[1].saved = networks[6].saved = networks[8].saved = TRUE;
  networks[6].active = TRUE;
  /* Coming up is with the active ones. */
  networks[7].connecting = TRUE;
  nw_networks_sort (networks, G_N_ELEMENTS (networks));
  for (guint i = 0; i < G_N_ELEMENTS (networks); i++)
    g_string_append_printf (order, "%s%s", i == 0 ? "" : ", ", networks[i].name);
  /* Active, then VPNs, then saved, then available, each by signal then name. */
  g_assert_cmpstr (order->str, ==, "Phone, Office VPN, Home VPN, Work, Library, Airport, Dock,"
                                   " Neighbour, Cafe");
}

static void
test_search (void)
{
  NwNetwork cafe = network (NW_NETWORK_WIFI, "Café Central", 40);

  g_assert_true (nw_network_matches (&cafe, ""));
  g_assert_true (nw_network_matches (&cafe, NULL));
  g_assert_true (nw_network_matches (&cafe, "central"));
  g_assert_true (nw_network_matches (&cafe, "CAFÉ"));
  g_assert_true (nw_network_matches (&cafe, "é c"));
  g_assert_false (nw_network_matches (&cafe, "home"));
}

static void
assert_bar (const NwNetwork *network, NwBar bar, const char *text)
{
  g_autofree char *shown = NULL;

  g_assert_cmpint (nw_state_bar (network, &shown), ==, bar);
  g_assert_cmpstr (shown, ==, text);
}

static void
test_bar (void)
{
  NwNetwork wifi = network (NW_NETWORK_WIFI, "Cafe", 40);
  NwNetwork vpn = network (NW_NETWORK_VPN, "Work", -1);
  NwNetwork wired = network (NW_NETWORK_WIRED, "Dock", -1);

  assert_bar (&wifi, NW_BAR_JOIN, "Join Cafe");
  wifi.security = NW_SECURITY_PSK;
  assert_bar (&wifi, NW_BAR_PASSWORD, "Join");
  wifi.security = NW_SECURITY_WEP;
  assert_bar (&wifi, NW_BAR_PASSWORD, "Join");
  /* nm-connection-editor's, in a window of its own. */
  wifi.security = NW_SECURITY_ENTERPRISE;
  assert_bar (&wifi, NW_BAR_EDITOR, "Join Cafe…");
  /* Saved, its secret is NetworkManager's or the agent's. */
  wifi.saved = TRUE;
  assert_bar (&wifi, NW_BAR_JOIN, "Join Cafe");
  wifi.security = NW_SECURITY_PSK;
  assert_bar (&wifi, NW_BAR_JOIN, "Join Cafe");
  wifi.connecting = TRUE;
  assert_bar (&wifi, NW_BAR_CONNECTING, "Connecting to Cafe…");
  wifi.connecting = FALSE;
  wifi.active = TRUE;
  assert_bar (&wifi, NW_BAR_NONE, NULL);
  vpn.saved = wired.saved = TRUE;
  assert_bar (&vpn, NW_BAR_JOIN, "Connect Work");
  assert_bar (&wired, NW_BAR_JOIN, "Join Dock");
}

static void
test_network_icons (void)
{
  NwNetwork wifi = network (NW_NETWORK_WIFI, "Cafe", 40);
  NwNetwork vpn = network (NW_NETWORK_VPN, "Work", -1);
  NwNetwork wired = network (NW_NETWORK_WIRED, "Dock", -1);

  g_assert_cmpstr (nw_network_icon_names (&wifi)[0], ==, "network-wireless-signal-ok");
  g_assert_cmpstr (nw_network_icon_names (&vpn)[0], ==, "network-vpn");
  g_assert_cmpstr (nw_network_icon_names (&wired)[0], ==, "network-wired");
}

static void
test_password_valid (void)
{
  NwNetwork wifi = network (NW_NETWORK_WIFI, "Cafe", 40);
  g_autofree char *hex = g_strnfill (64, 'a');
  g_autofree char *long_text = g_strnfill (64, 'z');

  wifi.security = NW_SECURITY_PSK;
  g_assert_false (nw_state_password_valid (&wifi, ""));
  /* WPA's: 8 to 63 characters, or 64 hex digits. */
  g_assert_false (nw_state_password_valid (&wifi, "1234567"));
  g_assert_true (nw_state_password_valid (&wifi, "12345678"));
  g_assert_true (nw_state_password_valid (&wifi, hex));
  g_assert_false (nw_state_password_valid (&wifi, long_text));
  /* WEP's key or passphrase. */
  wifi.security = NW_SECURITY_WEP;
  g_assert_false (nw_state_password_valid (&wifi, ""));
  g_assert_true (nw_state_password_valid (&wifi, "abcde"));
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/state/without-networkmanager-it-says-so", test_not_running);
  g_test_add_func ("/state/with-nothing-up-it-is-offline", test_not_connected);
  g_test_add_func ("/state/airplane-mode-with-nothing-up", test_airplane_mode);
  g_test_add_func ("/state/a-connection-wins-over-airplane-mode", test_wired_wins_over_airplane_mode);
  g_test_add_func ("/state/wi-fi-shows-its-signal-in-four-steps", test_wifi_steps);
  g_test_add_func ("/state/wi-fi-tooltip-has-the-signal-in-percent", test_wifi_tooltip);
  g_test_add_func ("/state/limited-connectivity-is-no-route-and-no-internet", test_limited);
  g_test_add_func ("/state/a-connection-coming-up-is-acquiring", test_connecting);
  g_test_add_func ("/state/a-vpn-is-on-the-second-line", test_vpn);
  g_test_add_func ("/state/statuses-compare", test_equal);
  g_test_add_func ("/state/the-wi-fi-tile-is-wireless-enabled", test_wifi_tile);
  g_test_add_func ("/state/the-wired-tile-follows-the-cable", test_wired_tile);
  g_test_add_func ("/state/the-vpn-tile-is-the-vpn-used-last", test_vpn_tile);
  g_test_add_func ("/state/the-airplane-mode-tile-is-on-while-every-radio-is-off", test_airplane_tile);
  g_test_add_func ("/state/the-card-shows-the-connection-up", test_card);
  g_test_add_func ("/state/the-card-without-a-connection-up", test_card_without_a_connection);
  g_test_add_func ("/state/networks-are-active-then-vpns-then-saved-then-available", test_order);
  g_test_add_func ("/state/the-search-finds-part-of-a-name-in-any-case", test_search);
  g_test_add_func ("/state/the-bar-joins-the-network-selected", test_bar);
  g_test_add_func ("/state/a-network-has-its-kind-s-icon", test_network_icons);
  g_test_add_func ("/state/a-password-is-one-networkmanager-takes", test_password_valid);
  return g_test_run ();
}
