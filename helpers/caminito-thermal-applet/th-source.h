/*
 * The Thermal applet's source (267): every chip's temperatures and fans through libsensors,
 * with its labels and ignore lines from /etc/sensors3.conf and /etc/sensors.d/.
 */

#pragma once

#include <glib.h>

/* Takes libsensors, whose state is the process's: every Thermal applet in the panel shares
   it, and the last to go releases it. */
void       th_source_acquire (void);
void       th_source_release (void);
/* Every temperature and fan now, a GPtrArray of ThRaw. The chips are found again whenever
   /sys/class/hwmon changes, as when a module is loaded or unloaded. */
GPtrArray *th_source_read    (void);
