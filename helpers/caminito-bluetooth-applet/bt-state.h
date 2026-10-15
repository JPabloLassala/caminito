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
void         bt_snapshot_add_adapter (BtSnapshot *snapshot,
                                      const char *path,
                                      gboolean    powered);
void         bt_snapshot_add_device  (BtSnapshot *snapshot,
                                      const char *path,
                                      const char *adapter,
                                      const char *alias,
                                      gboolean    connected,
                                      int         battery);

/* Every adapter counts as one: on if any is powered. */
BtIconState  bt_state_icon           (const BtSnapshot *snapshot);
/* The connected devices on powered adapters, in path order. */
GPtrArray   *bt_state_connected      (const BtSnapshot *snapshot);
/* The state, then each connected device with its battery; NULL without an adapter. */
char        *bt_state_tooltip        (const BtSnapshot *snapshot);
/* The themed names for the state, the MATE and Adwaita names first and then Breeze's,
   which has none of theirs. NULL-terminated; NULL for BT_ICON_NONE. */
const char *const *bt_state_icon_names (BtIconState state);

G_DEFINE_AUTOPTR_CLEANUP_FUNC (BtSnapshot, bt_snapshot_free)
