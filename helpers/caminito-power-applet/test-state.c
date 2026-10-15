/*
 * The icon's names, the percentage and the tooltip (262), and the table of batteries (263),
 * from displays and devices built by hand.
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

#define BAT0   "/org/freedesktop/UPower/devices/battery_BAT0"
#define BAT1   "/org/freedesktop/UPower/devices/battery_BAT1"
#define MOUSE  "/org/freedesktop/UPower/devices/mouse_hidpp_battery_0"
#define HEADS  "/org/freedesktop/UPower/devices/headphones_dev_AA"
#define LINE   "/org/freedesktop/UPower/devices/line_power_AC"

static PwDevice *
device_new (const char *path, guint type, gboolean supply, PwState state, double percentage,
            const char *model)
{
  PwDevice *device = pw_device_new ();

  device->path = g_strdup (path);
  device->type = type;
  device->power_supply = supply;
  device->present = TRUE;
  device->state = state;
  device->percentage = percentage;
  device->has_percentage = TRUE;
  device->model = g_strdup (model);
  return device;
}

static GPtrArray *
devices_new (void)
{
  return g_ptr_array_new_with_free_func ((GDestroyNotify) pw_device_free);
}

static const PwRow *
row_at (GPtrArray *rows, guint index)
{
  g_assert_cmpuint (index, <, rows->len);
  return g_ptr_array_index (rows, index);
}

/* A laptop's batteries come first, in the order of their paths, whatever order UPower
   lists them in, and then the peripherals by name. */
static void
test_rows_order (void)
{
  g_autoptr (GPtrArray) devices = devices_new ();
  g_autoptr (GPtrArray) rows = NULL;

  g_ptr_array_add (devices, device_new (MOUSE, PW_TYPE_MOUSE, FALSE, PW_STATE_DISCHARGING, 55, "MX Master 3S"));
  g_ptr_array_add (devices, device_new (BAT1, PW_TYPE_BATTERY, TRUE, PW_STATE_CHARGING, 80, "SECOND"));
  g_ptr_array_add (devices, device_new (HEADS, PW_TYPE_HEADPHONES, FALSE, PW_STATE_DISCHARGING, 18, "WH-1000XM4"));
  g_ptr_array_add (devices, device_new (BAT0, PW_TYPE_BATTERY, TRUE, PW_STATE_DISCHARGING, 73, "FIRST"));
  rows = pw_state_rows (devices);

  g_assert_cmpuint (rows->len, ==, 4);
  g_assert_cmpstr (row_at (rows, 0)->path, ==, BAT0);
  g_assert_cmpstr (row_at (rows, 0)->name, ==, "Battery 1");
  g_assert_true (row_at (rows, 0)->laptop);
  g_assert_cmpstr (row_at (rows, 1)->path, ==, BAT1);
  g_assert_cmpstr (row_at (rows, 1)->name, ==, "Battery 2");
  g_assert_cmpstr (row_at (rows, 2)->name, ==, "MX Master 3S");
  g_assert_false (row_at (rows, 2)->laptop);
  g_assert_cmpstr (row_at (rows, 3)->name, ==, "WH-1000XM4");
}

static void
test_rows_one_battery (void)
{
  g_autoptr (GPtrArray) devices = devices_new ();
  g_autoptr (GPtrArray) rows = NULL;
  const PwRow *row;

  g_ptr_array_add (devices, device_new (BAT0, PW_TYPE_BATTERY, TRUE, PW_STATE_DISCHARGING, 72.6, "DELL 5XKWK"));
  rows = pw_state_rows (devices);
  row = row_at (rows, 0);

  /* UPower's model names tell nothing: "Battery". */
  g_assert_cmpstr (row->name, ==, "Battery");
  g_assert_cmpstr (row->state, ==, "Discharging");
  g_assert_cmpstr (row->percentage, ==, "73%");
  g_assert_cmpfloat_with_epsilon (row->fraction, 0.73, 0.0001);
  g_assert_false (row->low);
  g_assert_cmpstr (row->icon_names[0], ==, "battery-good");
  g_assert_cmpstr (row->icon_names[1], ==, "battery-level-70-symbolic");
}

