/*
 * UPower as the Power applet reads it (262): the display device, which is UPower's one
 * battery for the whole computer, followed as it changes and as upowerd stops and starts;
 * and (263) every other device, as they come and go.
 */

#pragma once

#include <gio/gio.h>

#include "pw-state.h"

typedef struct _PwUpower PwUpower;

/* Called from the main loop once a burst of changes has settled, when a device comes or
   goes, and when upowerd comes or goes. */
typedef void (*PwUpowerChanged) (PwUpower *upower, gpointer data);

PwUpower *pw_upower_new     (PwUpowerChanged changed, gpointer data);
void      pw_upower_free    (PwUpower *upower);
/* The display device now: not present until the proxy is up and while upowerd isn't
   running. */
void      pw_upower_display (PwUpower *upower, PwDisplay *display);
/* Every device UPower lists now but the display device, as PwDevice, for pw_state_rows().
   Free with g_ptr_array_unref(). Empty until the proxies are up and while upowerd isn't
   running. */
GPtrArray *pw_upower_devices (PwUpower *upower);

/* error is NULL when UPower did it. The device's own changes follow, from the changed
   callback. Called from the main loop, never from pw_upower_enable_charge_threshold(), and
   not after pw_upower_free(). */
typedef void (*PwUpowerDone) (const char *path, const char *error, gpointer data);

/* Device.EnableChargeThreshold on the device at path, which asks polkit and may show its
   dialog: done says if it was refused. */
void pw_upower_enable_charge_threshold (PwUpower    *upower,
                                        const char  *path,
                                        gboolean     enable,
                                        PwUpowerDone done,
                                        gpointer     data);
