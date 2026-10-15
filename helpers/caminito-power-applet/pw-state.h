/*
 * What the Power applet shows (262), from UPower's display device: the icon's name, the
 * percentage beside it and the tooltip; and (263) the popup's table of batteries, from
 * UPower's devices. Nothing here talks to D-Bus, so that the tests can build any state by
 * hand.
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

/* UPower's Device.Type, the ones the table tells apart. */
enum
{
  PW_TYPE_UNKNOWN = 0,
  PW_TYPE_LINE_POWER = 1,
  PW_TYPE_BATTERY = 2,
  PW_TYPE_UPS = 3,
  PW_TYPE_MONITOR = 4,
  PW_TYPE_MOUSE = 5,
  PW_TYPE_KEYBOARD = 6,
  PW_TYPE_PDA = 7,
  PW_TYPE_PHONE = 8,
  PW_TYPE_MEDIA_PLAYER = 9,
  PW_TYPE_TABLET = 10,
  PW_TYPE_COMPUTER = 11,
  PW_TYPE_GAMING_INPUT = 12,
  PW_TYPE_PEN = 13,
  PW_TYPE_TOUCHPAD = 14,
  PW_TYPE_HEADSET = 17,
  PW_TYPE_SPEAKERS = 18,
  PW_TYPE_HEADPHONES = 19,
  PW_TYPE_OTHER_AUDIO = 21,
  PW_TYPE_REMOTE_CONTROL = 22,
  PW_TYPE_CAMERA = 25,
  PW_TYPE_WEARABLE = 26,
};

/* One of UPower's devices, as the table needs it. */
typedef struct
{
  char    *path;
  guint    type;
  gboolean power_supply;
  gboolean present;
  PwState  state;
  /* Percentage, 0 to 100, and whether UPower measures one: a device that only has a
     coarse level (low, normal, full…) reports 0. */
  double   percentage;
  gboolean has_percentage;
  char    *model;
  /* The battery's health, in percent of what it was built for; 0 when UPower doesn't
     know. */
  double   capacity;
  gboolean threshold_supported;
  gboolean threshold_enabled;
  guint    threshold_end;
} PwDevice;

PwDevice *pw_device_new  (void);
void      pw_device_free (PwDevice *device);

/* A row of the table, as drawn. */
typedef struct
{
  char     *path;
  /* A laptop's battery: it has the ⋮, and comes first. Else a peripheral. */
  gboolean  laptop;
  char     *name;
  /* "Discharging", or "Charging · limit 80%". */
  char     *state;
  /* The themed names of its icon, NULL-terminated. */
  char    **icon_names;
  /* 0 to 1, for the bar. */
  double    fraction;
  /* "73%". */
  char     *percentage;
  /* Below 20%: the percentage is in the warning colour. */
  gboolean  low;
  /* Laptop only. "Health 58%", or "Health unknown". */
  char     *health;
  /* Where the battery has a charge limit, whether it's on, and the percentage it limits
     to: the one UPower has, or 80 where UPower has none that limits. */
  gboolean  limit_supported;
  gboolean  limit_enabled;
  guint     limit_end;
} PwRow;

void       pw_row_free  (PwRow *row);
gboolean   pw_row_equal (const PwRow *a, const PwRow *b);
/* The table for devices (PwDevice): the laptop's batteries in the order of their paths,
   then the peripherals by name. A battery with PowerSupply true is the laptop's. Line
   power, devices that aren't present and ones with no percentage aren't in it. Free
   with g_ptr_array_unref(); the rows are PwRow. */
GPtrArray *pw_state_rows (GPtrArray *devices);
