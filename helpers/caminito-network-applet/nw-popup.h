/*
 * The Network applet's popup (CMN-193, CMN-196, CMN-201): its content, in the shared popup
 * window, as the prototype's Popup C. From the top: four tiles for Wi-Fi, Wired, VPN and
 * airplane mode; the card of the connection the icon shows; the Networks tab, which its own
 * ticket fills (CMN-200), and the Details tab, each connection up with its addresses and
 * the rest; and "Configure network connections…". With
 * NetworkManager not running it's only its icon and "NetworkManager isn't running". It
 * draws an NwStatus and calls back for what the user does, so that it knows nothing of
 * NetworkManager.
 */

#pragma once

#include "caminito-applet-popup.h"
#include "nw-state.h"

typedef struct _NwPopup NwPopup;

typedef struct
{
  /* A tile's click, for the state it doesn't have. The tile stays as it was until an
     update says NetworkManager has followed. */
  void (*set_tile)   (NwTile tile, gboolean on, gpointer data);
  /* The card's Disconnect. */
  void (*disconnect) (gpointer data);
  /* "Configure network connections…": the popup has closed before it's called. */
  void (*configure)  (gpointer data);
} NwPopupActions;

/* The content of applet_popup's window, which it lives as long as. actions is copied. */
NwPopup *nw_popup_new    (CaminitoAppletPopup  *applet_popup,
                          const NwPopupActions *actions,
                          gpointer              data);
/* Draws what NetworkManager has now. Called on every change, shown or not. */
void     nw_popup_update (NwPopup        *popup,
                          const NwStatus *status);
void     nw_popup_free   (NwPopup *popup);
