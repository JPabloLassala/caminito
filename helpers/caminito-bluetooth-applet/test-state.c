/*
 * The icon's state and the tooltip (259), from snapshots built by hand.
 */

#include "bt-state.h"

#define HCI0 "/org/bluez/hci0"
#define HCI1 "/org/bluez/hci1"

/* A paired device with no icon, which is what most of these cases need. */
static void
add_device (BtSnapshot *snapshot, const char *path, const char *adapter, const char *alias,
            gboolean connected, int battery)
{
  bt_snapshot_add_device (snapshot, path, adapter, alias, "00:11:22:33:44:55", NULL, TRUE,
                          connected, battery);
}

static void
test_no_adapter (void)
{
  g_autoptr (BtSnapshot) snapshot = bt_snapshot_new ();

  g_assert_cmpint (bt_state_icon (snapshot), ==, BT_ICON_NONE);
  g_assert_null (bt_state_icon_names (BT_ICON_NONE));
}

static void
test_off (void)
{
  g_autoptr (BtSnapshot) snapshot = bt_snapshot_new ();
  g_autofree char *tooltip = NULL;

  bt_snapshot_add_adapter (snapshot, HCI0, FALSE);
  /* BlueZ keeps a device's Connected while the adapter goes down only for a moment, and
     the icon mustn't say connected through it. */
  add_device (snapshot, HCI0 "/dev_A", HCI0, "Headphones", TRUE, 80);
  g_assert_cmpint (bt_state_icon (snapshot), ==, BT_ICON_OFF);
  tooltip = bt_state_tooltip (snapshot);
  g_assert_cmpstr (tooltip, ==, "Bluetooth is off");
}

static void
test_on (void)
{
  g_autoptr (BtSnapshot) snapshot = bt_snapshot_new ();
  g_autofree char *tooltip = NULL;

  bt_snapshot_add_adapter (snapshot, HCI0, TRUE);
  add_device (snapshot, HCI0 "/dev_A", HCI0, "Headphones", FALSE, 80);
  g_assert_cmpint (bt_state_icon (snapshot), ==, BT_ICON_ON);
  tooltip = bt_state_tooltip (snapshot);
  g_assert_cmpstr (tooltip, ==, "Bluetooth is on\nNo devices connected");
}

static void
test_connected (void)
{
  g_autoptr (BtSnapshot) snapshot = bt_snapshot_new ();
  g_autofree char *tooltip = NULL;

  bt_snapshot_add_adapter (snapshot, HCI0, TRUE);
  add_device (snapshot, HCI0 "/dev_B", HCI0, "Mouse", TRUE, -1);
  add_device (snapshot, HCI0 "/dev_A", HCI0, "Headphones", TRUE, 80);
  add_device (snapshot, HCI0 "/dev_C", HCI0, "Phone", FALSE, 50);
  g_assert_cmpint (bt_state_icon (snapshot), ==, BT_ICON_CONNECTED);
  tooltip = bt_state_tooltip (snapshot);
  g_assert_cmpstr (tooltip, ==, "Bluetooth is on\nHeadphones: 80%\nMouse");
}

/* The applet treats every adapter as one (the spec's "More than one adapter"). */
static void
test_two_adapters (void)
{
  g_autoptr (BtSnapshot) snapshot = bt_snapshot_new ();
  g_autofree char *tooltip = NULL;

  bt_snapshot_add_adapter (snapshot, HCI0, FALSE);
  bt_snapshot_add_adapter (snapshot, HCI1, TRUE);
  add_device (snapshot, HCI0 "/dev_A", HCI0, "Headphones", TRUE, 80);
  add_device (snapshot, HCI1 "/dev_B", HCI1, "Keyboard", TRUE, 5);
  g_assert_cmpint (bt_state_icon (snapshot), ==, BT_ICON_CONNECTED);
  tooltip = bt_state_tooltip (snapshot);
  g_assert_cmpstr (tooltip, ==, "Bluetooth is on\nKeyboard: 5%");
}

/* A device whose adapter BlueZ hasn't announced yet counts as on no powered adapter. */
static void
test_unknown_adapter (void)
{
  g_autoptr (BtSnapshot) snapshot = bt_snapshot_new ();

  bt_snapshot_add_adapter (snapshot, HCI0, TRUE);
  add_device (snapshot, HCI1 "/dev_A", HCI1, "Headphones", TRUE, -1);
  g_assert_cmpint (bt_state_icon (snapshot), ==, BT_ICON_ON);
}

static gboolean
names_have (const char *const *names, const char *name)
{
  return names != NULL && g_strv_contains (names, name);
}

