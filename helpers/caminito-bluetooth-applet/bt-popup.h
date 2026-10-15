/*
 * The Bluetooth applet's popup (260, ADR 0035), in Plasma's layout: a header with the
 * switch for Bluetooth, the connected devices and then the paired ones that aren't, each
 * row expanding to Connect or Disconnect and the device's type and address, and a footer
 * with Add New Device… and Configure Bluetooth…. It draws what bt-state.c lists and
 * calls back for what the user does, so that it knows nothing of BlueZ.
  */

#pragma once

#include <gtk/gtk.h>

#include "bt-state.h"
#include "caminito-applet-popup.h"

typedef struct _BtPopup BtPopup;

typedef struct
{
  /* The header's switch, for every adapter. */
  void (*set_powered)   (gboolean on, gpointer data);
  /* A row's Connect (TRUE) or Disconnect, for the device at path. The popup shows the
     device busy until bt_popup_connect_done(). */
  void (*set_connected) (const char *path, gboolean connected, gpointer data);
  /* "Add New Device…" and the ⚙ button: the popup closes after them. */
  void (*add_device)    (gpointer data);
  void (*configure)     (gpointer data);
} BtPopupActions;

/* The content of popup's window, which it lives as long as. actions is copied. */
BtPopup *bt_popup_new          (CaminitoAppletPopup *popup,
                                const BtPopupActions *actions,
                                gpointer              data);
/* Draws what BlueZ has now. Called on every change, shown or not. */
void     bt_popup_update       (BtPopup         *popup,
                                const BtSnapshot *snapshot);
/* A Connect or Disconnect has ended: error is BlueZ's message, or NULL. */
void     bt_popup_connect_done (BtPopup    *popup,
                                const char *path,
                                const char *error);
void     bt_popup_free         (BtPopup *popup);
