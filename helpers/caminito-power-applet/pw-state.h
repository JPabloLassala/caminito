/*
 * What the Power applet shows (262), from UPower's display device: the icon's name, the
 * percentage beside it and the tooltip. Nothing here talks to D-Bus, so that the tests can
 * build any state by hand.
 */

#pragma once

#include <glib.h>

/* UPower's Device.State. */
typedef enum
{
  PW_STATE_UNKNOWN = 0,
  PW_STATE_CHARGING = 1,
  PW_STATE_DISCHARGING = 2,
  PW_STATE_EMPTY = 3,
  PW_STATE_FULLY_CHARGED = 4,
  PW_STATE_PENDING_CHARGE = 5,
  PW_STATE_PENDING_DISCHARGE = 6,
} PwState;

typedef struct
{
  /* The display device's IsPresent: FALSE with no battery, or without UPower. */
  gboolean present;
  PwState  state;
  /* Percentage, 0 to 100. */
  double   percentage;
  /* Seconds, 0 when UPower doesn't know. */
  gint64   time_to_empty;
  gint64   time_to_full;
} PwDisplay;

typedef enum
{
  /* A battery that's discharging, or that UPower has no state for. */
  PW_ICON_BATTERY,
  PW_ICON_CHARGING,
  /* On the charger and fully charged. */
  PW_ICON_FULL,
  /* No battery: a plug. */
  PW_ICON_PLUG,
} PwIconKind;

PwIconKind          pw_state_icon_kind  (const PwDisplay *display);
/* The themed names for the display device's icon, the MATE and Adwaita names first and
   then Breeze's, which has none of theirs. NULL-terminated, in static storage. */
const char *const *pw_state_icon_names  (const PwDisplay *display);
/* The text beside the glyph, "85%"; NULL with no battery. */
char               *pw_state_percentage (const PwDisplay *display);
/* A duration for the tooltip: "3 h 40 min", "50 min", "2 h". Minutes round to the nearest,
   and under one minute it's "1 min". */
char               *pw_state_duration   (gint64 seconds);
/* The percentage and the time: "85%\n3 h 40 min left", "100%\nFull in 50 min"…, or
   "Plugged in" with no battery. */
char               *pw_state_tooltip    (const PwDisplay *display);
