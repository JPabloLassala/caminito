/*
 * The icon's names, the percentage and the tooltip (262), from displays built by hand.
 */

#include "pw-state.h"

static PwDisplay
battery (PwState state, double percentage, gint64 to_empty, gint64 to_full)
{
  return (PwDisplay) {
    .present = TRUE,
    .state = state,
    .percentage = percentage,
    .time_to_empty = to_empty,
    .time_to_full = to_full,
  };
}

static void
test_duration (void)
{
  struct { gint64 seconds; const char *text; } cases[] = {
    { 0, "1 min" },
    { 20, "1 min" },
    { 90, "2 min" },
    { 50 * 60, "50 min" },
    { 3600, "1 h" },
    { 3 * 3600 + 40 * 60, "3 h 40 min" },
    { 3 * 3600 + 40 * 60 + 29, "3 h 40 min" },
    { 3 * 3600 + 59 * 60 + 40, "4 h" },
  };

  for (guint i = 0; i < G_N_ELEMENTS (cases); i++)
    {
      g_autofree char *text = pw_state_duration (cases[i].seconds);

      g_assert_cmpstr (text, ==, cases[i].text);
    }
}

static void
test_discharging (void)
{
  PwDisplay display = battery (PW_STATE_DISCHARGING, 78.4, 3 * 3600 + 40 * 60, 0);
  const char *const *names = pw_state_icon_names (&display);
  g_autofree char *tooltip = pw_state_tooltip (&display);
  g_autofree char *percentage = pw_state_percentage (&display);

  g_assert_cmpint (pw_state_icon_kind (&display), ==, PW_ICON_BATTERY);
  /* MATE's name, then Adwaita's in tens, then Breeze's. */
  g_assert_cmpstr (names[0], ==, "battery-full");
  g_assert_cmpstr (names[1], ==, "battery-level-80-symbolic");
  g_assert_cmpstr (names[2], ==, "battery-080");
  g_assert_null (names[3]);
  g_assert_cmpstr (percentage, ==, "78%");
  g_assert_cmpstr (tooltip, ==, "78%\n3 h 40 min left");
}

static void
test_discharging_unknown_time (void)
{
  PwDisplay display = battery (PW_STATE_DISCHARGING, 50, 0, 0);
  g_autofree char *tooltip = pw_state_tooltip (&display);

  g_assert_cmpstr (tooltip, ==, "50%\nOn battery");
}

static void
test_levels (void)
{
  /* The percentage rounds to the nearest ten, and MATE's five names follow those. */
  struct { double percentage; const char *mate; const char *adwaita; } cases[] = {
    { 100, "battery-full", "battery-level-100-symbolic" },
    { 80, "battery-full", "battery-level-80-symbolic" },
    { 75, "battery-full", "battery-level-80-symbolic" },
    { 74.9, "battery-good", "battery-level-70-symbolic" },
    { 50, "battery-good", "battery-level-50-symbolic" },
    { 44, "battery-low", "battery-level-40-symbolic" },
    { 20, "battery-low", "battery-level-20-symbolic" },
    { 14, "battery-caution", "battery-level-10-symbolic" },
    { 5, "battery-caution", "battery-level-10-symbolic" },
    { 4, "battery-empty", "battery-level-0-symbolic" },
    { 0, "battery-empty", "battery-level-0-symbolic" },
  };

  for (guint i = 0; i < G_N_ELEMENTS (cases); i++)
    {
      PwDisplay display = battery (PW_STATE_DISCHARGING, cases[i].percentage, 100, 0);
      const char *const *names = pw_state_icon_names (&display);

      g_assert_cmpstr (names[0], ==, cases[i].mate);
      g_assert_cmpstr (names[1], ==, cases[i].adwaita);
    }
}

static void
test_charging (void)
{
  PwDisplay display = battery (PW_STATE_CHARGING, 42, 0, 50 * 60);
  const char *const *names = pw_state_icon_names (&display);
  g_autofree char *tooltip = pw_state_tooltip (&display);

  g_assert_cmpint (pw_state_icon_kind (&display), ==, PW_ICON_CHARGING);
  g_assert_cmpstr (names[0], ==, "battery-low-charging");
  g_assert_cmpstr (names[1], ==, "battery-level-40-charging-symbolic");
  g_assert_cmpstr (names[2], ==, "battery-040-charging");
  g_assert_cmpstr (tooltip, ==, "42%\nFull in 50 min");

  display.time_to_full = 0;
  g_free (tooltip);
  tooltip = pw_state_tooltip (&display);
  g_assert_cmpstr (tooltip, ==, "42%\nCharging");
}

static void
test_full (void)
{
  PwDisplay display = battery (PW_STATE_FULLY_CHARGED, 100, 0, 0);
  const char *const *names = pw_state_icon_names (&display);
  g_autofree char *tooltip = pw_state_tooltip (&display);

  g_assert_cmpint (pw_state_icon_kind (&display), ==, PW_ICON_FULL);
  g_assert_cmpstr (names[0], ==, "battery-full-charged");
  g_assert_cmpstr (names[1], ==, "battery-level-100-charged-symbolic");
  g_assert_cmpstr (names[2], ==, "battery-100");
  g_assert_cmpstr (tooltip, ==, "100%\nPlugged in");

  /* A battery held at a charge limit is on the charger and isn't charging. */
  display = battery (PW_STATE_PENDING_CHARGE, 80, 0, 0);
  g_free (tooltip);
  tooltip = pw_state_tooltip (&display);
  g_assert_cmpstr (tooltip, ==, "80%\nPlugged in");
}

static void
test_no_battery (void)
{
  PwDisplay display = { 0 };
  const char *const *names = pw_state_icon_names (&display);
  g_autofree char *tooltip = pw_state_tooltip (&display);
  g_autofree char *percentage = pw_state_percentage (&display);

  g_assert_cmpint (pw_state_icon_kind (&display), ==, PW_ICON_PLUG);
  g_assert_cmpstr (names[0], ==, "ac-adapter");
  g_assert_cmpstr (names[1], ==, "ac-adapter-symbolic");
  g_assert_null (names[2]);
  g_assert_null (percentage);
  g_assert_cmpstr (tooltip, ==, "Plugged in");
  /* Whatever else UPower says about a battery that isn't there. */
  display.state = PW_STATE_DISCHARGING;
  display.percentage = 50;
  g_free (tooltip);
  tooltip = pw_state_tooltip (&display);
  g_assert_cmpstr (tooltip, ==, "Plugged in");
}

static void
test_empty (void)
{
  PwDisplay display = battery (PW_STATE_EMPTY, 0, 0, 0);
  g_autofree char *tooltip = pw_state_tooltip (&display);

  g_assert_cmpstr (tooltip, ==, "0%\nEmpty");
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/state/a-duration-rounds-to-the-nearest-minute", test_duration);
  g_test_add_func ("/state/discharging-shows-the-time-left", test_discharging);
  g_test_add_func ("/state/discharging-without-a-time-says-on-battery", test_discharging_unknown_time);
  g_test_add_func ("/state/the-glyph-follows-the-percentage-in-steps-of-ten", test_levels);
  g_test_add_func ("/state/charging-shows-the-time-to-full", test_charging);
  g_test_add_func ("/state/full-and-held-at-a-limit-say-plugged-in", test_full);
  g_test_add_func ("/state/no-battery-is-a-plug", test_no_battery);
  g_test_add_func ("/state/empty-says-so", test_empty);
  return g_test_run ();
}
