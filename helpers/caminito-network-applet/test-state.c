/*
 * The icon's names, the lock and the tooltip (CMN-193), from statuses built by hand.
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
  nw_status_clear (&other);
  g_assert_null (other.name);
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
  return g_test_run ();
}