static void
test_rows_states (void)
{
  struct { PwState state; const char *text; const char *icon; } cases[] = {
    { PW_STATE_CHARGING, "Charging", "battery-good-charging" },
    { PW_STATE_DISCHARGING, "Discharging", "battery-good" },
    { PW_STATE_PENDING_CHARGE, "Not charging", "battery-good" },
    { PW_STATE_PENDING_DISCHARGE, "Not charging", "battery-good" },
    { PW_STATE_FULLY_CHARGED, "Fully charged", "battery-full-charged" },
    { PW_STATE_EMPTY, "Empty", "battery-good" },
    { PW_STATE_UNKNOWN, "Unknown", "battery-good" },
  };

  for (guint i = 0; i < G_N_ELEMENTS (cases); i++)
    {
      g_autoptr (GPtrArray) devices = devices_new ();
      g_autoptr (GPtrArray) rows = NULL;

      g_ptr_array_add (devices, device_new (BAT0, PW_TYPE_BATTERY, TRUE, cases[i].state, 60, NULL));
      rows = pw_state_rows (devices);
      g_assert_cmpstr (row_at (rows, 0)->state, ==, cases[i].text);
      g_assert_cmpstr (row_at (rows, 0)->icon_names[0], ==, cases[i].icon);
    }
}

static void
test_rows_limit (void)
{
  g_autoptr (GPtrArray) devices = devices_new ();
  g_autoptr (GPtrArray) rows = NULL;
  PwDevice *device = device_new (BAT0, PW_TYPE_BATTERY, TRUE, PW_STATE_PENDING_CHARGE, 80, NULL);

  device->capacity = 57.6;
  device->threshold_supported = TRUE;
  device->threshold_enabled = TRUE;
  device->threshold_end = 80;
  g_ptr_array_add (devices, device);
  rows = pw_state_rows (devices);

  /* The limit is in the state's line when it's on. */
  g_assert_cmpstr (row_at (rows, 0)->state, ==, "Not charging · limit 80%");
  g_assert_cmpstr (row_at (rows, 0)->health, ==, "Health 58%");
  g_assert_true (row_at (rows, 0)->limit_supported);
  g_assert_true (row_at (rows, 0)->limit_enabled);
  g_assert_cmpuint (row_at (rows, 0)->limit_end, ==, 80);

  /* Off: UPower reports the end of the scale, which limits nothing, and the switch offers
     the usual 80%. */
  device->threshold_enabled = FALSE;
  device->threshold_end = 100;
  g_clear_pointer (&rows, g_ptr_array_unref);
  rows = pw_state_rows (devices);
  g_assert_cmpstr (row_at (rows, 0)->state, ==, "Not charging");
  g_assert_true (row_at (rows, 0)->limit_supported);
  g_assert_false (row_at (rows, 0)->limit_enabled);
  g_assert_cmpuint (row_at (rows, 0)->limit_end, ==, 80);

  /* An end threshold that UPower keeps while the limit is off is the one offered. */
  device->threshold_end = 60;
  g_clear_pointer (&rows, g_ptr_array_unref);
  rows = pw_state_rows (devices);
  g_assert_cmpuint (row_at (rows, 0)->limit_end, ==, 60);

  /* Unsupported: no switch, whatever else UPower says, and no limit in the line. */
  device->threshold_supported = FALSE;
  device->threshold_enabled = TRUE;
  device->capacity = 0;
  g_clear_pointer (&rows, g_ptr_array_unref);
  rows = pw_state_rows (devices);
  g_assert_false (row_at (rows, 0)->limit_supported);
  g_assert_false (row_at (rows, 0)->limit_enabled);
  g_assert_cmpstr (row_at (rows, 0)->state, ==, "Not charging");
  g_assert_cmpstr (row_at (rows, 0)->health, ==, "Health unknown");
}

static void
test_rows_peripherals (void)
{
  g_autoptr (GPtrArray) devices = devices_new ();
  g_autoptr (GPtrArray) rows = NULL;
  PwDevice *nameless = device_new (HEADS, PW_TYPE_KEYBOARD, FALSE, PW_STATE_UNKNOWN, 42, "");

  g_ptr_array_add (devices, device_new (MOUSE, PW_TYPE_MOUSE, FALSE, PW_STATE_DISCHARGING, 19.4, "MX"));
  g_ptr_array_add (devices, nameless);
  rows = pw_state_rows (devices);

  /* No model: the kind. No state: the kind again, in place of "Unknown". */
  g_assert_cmpstr (row_at (rows, 0)->name, ==, "Keyboard");
  g_assert_cmpstr (row_at (rows, 0)->state, ==, "Keyboard");
  g_assert_cmpstr (row_at (rows, 0)->icon_names[0], ==, "input-keyboard-symbolic");
  g_assert_false (row_at (rows, 0)->low);
  g_assert_false (row_at (rows, 0)->limit_supported);
  g_assert_cmpstr (row_at (rows, 1)->icon_names[0], ==, "input-mouse-symbolic");
  g_assert_cmpstr (row_at (rows, 1)->icon_names[1], ==, "input-mouse");
  g_assert_cmpstr (row_at (rows, 1)->percentage, ==, "19%");
  /* Below 20%, as shown: 19.4% shows as 19%. */
  g_assert_true (row_at (rows, 1)->low);
}

