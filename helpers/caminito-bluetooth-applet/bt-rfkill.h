/*
 * Bluetooth off is an rfkill soft block on every Bluetooth adapter (the spec's "On and
 * off"), so that systemd-rfkill keeps it across reboots and the airplane-mode key agrees.
 */

#pragma once

#include <glib.h>

#define BT_RFKILL_DEVICE "/dev/rfkill"

/* Blocks or unblocks every Bluetooth radio through the rfkill device at path. The seat's
   user may write /dev/rfkill through systemd's uaccess. */
gboolean bt_rfkill_set_blocked (const char *path,
                                gboolean    blocked,
                                GError    **error);
