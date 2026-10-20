/*
 * The Thermal applet's popup (268): its content, in the shared popup window. From the top:
 * a tile for each of CPU, GPU, Storage and Fans that the computer has, two to a row, then
 * "Show all sensors", an expander over every reading with bars, grouped. The pins on its
 * rows show the panel sensors, and a click on one is the applet's to act on (269).
 */

#pragma once

#include "caminito-applet-popup.h"
#include "th-model.h"

typedef struct _ThPopup ThPopup;

/* What the user does in it, for the applet to keep (269). */
typedef struct
{
  /* A temperature row's pin was clicked: the reading's id. */
  void (*pin)      (const char *id, gpointer data);
  /* Show all sensors was opened or closed. */
  void (*show_all) (gboolean open, gpointer data);
} ThPopupActions;

ThPopup *th_popup_new    (CaminitoAppletPopup  *applet_popup,
                          const ThPopupActions *actions,
                          gpointer              data);
void     th_popup_free   (ThPopup *popup);
/* What the model has now. The widgets are changed in place, so that the pointer's hover
   and the expander stay as they are; they're made again only when a reading comes or
   goes. */
void     th_popup_update (ThPopup *popup, ThModel *model);
/* Opens or closes Show all sensors, as the preferences have it, with no call to show_all. */
void     th_popup_set_show_all (ThPopup *popup, gboolean open);
