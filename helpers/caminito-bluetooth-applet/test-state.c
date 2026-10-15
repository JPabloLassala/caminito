/*
 * The icon's state and the tooltip (259), from snapshots built by hand.
 */

#include "bt-state.h"

#define HCI0 "/org/bluez/hci0"
#define HCI1 "/org/bluez/hci1"

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
  bt_snapshot_add_device (snapshot, HCI0 "/dev_A", HCI0, "Headphones", TRUE, 80);
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
  bt_snapshot_add_device (snapshot, HCI0 "/dev_A", HCI0, "Headphones", FALSE, 80);
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
  bt_snapshot_add_device (snapshot, HCI0 "/dev_B", HCI0, "Mouse", TRUE, -1);
  bt_snapshot_add_device (snapshot, HCI0 "/dev_A", HCI0, "Headphones", TRUE, 80);
  bt_snapshot_add_device (snapshot, HCI0 "/dev_C", HCI0, "Phone", FALSE, 50);
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
  bt_snapshot_add_device (snapshot, HCI0 "/dev_A", HCI0, "Headphones", TRUE, 80);
  bt_snapshot_add_device (snapshot, HCI1 "/dev_B", HCI1, "Keyboard", TRUE, 5);
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
  bt_snapshot_add_device (snapshot, HCI1 "/dev_A", HCI1, "Headphones", TRUE, -1);
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
  return g_test_run ();
}
