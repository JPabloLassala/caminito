/*
 * UPower as the Power applet reads it (262): the display device, which is UPower's one
 * battery for the whole computer, followed as it changes and as upowerd stops and starts.
 */

#pragma once

#include <gio/gio.h>

#include "pw-state.h"

typedef struct _PwUpower PwUpower;

/* Called from the main loop once a burst of changes has settled, and when upowerd comes or
   goes. */
typedef void (*PwUpowerChanged) (PwUpower *upower, gpointer data);

PwUpower *pw_upower_new     (PwUpowerChanged changed, gpointer data);
void      pw_upower_free    (PwUpower *upower);
/* The display device now: not present until the proxy is up and while upowerd isn't
   running. */
void      pw_upower_display (PwUpower *upower, PwDisplay *display);
