/*
 * The Notification center (CMN-260): the Notifications applet's popup, in the shared popup
 * window. The Notification history as a flat list, newest first, each entry with its app's
 * icon and name, its age, a close button, its summary, body and image, and its action
 * buttons; "No notifications" when it's empty; and "Clear all" at the bottom. Grouping by
 * app (CMN-265) and the Do Not Disturb row (CMN-263) come later. It draws NtEntries and calls
 * back for what the user does, so that it knows nothing of D-Bus.
 */

#pragma once

#include "caminito-applet-popup.h"
#include "nt-state.h"

typedef struct _NtPopup NtPopup;

typedef struct
{
  /* A click on an entry, for "default", or one of its action buttons, for the action's
     key. The popup has closed before it's called. */
  void (*invoke)    (guint32 id, const char *key, gpointer data);
  /* An entry's close button. */
  void (*dismiss)   (guint32 id, gpointer data);
  /* "Clear all". The popup has closed before it's called. */
  void (*clear_all) (gpointer data);
} NtPopupActions;

/* The content of applet_popup's window, which it lives as long as. actions is copied. */
NtPopup *nt_popup_new    (CaminitoAppletPopup  *applet_popup,
                          const NtPopupActions *actions,
                          gpointer              data);
/* Draws entries, the NtEntries of the history newest first, with their ages at now, in
   seconds since the epoch. Called on every change, shown or not. While the popup shows, the
   ages are brought up to date each minute. */
void     nt_popup_update (NtPopup         *popup,
                          const GPtrArray *entries,
                          gint64           now);
void     nt_popup_free   (NtPopup *popup);
