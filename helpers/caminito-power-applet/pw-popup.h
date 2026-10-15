/*
 * The Power applet's popup (262): its content, in the shared popup window. From the top: the
 * power profile (264), Stay awake (265) and what keeps the computer awake, and "Configure
 * Power Management…" at the bottom. CMN-104 fills in the batteries.
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
} PwPopupActions;

PwPopup *pw_popup_new  (CaminitoAppletPopup *applet_popup,
                        const PwPopupActions *actions,
                        gpointer              data);
void     pw_popup_free (PwPopup *popup);
/* Stay awake's switch as it is now. It doesn't call set_stay_awake. */
void     pw_popup_set_stay_awake (PwPopup *popup, gboolean on);
/* What keeps the computer awake: the PwInhibitors, which are copied. */
void     pw_popup_set_inhibitors (PwPopup *popup, const GPtrArray *inhibitors);
