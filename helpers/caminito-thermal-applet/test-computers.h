/*
 * The computers the Thermal applet's tests build by hand (267, 268), as the source would
 * read them: GPtrArrays of ThRaw.
 */

#pragma once

#include "th-model.h"

/* A temperature in °C with its crit, NAN with none; a fan in RPM with its max. */
void       temp     (GPtrArray *raws, const char *chip, const char *chip_id,
                     const char *feature, const char *label, double value, double crit);
void       fan      (GPtrArray *raws, const char *chip, const char *chip_id,
                     const char *feature, const char *label, double rpm, double max);
GPtrArray *raws_new (void);
/* The developer's ThinkPad: coretemp, the embedded controller with one fan with no max and
   readings that are always 0, ACPI, Wi-Fi, an NVMe drive, and the battery chips. */
GPtrArray *thinkpad (double package, double fan_rpm);
/* A desktop: an AMD CPU, a Radeon with a fan with a max, two NVMe drives, two fans with
   none. */
GPtrArray *desktop  (double tctl, double junction, double cpu_fan, double chassis_fan,
                     double gpu_fan);
