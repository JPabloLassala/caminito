/*
 * What the Sound applet's icon shows (CMN-213), from the sound server's Default output: the
 * icon's names, the strike over it, the red dot while an application records, and the
 * tooltip; the volume a scroll on it sets; and the right-click menu's cards and their
 * profiles. Nothing here talks to the sound server, so that the tests can build any status
 * by hand.
 */

#pragma once

#include <glib.h>
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
gboolean sd_cards_equal  (const SdStatus *a, const SdStatus *b);
/* Copies from's cards into to, which has none. */
void     sd_cards_copy   (const SdStatus *from, SdStatus *to);

/* The themed names for the icon, MATE's first, then Adwaita's symbolic ones, then Breeze's
   where it has neither. NULL-terminated, in static storage. */
const char *const *sd_state_icon_names   (const SdStatus *status);
/* Whether the applet strikes the icon out itself: no theme has crossed-out headphones. */
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
