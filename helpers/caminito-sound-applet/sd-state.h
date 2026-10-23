/*
 * What the Sound applet's icon shows (CMN-213), from the sound server's Default output: the
 * icon's names, the strike over it, the red dot while an application records, and the
 * tooltip; the volume a scroll on it sets; and the right-click menu's cards and their
 * profiles. For the popup's output and input rows (CMN-215): the sound devices of each side,
 * their forms, ports and icons, and the room the popup has where it opens. For its
 * Applications (CMN-216): the streams grouped into Sound applications, their slider's volume
 * and System sounds' volume. For its Pinned applications (CMN-217): the pins as the applet's
 * settings keep them, unpinning and its undo, and each pinned application, running or not.
 * Nothing here talks to the sound server, so that the tests can build any status by hand.
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

/* One of a Sound application's Streams, as its expanded row has it. */
typedef struct
{
  guint32     index;
  char       *title;
  pa_volume_t volume;
} SdStreamVolume;

/* A Sound application playing or recording now (CMN-216): its streams under one row. */
typedef struct
{
  /* Its Flatpak app ID, name or binary, which tell programs apart. */
  char        *id;
  char        *name;
  /* Its themed icon's name, NULL with none. */
  char        *icon;
  /* SD_OUTPUT where it plays, SD_INPUT where it records. */
  SdSide       side;
  /* The sound device it's moved to, by its SdDevice's name, and the server's description of
     it, which may be an output's monitor that it records: NULL while it follows the
     default. */
  char           *device;
  char           *device_description;
  /* Its loudest stream's, which its slider shows. */
  pa_volume_t     volume;
  SdStreamVolume *streams;
  guint           n_streams;
} SdApp;

/* A pin (CMN-217): a Sound application that the applet sends to a sound device every time it
   plays or records, as the applet keeps it. */
typedef struct
{
  SdSide side;
  /* The application's identity, sd_state_stream_identity()'s. */
  char  *id;
  /* What its row shows while it isn't running: its name and themed icon when it was pinned,
     the icon NULL with none. */
  char  *name;
  char  *icon;
  /* The device by its SdDevice's name, and the server's description of it, which its badge
     shows while it's gone. */
  char  *device;
  char  *device_description;
} SdPin;

/* A pin taken away, and where it was among the pins, for sd_pins_restore(). */
typedef struct
{
  SdPin pin;
  guint position;
} SdRemovedPin;

typedef enum
{
  /* A stream of it plays or records. */
  SD_PIN_PLAYING,
  /* Its streams are all paused. */
  SD_PIN_PAUSED,
  /* It has no stream. */
  SD_PIN_NOT_RUNNING,
} SdPinActivity;

/* A pinned application, running or not (CMN-217). */
typedef struct
{
  /* Its device is always the pin's; it has no streams while it isn't running. */
  SdApp         app;
  SdPinActivity activity;
  /* The pinned device isn't on the server: the application plays to the default meanwhile. */
  gboolean      device_gone;
} SdPinnedApp;

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
  /* The popup's Applications (CMN-216): those playing, in the server's order, then those
     recording, but the pinned ones. */
  SdApp      *apps;
  guint       n_apps;
  /* Its Pinned applications (CMN-217), in the pins' order. */
  SdPinnedApp *pinned;
  guint        n_pinned;
  /* System sounds' volume, the event role's in the stream-restore extension: FALSE until
     the server has given it. */
  gboolean    has_system_sounds;
  pa_volume_t system_volume;
} SdStatus;

/* A Stream as the sound server has it, for sd_state_recording_names() and sd_state_apps().
   NULL where the sound server hasn't the property. */
typedef struct
{
  const char *app_id;
  const char *name;
  const char *binary;
  /* Its themed icon's name. */
  const char *icon;
  /* What it plays or records, as a tab's title. */
  const char *title;
  guint32     index;
  /* The sound device it plays to or records from, by its SdDevice's name, and the server's
     description of it. */
  const char *device;
  const char *device_description;
  /* Its loudest channel's. */
  pa_volume_t volume;
  /* Paused. */
  gboolean    corked;
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
/* What tells a stream's program apart: its Flatpak app ID, then the application's name,
   then its binary. NULL with none of them. */
const char        *sd_state_stream_identity (const SdStream *stream);
/* The side's streams as Sound applications, one for the copies of a program, in the order
   each first comes. A stream with no identity is left out. An application whose stream is
   on another device than default_device is moved there. NULL with none. */
SdApp             *sd_state_apps            (const SdStream *streams,
                                             guint           n_streams,
                                             SdSide          side,
                                             const char     *default_device,
                                             guint          *n_apps);
/* A stream's volume once its application's slider goes from loudest to volume: the same
   share of it. With every stream silent, volume. */
pa_volume_t        sd_state_scaled_volume   (pa_volume_t stream,
                                             pa_volume_t loudest,
                                             pa_volume_t volume);

/* Frees what the application holds, not the application. */
void            sd_app_clear (SdApp *app);
gboolean        sd_app_equal (const SdApp *a, const SdApp *b);
/* The status's application on side by its SdApp's id, pinned or not, NULL where there's
   none. */
const SdApp    *sd_status_app (const SdStatus *status,
                               SdSide          side,
                               const char     *id);

/* Frees what the pin holds, not the pin. */
void            sd_pin_clear   (SdPin *pin);
/* A deep copy of pin into copy, for sd_pin_clear(). */
void            sd_pin_copy    (const SdPin *pin, SdPin *copy);
/* An empty GArray of SdPin, which frees the pins it drops. */
GArray         *sd_pins_new    (void);
/* An empty GArray of SdRemovedPin, which frees the pins it drops. */
GArray         *sd_removed_pins_new (void);
/* The pin's index for the application on side, -1 where it isn't pinned. */
int             sd_pins_find   (GArray     *pins,
                                SdSide      side,
                                const char *id);
/* A copy of pin among pins: in its application's place where it's pinned already, at the
   end otherwise. */
void            sd_pins_set    (GArray      *pins,
                                const SdPin *pin);
/* Takes the application's pin away, into removed, which forgets what an unpin before put
   there: only the last unpin is undone. */
void            sd_pins_remove (GArray     *pins,
                                SdSide      side,
                                const char *id,
                                GArray     *removed);
/* Takes every pin away, into removed, as sd_pins_remove(). */
void            sd_pins_remove_all (GArray *pins,
                                    GArray *removed);
/* Puts the pins of removed back where they were, as they were, and empties it: an
   application pinned again meanwhile is pinned as before. */
void            sd_pins_restore (GArray *pins,
                                 GArray *removed);
/* The pins as the applet's settings keep them: an aa{ss} of side, app, name, icon, device and
   device-description. A floating reference. */
GVariant       *sd_pins_to_variant   (GArray *pins);
/* The pins of the settings' value, leaving out those with no side, application or device,
   and an application's second pin on a side. */
GArray         *sd_pins_from_variant (GVariant *value);
/* The pinned application of pin, from its side's streams, paused ones too, and its side's
   devices, monitors too. */
void            sd_state_pinned_app  (const SdPin    *pin,
                                      const SdStream *streams,
                                      guint           n_streams,
                                      const SdDevice *devices,
                                      guint           n_devices,
                                      SdPinnedApp    *pinned);
/* Frees what the pinned application holds, not the pinned application. */
void            sd_pinned_app_clear  (SdPinnedApp *pinned);

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
