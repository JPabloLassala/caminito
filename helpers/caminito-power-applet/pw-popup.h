/*
 * The Power applet's popup (262): its content, in the shared popup window. Empty but for
 * "Configure Power Management…" at its bottom, which CMN-104, CMN-98 and CMN-101 fill
 * from above.
 */

#pragma once

#include "caminito-applet-popup.h"

typedef struct _PwPopup PwPopup;

typedef struct
{
  /* "Configure Power Management…" was pressed. The popup has closed. */
  void (*configure) (gpointer data);
} PwPopupActions;

PwPopup *pw_popup_new  (CaminitoAppletPopup *applet_popup,
                        const PwPopupActions *actions,
                        gpointer              data);
void     pw_popup_free (PwPopup *popup);
