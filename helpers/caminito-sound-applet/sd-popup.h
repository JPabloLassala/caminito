/*
 * The Sound applet's popup (CMN-213): its content, in the shared popup window. It's empty
 * until the popup's tickets fill it (CMN-215, CMN-216, CMN-217), and with no sound server
 * it's "No sound server". It draws an SdStatus, so that it knows nothing of the sound server.
 */

#pragma once

#include "caminito-applet-popup.h"
#include "sd-state.h"

typedef struct _SdPopup SdPopup;

/* The content of applet_popup's window, which it lives as long as. */
SdPopup *sd_popup_new    (CaminitoAppletPopup *applet_popup);
/* Draws what the sound server has now. Called on every change, shown or not. */
void     sd_popup_update (SdPopup        *popup,
                          const SdStatus *status);
void     sd_popup_free   (SdPopup *popup);
