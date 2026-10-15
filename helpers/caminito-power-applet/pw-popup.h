/*
 * The Power applet's popup (262): its content, in the shared popup window. From the top: the
 * power profile (264), Stay awake (265) and what keeps the computer awake, the table of
 * batteries (263), and "Configure Power Management…" at the bottom.
 */

#pragma once

#include "caminito-applet-popup.h"
#include "pw-state.h"

typedef struct _PwPopup PwPopup;

typedef struct
{
  /* "Configure Power Management…" was pressed. The popup has closed. */
  void (*configure) (gpointer data);
  /* Stay awake's switch was turned on or off. */
  void (*set_stay_awake) (gboolean on, gpointer data);
  /* The switch "Limit charging to N%" of the battery at path was turned on or off. The
     caller answers with pw_popup_limit_done(), and the switch stays where the user put it
     until then. */
  void (*set_limit) (const char *path, gboolean enable, gpointer data);
} PwPopupActions;

PwPopup *pw_popup_new  (CaminitoAppletPopup *applet_popup,
                        const PwPopupActions *actions,
                        gpointer              data);
void     pw_popup_free (PwPopup *popup);
/* Stay awake's switch as it is now. It doesn't call set_stay_awake. */
void     pw_popup_set_stay_awake (PwPopup *popup, gboolean on);
/* What keeps the computer awake: the PwInhibitors, which are copied. */
void     pw_popup_set_inhibitors (PwPopup *popup, const GPtrArray *inhibitors);
/* The devices UPower has now (PwDevice, see pw_upower_devices()). The table follows them;
   one that comes or goes is a row that does. */
void     pw_popup_update_devices (PwPopup *popup, GPtrArray *devices);
/* The answer to set_limit(): error is NULL when UPower did it. A refusal puts the switch
   back as it was and says why in the battery's menu. */
void     pw_popup_limit_done     (PwPopup *popup, const char *path, const char *error);
