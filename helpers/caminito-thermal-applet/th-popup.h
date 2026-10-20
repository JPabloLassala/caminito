/*
 * The Thermal applet's popup (268): its content, in the shared popup window. From the top:
 * a tile for each of CPU, GPU, Storage and Fans that the computer has, two to a row, then
 * "Show all sensors", an expander over every reading with bars, grouped. The pins on its
 * rows show the panel sensors and do nothing until 269.
 */

#pragma once

#include "caminito-applet-popup.h"
#include "th-model.h"

typedef struct _ThPopup ThPopup;

ThPopup *th_popup_new    (CaminitoAppletPopup *applet_popup);
void     th_popup_free   (ThPopup *popup);
/* What the model has now. The widgets are changed in place, so that the pointer's hover
   and the expander stay as they are; they're made again only when a reading comes or
   goes. */
void     th_popup_update (ThPopup *popup, ThModel *model);
