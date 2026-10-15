/*
 * The Power applet's popup (262): its content, in the shared popup window. The table of
 * batteries (263) is above "Configure Power Management…" at its bottom; CMN-98 and CMN-101
 * add the power profile and Stay awake above the table.
 */

#pragma once

#include "caminito-applet-popup.h"
#include "pw-state.h"

typedef struct _PwPopup PwPopup;

typedef struct
{
  /* "Configure Power Management…" was pressed. The popup has closed. */
  void (*configure) (gpointer data);
  /* The switch "Limit charging to N%" of the battery at path was turned on or off. The
     caller answers with pw_popup_limit_done(), and the switch stays where the user put it
     until then. */
  void (*set_limit) (const char *path, gboolean enable, gpointer data);
} PwPopupActions;

PwPopup *pw_popup_new        (CaminitoAppletPopup *applet_popup,
                              const PwPopupActions *actions,
                              gpointer              data);
void     pw_popup_free       (PwPopup *popup);
/* The devices UPower has now (PwDevice, see pw_upower_devices()). The table follows them;
   one that comes or goes is a row that does. */
void     pw_popup_update     (PwPopup *popup, GPtrArray *devices);
/* The answer to set_limit(): error is NULL when UPower did it. A refusal puts the switch
   back as it was and says why in the battery's menu. */
void     pw_popup_limit_done (PwPopup *popup, const char *path, const char *error);
