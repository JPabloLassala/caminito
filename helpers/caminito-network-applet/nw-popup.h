/*
 * The Network applet's popup (CMN-193): its content, in the shared popup window. With
 * NetworkManager not running it's only its icon and "NetworkManager isn't running".
 * Otherwise it's empty: the tiles and the card (CMN-196), the Networks tab (CMN-200) and the
 * Details tab (CMN-201) fill it.
 */

#pragma once

#include "caminito-applet-popup.h"

typedef struct _NwPopup NwPopup;

NwPopup *nw_popup_new         (CaminitoAppletPopup *applet_popup);
void     nw_popup_free        (NwPopup *popup);
/* Whether NetworkManager is running, which shows the popup's content or says it isn't. */
void     nw_popup_set_running (NwPopup *popup, gboolean running);