static void
test_icon_names (void)
{
  const char *const *off = bt_state_icon_names (BT_ICON_OFF);
  const char *const *on = bt_state_icon_names (BT_ICON_ON);
  const char *const *connected = bt_state_icon_names (BT_ICON_CONNECTED);

  /* MATE's and Adwaita's first, Breeze's last. */
  g_assert_cmpstr (off[0], ==, "bluetooth-disabled");
  g_assert_true (names_have (off, "bluetooth-disabled-symbolic"));
  g_assert_cmpstr (off[g_strv_length ((char **) off) - 1], ==, "network-bluetooth-inactive");
  g_assert_cmpstr (on[0], ==, "bluetooth-active");
  g_assert_true (names_have (on, "bluetooth-active-symbolic"));
  g_assert_cmpstr (on[g_strv_length ((char **) on) - 1], ==, "network-bluetooth");
  g_assert_cmpstr (connected[0], ==, "bluetooth-paired");
  g_assert_cmpstr (connected[g_strv_length ((char **) connected) - 1], ==,
                   "network-bluetooth-activated");
}

static char *
alias_list (GPtrArray *devices)
{
  GString *list = g_string_new (NULL);

  for (guint i = 0; i < devices->len; i++)
    g_string_append_printf (list, "%s%s", i > 0 ? "," : "",
                            ((const BtDevice *) g_ptr_array_index (devices, i))->alias);
  return g_string_free (list, FALSE);
}

/* The popup's two sections (260): paired devices only, split by their connection. */
static void
test_listed_sections (void)
{
  g_autoptr (BtSnapshot) snapshot = bt_snapshot_new ();
  g_autoptr (GPtrArray) connected = NULL;
  g_autoptr (GPtrArray) available = NULL;
  g_autofree char *connected_names = NULL;
  g_autofree char *available_names = NULL;

  bt_snapshot_add_adapter (snapshot, HCI0, TRUE);
  bt_snapshot_add_device (snapshot, HCI0 "/dev_A", HCI0, "Headphones", "11:22:33:44:55:66",
                          "audio-headphones", TRUE, TRUE, 80);
  bt_snapshot_add_device (snapshot, HCI0 "/dev_B", HCI0, "Phone", "11:22:33:44:55:67",
                          "phone", TRUE, FALSE, -1);
  /* Nearby and unpaired, which BlueZ lists too: Add New Device's, not the popup's. */
  bt_snapshot_add_device (snapshot, HCI0 "/dev_C", HCI0, "TV", "11:22:33:44:55:68", NULL,
                          FALSE, FALSE, -1);
  connected = bt_state_listed (snapshot, TRUE);
  available = bt_state_listed (snapshot, FALSE);
  connected_names = alias_list (connected);
  available_names = alias_list (available);
  g_assert_cmpstr (connected_names, ==, "Headphones");
  g_assert_cmpstr (available_names, ==, "Phone");
}

/* The adapters are merged, and sorted by name, not by the adapter's path. */
static void
test_listed_merges_adapters_by_name (void)
{
  g_autoptr (BtSnapshot) snapshot = bt_snapshot_new ();
  g_autoptr (GPtrArray) available = NULL;
  g_autofree char *available_names = NULL;

  bt_snapshot_add_adapter (snapshot, HCI0, TRUE);
  bt_snapshot_add_adapter (snapshot, HCI1, TRUE);
  add_device (snapshot, HCI0 "/dev_A", HCI0, "mouse", FALSE, -1);
  add_device (snapshot, HCI1 "/dev_B", HCI1, "Keyboard", FALSE, -1);
  add_device (snapshot, HCI0 "/dev_C", HCI0, "Headphones", FALSE, -1);
  available = bt_state_listed (snapshot, FALSE);
  available_names = alias_list (available);
  g_assert_cmpstr (available_names, ==, "Headphones,Keyboard,mouse");
}

/* Nothing on an adapter that's off can be connected, so nothing of it is listed. */
static void
test_listed_leaves_out_devices_of_an_unpowered_adapter (void)
{
  g_autoptr (BtSnapshot) snapshot = bt_snapshot_new ();
  g_autoptr (GPtrArray) available = NULL;
  g_autoptr (GPtrArray) connected = NULL;

  bt_snapshot_add_adapter (snapshot, HCI0, TRUE);
  bt_snapshot_add_adapter (snapshot, HCI1, FALSE);
  add_device (snapshot, HCI0 "/dev_A", HCI0, "Headphones", TRUE, -1);
  add_device (snapshot, HCI1 "/dev_B", HCI1, "Keyboard", FALSE, -1);
  add_device (snapshot, HCI1 "/dev_C", HCI1, "Mouse", TRUE, -1);
  connected = bt_state_listed (snapshot, TRUE);
  available = bt_state_listed (snapshot, FALSE);
  g_assert_cmpuint (connected->len, ==, 1);
  g_assert_cmpuint (available->len, ==, 0);
}

static void
test_battery_low (void)
{
  /* Red below 20%, and not for a device with no Battery1. */
  g_assert_false (bt_state_battery_low (-1));
  g_assert_true (bt_state_battery_low (0));
  g_assert_true (bt_state_battery_low (19));
  g_assert_false (bt_state_battery_low (20));
  g_assert_false (bt_state_battery_low (100));
}

