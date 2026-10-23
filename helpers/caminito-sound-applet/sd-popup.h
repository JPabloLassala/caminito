/*
 * The Sound applet's popup (CMN-213, CMN-215, CMN-216): its content, in the shared popup
 * window. From the top, in one scrolled window, the output row and the input row: each a
 * round mute button with the device's form, the Default device's slider and a pill with its
 * name, which opens the side's list of sound devices beneath, with a port dropdown under it
 * when the default has more than one plugged port; the input's has a level meter under its
 * slider. Then Applications: System sounds first, the Sound applications playing, and those
 * recording under their own heading; each with its icon, its name, which opens "Plays to" or
 * "Records from" beneath with a slider for each of its streams when it has several, its
 * slider and percent, its activity bar, and its device as a badge while it doesn't follow the
 * default. Under the scrolled window, the footer's "Sound settings…". With no sound server
 * it's "No sound server" and the footer. Pinned applications are CMN-217's, and so is the
 * pin button, hidden until then.
 *
 * It draws an SdStatus, so that it knows nothing of the sound server, and calls back for what
 * the user does. It never changes a default by itself.
 */

#pragma once

#include "caminito-applet-popup.h"
#include "sd-state.h"

typedef struct _SdPopup SdPopup;

/* What the user asks for. The status that follows shows what the server made of it. */
typedef struct
{
  void (*set_volume)    (SdSide side, pa_volume_t volume, gpointer data);
  void (*set_muted)     (SdSide side, gboolean muted, gpointer data);
  /* The device by its SdDevice's name. */
  void (*set_default)   (SdSide side, const char *device, gpointer data);
  /* The device's port by its SdPort's name. */
  void (*set_port)      (SdSide side, const char *device, const char *port, gpointer data);
  /* "Sound settings…", once the popup is closed. */
  void (*open_settings) (gpointer data);
  /* An application's slider, by its SdApp's id. */
  void (*set_app_volume) (SdSide side, const char *app, pa_volume_t volume, gpointer data);
  /* One of its streams' sliders, by the SdAppStream's index. */
  void (*set_stream_volume) (SdSide side, guint32 stream, pa_volume_t volume, gpointer data);
  /* A device for the application, by its SdDevice's name; NULL to follow the default. */
  void (*move_app) (SdSide side, const char *app, const char *device, gpointer data);
  void (*set_system_volume) (pa_volume_t volume, gpointer data);
} SdPopupActions;

/* The content of applet_popup's window, which it lives as long as. */
SdPopup *sd_popup_new                 (CaminitoAppletPopup  *applet_popup,
                                       const SdPopupActions *actions,
                                       gpointer              data);
/* Draws what the sound server has now. Called on every change, shown or not. */
void     sd_popup_update              (SdPopup        *popup,
                                       const SdStatus *status);
/* media-keys' volume-step, which Shift+wheel and the arrows move a slider by, and GNOME's
   allow-volume-above-100-percent, which takes the sliders to 150%. */
void     sd_popup_set_volume_settings (SdPopup *popup,
                                       guint    step,
                                       gboolean above_100);
/* The most height the popup may take where it opens (sd_state_popup_room()): what's above
   the footer scrolls past it. */
void     sd_popup_set_room            (SdPopup *popup,
                                       int      room);
/* The Default input's peak, from 0 to 1, which the meter shows from its next frame, until
   the popup is closed. */
void     sd_popup_set_input_level     (SdPopup *popup,
                                       double   level);
/* An application's peak, from 0 to 1, which its activity bar shows from its next frame,
   until the popup is closed. */
void     sd_popup_set_app_level       (SdPopup    *popup,
                                       SdSide      side,
                                       const char *app,
                                       double      level);
void     sd_popup_free                (SdPopup *popup);
