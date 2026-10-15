/*
 * power-profiles-daemon as the Power popup reads it (264): its profiles, followed as they
 * change, and the call that sets the active one. The daemon is on the bus name
 * org.freedesktop.UPower.PowerProfiles (0.20 and later, and tuned-ppd), or on the earlier
 * net.hadess.PowerProfiles, and the first is preferred where both are there.
 */

#pragma once

#include <gio/gio.h>

#include "pw-profiles.h"

typedef struct _PwPpd PwPpd;

/* Called from the main loop once a burst of changes has settled, and when the daemon comes
   or goes. */
typedef void (*PwPpdChanged) (PwPpd *ppd, gpointer data);

PwPpd      *pw_ppd_new      (PwPpdChanged changed, gpointer data);
void        pw_ppd_free     (PwPpd *ppd);
/* The daemon's profiles now, to free with pw_profiles_free; NULL when no daemon is on the
   bus, and until its proxy is up. */
PwProfiles *pw_ppd_profiles (PwPpd *ppd);
/* Sets ActiveProfile. The daemon's answer is a change; a failure is a warning. */
void        pw_ppd_set_active (PwPpd *ppd, PwProfile profile);