static void
test_device_kind (void)
{
  /* BlueZ's Device1.Icon, from the device's class, names the type for the details line. */
  g_assert_cmpstr (bt_state_device_kind ("audio-headphones"), ==, "Headphones");
  g_assert_cmpstr (bt_state_device_kind ("audio-headset"), ==, "Headset");
  g_assert_cmpstr (bt_state_device_kind ("audio-card"), ==, "Audio device");
  g_assert_cmpstr (bt_state_device_kind ("input-keyboard"), ==, "Keyboard");
  g_assert_cmpstr (bt_state_device_kind ("input-mouse"), ==, "Mouse");
  g_assert_cmpstr (bt_state_device_kind ("input-gaming"), ==, "Game controller");
  g_assert_cmpstr (bt_state_device_kind ("input-tablet"), ==, "Tablet");
  g_assert_cmpstr (bt_state_device_kind ("phone"), ==, "Phone");
  g_assert_cmpstr (bt_state_device_kind ("computer"), ==, "Computer");
  g_assert_cmpstr (bt_state_device_kind ("camera-photo"), ==, "Camera");
  g_assert_cmpstr (bt_state_device_kind ("printer"), ==, "Printer");
  g_assert_cmpstr (bt_state_device_kind ("video-display"), ==, "Display");
  /* A class BlueZ has no icon for, and a name this table has never heard of. */
  g_assert_cmpstr (bt_state_device_kind (NULL), ==, "Device");
  g_assert_cmpstr (bt_state_device_kind ("something-new"), ==, "Device");
}

static void
test_icon_for_a_device (void)
{
  g_auto (GStrv) headphones = bt_state_device_icon_names ("audio-headphones");
  g_auto (GStrv) none = bt_state_device_icon_names (NULL);

  /* The device's own icon first, symbolic or not as the theme has it, then a generic one
     so that a row is never blank. */
  g_assert_cmpstr (headphones[0], ==, "audio-headphones");
  g_assert_true (g_strv_contains ((const char *const *) headphones, "audio-headphones-symbolic"));
  g_assert_cmpstr (headphones[g_strv_length (headphones) - 1], ==,
                   "bluetooth-active-symbolic");
  g_assert_cmpstr (none[0], ==, "bluetooth-active-symbolic");
}

static void
test_copy (void)
{
  g_autoptr (BtSnapshot) snapshot = bt_snapshot_new ();
  g_autoptr (BtSnapshot) copy = NULL;
  const BtDevice *device;

  bt_snapshot_add_adapter (snapshot, HCI0, TRUE);
  bt_snapshot_add_device (snapshot, HCI0 "/dev_A", HCI0, "Headphones", "11:22:33:44:55:66",
                          "audio-headphones", TRUE, TRUE, 80);
  copy = bt_snapshot_copy (snapshot);
  g_clear_pointer (&snapshot, bt_snapshot_free);
  g_assert_cmpuint (copy->adapters->len, ==, 1);
  g_assert_cmpuint (copy->devices->len, ==, 1);
  device = g_ptr_array_index (copy->devices, 0);
  g_assert_cmpstr (device->alias, ==, "Headphones");
  g_assert_cmpstr (device->address, ==, "11:22:33:44:55:66");
  g_assert_cmpstr (device->icon, ==, "audio-headphones");
  g_assert_true (device->paired);
  g_assert_true (device->connected);
  g_assert_cmpint (device->battery, ==, 80);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/state/no-adapter-shows-no-icon", test_no_adapter);
  g_test_add_func ("/state/no-powered-adapter-is-off-even-with-a-connected-device", test_off);
  g_test_add_func ("/state/a-powered-adapter-with-nothing-connected-is-on", test_on);
  g_test_add_func ("/state/the-tooltip-names-each-connected-device-and-its-battery", test_connected);
  g_test_add_func ("/state/every-adapter-counts-as-one", test_two_adapters);
  g_test_add_func ("/state/a-device-on-an-unannounced-adapter-is-not-connected", test_unknown_adapter);
  g_test_add_func ("/state/each-state-asks-for-mate-and-adwaita-names-before-breeze", test_icon_names);
  g_test_add_func ("/popup/sections-list-only-paired-devices-split-by-connection", test_listed_sections);
  g_test_add_func ("/popup/adapters-are-merged-and-sorted-by-name", test_listed_merges_adapters_by_name);
  g_test_add_func ("/popup/a-device-of-an-unpowered-adapter-is-not-listed",
                   test_listed_leaves_out_devices_of_an_unpowered_adapter);
  g_test_add_func ("/popup/a-battery-below-20-is-low", test_battery_low);
  g_test_add_func ("/popup/the-type-comes-from-the-devices-icon", test_device_kind);
  g_test_add_func ("/popup/a-row-always-has-an-icon", test_icon_for_a_device);
  g_test_add_func ("/popup/a-copy-outlives-its-snapshot", test_copy);
  return g_test_run ();
}
