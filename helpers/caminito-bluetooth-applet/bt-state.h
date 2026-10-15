/*
 * What the Bluetooth applet shows (259), from what BlueZ has: the icon's state and the
 * tooltip. Nothing here talks to D-Bus, so that the tests can build any state by hand.
 */

#pragma once

#include <glib.h>

typedef enum
{
  /* No adapter: the applet hides itself. */
  BT_ICON_NONE,
  /* No adapter is powered, which is what an rfkill block leaves too. */
  BT_ICON_OFF,
  BT_ICON_ON,
  /* A device is connected to a powered adapter. */
  BT_ICON_CONNECTED,
} BtIconState;

typedef struct
{
  char     *path;
  gboolean  powered;
} BtAdapter;

typedef struct
{
  char     *path;
  /* The adapter's object path, Device1.Adapter. */
  char     *adapter;
  /* Device1.Alias, which BlueZ falls back to the name or the address for. */
  char     *alias;
  /* Device1.Address, as "AA:BB:CC:DD:EE:FF". */
  char     *address;
  /* Device1.Icon, BlueZ's themed name for the device's class; NULL when it has none. */
  char     *icon;
  /* Device1.Paired. BlueZ lists nearby devices that aren't, which the popup doesn't. */
  gboolean  paired;
  gboolean  connected;
  /* Battery1.Percentage, or -1 without Battery1. */
  int       battery;
} BtDevice;

typedef struct
{
  GPtrArray *adapters;
  GPtrArray *devices;
} BtSnapshot;

/* An empty snapshot, which adapters and devices are added to in any order. */
BtSnapshot  *bt_snapshot_new         (void);
void         bt_snapshot_free        (BtSnapshot *snapshot);
/* A deep copy, for what outlives the call that got the snapshot. */
BtSnapshot  *bt_snapshot_copy        (const BtSnapshot *snapshot);
void         bt_snapshot_add_adapter (BtSnapshot *snapshot,
                                      const char *path,
                                      gboolean    powered);
void         bt_snapshot_add_device  (BtSnapshot *snapshot,
                                      const char *path,
                                      const char *adapter,
                                      const char *alias,
                                      const char *address,
                                      const char *icon,
                                      gboolean    paired,
                                      gboolean    connected,
                                      int         battery);

/* Every adapter counts as one: on if any is powered. */
BtIconState  bt_state_icon           (const BtSnapshot *snapshot);
/* The connected devices on powered adapters, in path order. */
GPtrArray   *bt_state_connected      (const BtSnapshot *snapshot);
/* The popup's sections (260): the paired devices on powered adapters, from every adapter
   merged, that are connected or, with FALSE, aren't; by name, then path. */
GPtrArray   *bt_state_listed         (const BtSnapshot *snapshot,
                                      gboolean          connected);
/* Whether a battery's level is shown in red: below 20%. -1 is no Battery1. */
gboolean     bt_state_battery_low    (int battery);
/* The device's type for the details line, from Device1.Icon: "Headphones", "Keyboard"…;
   "Device" for a class BlueZ has no icon for. */
const char  *bt_state_device_kind    (const char *icon);
/* The themed names for a row's icon: the device's own, then its symbolic one, and a
   Bluetooth icon last so that no row is blank. Free with g_strfreev(). */
char       **bt_state_device_icon_names (const char *icon);
/* The state, then each connected device with its battery; NULL without an adapter. */
char        *bt_state_tooltip        (const BtSnapshot *snapshot);
/* The themed names for the state, the MATE and Adwaita names first and then Breeze's,
   which has none of theirs. NULL-terminated; NULL for BT_ICON_NONE. */
const char *const *bt_state_icon_names (BtIconState state);

G_DEFINE_AUTOPTR_CLEANUP_FUNC (BtSnapshot, bt_snapshot_free)
