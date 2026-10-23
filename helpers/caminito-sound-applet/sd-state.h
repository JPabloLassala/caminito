/*
 * What the Sound applet's icon shows (CMN-213), from the sound server's Default output: the
 * icon's names, the strike over it, the red dot while an application records, and the
 * tooltip; the volume a scroll on it sets; and the right-click menu's cards and their
 * profiles. For the popup's output and input rows (CMN-215): the sound devices of each side,
 * their forms, ports and icons, and the room the popup has where it opens. Nothing here
 * talks to the sound server, so that the tests can build any status by hand.
 */

#pragma once

#include <glib.h>
#include <pulse/def.h>
#include <pulse/volume.h>

/* A card's profile, as the right-click menu lists it. */
typedef struct
{
  char *name;
  char *description;
} SdProfile;

/* A card of the sound server, with the profiles that are available. */
typedef struct
{
  guint32    index;
  char      *description;
  SdProfile *profiles;
  guint      n_profiles;
  /* The active profile's name, NULL with none. */
  char      *active;
} SdCard;

/* What a sound device is, for its icon. */
typedef enum
{
  SD_FORM_SPEAKER,
  /* A headset, or a device on its Headphones port. */
  SD_FORM_HEADPHONES,
  /* A monitor's or a television's, over HDMI or DisplayPort. */
  SD_FORM_DISPLAY,
  /* Every input. */
  SD_FORM_MICROPHONE,
} SdForm;

/* The Default output's side of the popup, or the Default input's. */
typedef enum
{
  SD_OUTPUT,
  SD_INPUT,
} SdSide;

/* A port of a sound device, as the popup's port dropdown lists it. */
typedef struct
{
  char *name;
  char *description;
} SdPort;

/* A port as the sound server has it, for sd_state_plugged_ports(). */
typedef struct
{
  const char *name;
  const char *description;
  /* The server knows it's unplugged, not merely that it can't tell. */
  gboolean    unplugged;
} SdPortInfo;

/* A sound device of the popup's lists (CMN-215): an output, or an input that isn't an
   output's monitor. */
typedef struct
{
  /* The server's name, which the applet sets the default by. */
  char       *name;
  /* The server's description, which the applet shows. */
  char       *description;
  SdForm      form;
  /* No card: Easy Effects, a null sink. */
  gboolean    virtual_device;
  /* Its loudest channel's. */
  pa_volume_t volume;
  gboolean    muted;
  /* The plugged ports. */
  SdPort     *ports;
  guint       n_ports;
  /* The active port's name and description, NULL with none. */
  char       *active_port;
  char       *active_port_description;
} SdDevice;

/* A rectangle of the screen, in GDK's coordinates. */
typedef struct
{
  int x;
  int y;
  int width;
  int height;
} SdRect;

/* Where the popup opens, for sd_state_popup_room(). */
typedef struct
{
  /* The monitor's, and its work area, which GDK3 makes the monitor's on Wayland. */
  SdRect   geometry;
  SdRect   workarea;
  /* The panel's window, where the display says where it is: X11 does, Wayland doesn't. */
  gboolean panel_known;
  SdRect   panel;
  /* The panel's thickness, from its window's size. */
  int      panel_thickness;
  /* The way the popup opens: down from a top panel, up from a bottom one, or sideways
     from a vertical one. */
  gboolean opens_up;
  gboolean sideways;
} SdPlace;

typedef struct
{
  /* Connected to a sound server. The rest is meaningless without it. */
  gboolean    running;
  /* The Default output is one of the server's sound devices. */
  gboolean    has_output;
  char       *output_name;
  /* Its loudest channel's. */
  pa_volume_t volume;
  gboolean    muted;
  /* A headset, or a device on its Headphones port. */
  gboolean    headphones;
  /* The Default input is a microphone, not an output's monitor. */
  gboolean    has_input;
  gboolean    input_muted;
  /* The popup's lists, in the server's order, and the defaults' names: NULL where the
     default isn't one of them, as an output's monitor isn't. */
  SdDevice   *outputs;
  guint       n_outputs;
  SdDevice   *inputs;
  guint       n_inputs;
  char       *default_output;
  char       *default_input;
  /* The Sound applications recording now, by name, NULL-terminated; NULL with none. */
  char      **recording;
  SdCard     *cards;
  guint       n_cards;
} SdStatus;

