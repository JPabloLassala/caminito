/*
 * BlueZ as the Bluetooth applet reads it (259): its adapters, devices and batteries,
 * through the object manager at / on the system bus, followed as they change and as
 * bluetoothd stops and starts.
 */

#pragma once

#include <gio/gio.h>

#include "bt-state.h"

typedef struct _BtBluez BtBluez;

/* Called from the main loop once a burst of changes has settled, and when bluetoothd
   comes or goes. */
typedef void (*BtBluezChanged) (BtBluez *bluez, gpointer data);

BtBluez    *bt_bluez_new         (BtBluezChanged changed, gpointer data);
void        bt_bluez_free        (BtBluez *bluez);
/* What BlueZ has now; empty until the object manager is up and while bluetoothd isn't
   running. */
BtSnapshot *bt_bluez_snapshot    (BtBluez *bluez);
/* Sets Adapter1.Powered on every adapter. Turning on retries a few times, for an
   adapter that bluetoothd hasn't seen unblocked yet. */
void        bt_bluez_set_powered (BtBluez *bluez, gboolean powered);