static void
test_rows_low_is_the_shown_percentage (void)
{
  g_autoptr (GPtrArray) devices = devices_new ();
  g_autoptr (GPtrArray) rows = NULL;

  /* 19.6% shows as 20%, which isn't below 20%. */
  g_ptr_array_add (devices, device_new (MOUSE, PW_TYPE_MOUSE, FALSE, PW_STATE_DISCHARGING, 19.6, "MX"));
  rows = pw_state_rows (devices);
  g_assert_cmpstr (row_at (rows, 0)->percentage, ==, "20%");
  g_assert_false (row_at (rows, 0)->low);
}

static void
test_rows_leave_out (void)
{
  g_autoptr (GPtrArray) devices = devices_new ();
  g_autoptr (GPtrArray) rows = NULL;
  PwDevice *gone = device_new (MOUSE, PW_TYPE_MOUSE, FALSE, PW_STATE_DISCHARGING, 55, "Off");
  PwDevice *levels = device_new (HEADS, PW_TYPE_HEADSET, FALSE, PW_STATE_DISCHARGING, 0, "Levels only");

  gone->present = FALSE;
  levels->has_percentage = FALSE;
  g_ptr_array_add (devices, device_new (LINE, PW_TYPE_LINE_POWER, TRUE, PW_STATE_UNKNOWN, 0, NULL));
  g_ptr_array_add (devices, gone);
  g_ptr_array_add (devices, levels);
  rows = pw_state_rows (devices);

  /* The mains, a mouse that's turned off and a device with no percentage: no rows, and so
     no table. */
  g_assert_cmpuint (rows->len, ==, 0);
}

static void
test_rows_battery_that_is_no_laptops (void)
{
  g_autoptr (GPtrArray) devices = devices_new ();
  g_autoptr (GPtrArray) rows = NULL;

  /* A battery that doesn't power the computer, a gamepad's, is a peripheral: no ⋮. */
  g_ptr_array_add (devices, device_new (BAT0, PW_TYPE_BATTERY, FALSE, PW_STATE_DISCHARGING, 40, "Pad"));
  rows = pw_state_rows (devices);
  g_assert_cmpuint (rows->len, ==, 1);
  g_assert_false (row_at (rows, 0)->laptop);
  g_assert_cmpstr (row_at (rows, 0)->name, ==, "Pad");
  g_assert_cmpstr (row_at (rows, 0)->icon_names[0], ==, "battery-symbolic");
}

static void
test_rows_equal (void)
{
  g_autoptr (GPtrArray) devices = devices_new ();
  g_autoptr (GPtrArray) a = NULL;
  g_autoptr (GPtrArray) b = NULL;
  PwDevice *device = device_new (BAT0, PW_TYPE_BATTERY, TRUE, PW_STATE_DISCHARGING, 70, NULL);

  g_ptr_array_add (devices, device);
  a = pw_state_rows (devices);
  /* What the table doesn't show doesn't make it different. */
  b = pw_state_rows (devices);
  g_assert_true (pw_row_equal (row_at (a, 0), row_at (b, 0)));
  device->percentage = 69;
  g_clear_pointer (&b, g_ptr_array_unref);
  b = pw_state_rows (devices);
  g_assert_false (pw_row_equal (row_at (a, 0), row_at (b, 0)));
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
  g_test_add_func ("/rows/laptop-batteries-come-first-in-the-order-of-their-paths", test_rows_order);
  g_test_add_func ("/rows/one-battery-is-just-battery", test_rows_one_battery);
  g_test_add_func ("/rows/the-four-states-in-words", test_rows_states);
  g_test_add_func ("/rows/the-limit-health-and-switch", test_rows_limit);
  g_test_add_func ("/rows/peripherals-have-their-icon-and-a-name", test_rows_peripherals);
  g_test_add_func ("/rows/low-is-below-20-as-shown", test_rows_low_is_the_shown_percentage);
  g_test_add_func ("/rows/the-mains-and-what-is-off-are-left-out", test_rows_leave_out);
  g_test_add_func ("/rows/a-battery-that-powers-nothing-is-a-peripheral", test_rows_battery_that_is_no_laptops);
  g_test_add_func ("/rows/equal-rows-need-no-redraw", test_rows_equal);
  return g_test_run ();
}