/* One stream recording, as sd_state_recording_names() is given it. NULL where the sound
   server hasn't the property. */
typedef struct
{
  const char *app_id;
  const char *name;
  const char *binary;
} SdStream;

void     sd_status_clear (SdStatus *status);
gboolean sd_status_equal (const SdStatus *a, const SdStatus *b);

/* Frees what the card holds, not the card. */
void     sd_card_clear  (SdCard *card);
/* A deep copy of card into copy, for sd_card_clear(). */
void     sd_card_copy   (const SdCard *card, SdCard *copy);
gboolean sd_cards_equal (const SdCard *a, guint n_a, const SdCard *b, guint n_b);

/* The themed names for the icon, MATE's first, then the symbolic ones that Adwaita and
   Breeze have. NULL-terminated, in static storage. */
const char *const *sd_state_icon_names   (const SdStatus *status);
/* Whether the applet strikes the icon out itself: no theme has crossed-out headphones, and
   MATE's and Adwaita's have no speaker struck through. */
gboolean           sd_state_icon_struck  (const SdStatus *status);
/* "WH-1000XM4: 45% (muted)\nRecording: Discord, OBS Studio", "No sound server"… */
char              *sd_state_tooltip      (const SdStatus *status);
/* The volume a step of the wheel sets: step percent up or down from volume, at most 100%,
   or 150% with above_100. A volume already past it isn't lowered by a step up. */
pa_volume_t        sd_state_scroll_volume (pa_volume_t volume,
                                           guint       step,
                                           gboolean    up,
                                           gboolean    above_100);
/* The Sound applications' names for streams, one for the copies of a program: they're told
   apart by Flatpak app ID, then the application's name, then its binary. NULL-terminated,
   NULL with none. */
char             **sd_state_recording_names (const SdStream *streams,
                                             guint           n_streams);

/* Frees what the device holds, not the device. */
void            sd_device_clear (SdDevice *device);
/* A deep copy of device into copy, for sd_device_clear(). */
void            sd_device_copy  (const SdDevice *device, SdDevice *copy);
gboolean        sd_device_equal (const SdDevice *a, const SdDevice *b);
/* The side's list and its length. */
const SdDevice *sd_status_devices       (const SdStatus *status,
                                         SdSide          side,
                                         guint          *n_devices);
/* The side's Default device, NULL where it isn't in the list. */
const SdDevice *sd_status_default_device (const SdStatus *status,
                                          SdSide          side);

/* What a device is, from the server's form factor and its active port's name and type
   (a pa_device_port_type_t), NULL and PA_DEVICE_PORT_TYPE_UNKNOWN where it has none. Every
   input is a microphone. */
SdForm             sd_state_device_form       (SdSide      side,
                                               const char *form_factor,
                                               const char *port_name,
                                               int         port_type);
/* The ports the dropdown lists: the unplugged ones left out. NULL with none. */
SdPort            *sd_state_plugged_ports     (const SdPortInfo *ports,
                                               guint             n_ports,
                                               guint            *n_plugged);
/* The line under a device's name in the lists: "Virtual device", or its port; NULL with
   neither. */
const char        *sd_state_device_detail     (const SdDevice *device);
/* The themed names for a device in the lists, NULL-terminated, in static storage. */
const char *const *sd_state_device_icon_names (const SdDevice *device);
/* The themed names for the side's round mute button: the output's form, the speaker by its
   level; the microphone by its level. device is NULL with no Default device. */
const char *const *sd_state_mute_icon_names   (SdSide          side,
                                               const SdDevice *device);
/* Whether the popup strikes the mute button's icon out itself: themes have no crossed-out
   headphones or monitor. */
gboolean           sd_state_mute_struck       (SdSide          side,
                                               const SdDevice *device);
/* A volume in percent, as pactl rounds it, and back. */
guint              sd_state_percent           (pa_volume_t volume);
pa_volume_t        sd_state_volume_of_percent (double percent);
/* The most height the popup may take where it's placed (CMN-210): from its edge against
   the panel to the far side of the work area. */
int                sd_state_popup_room        (const SdPlace *place);
