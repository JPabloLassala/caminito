/*
 * The Notification center (CMN-260): the Notifications applet's popup, in the shared popup
 * window. The Notification history grouped by app (CMN-265), newest group first: each group
 * with its app's icon and name, its count and a clear button, collapsed to its latest few with
 * "N more" to open it. An entry of an app whose grouping is off stands alone among the
 * groups, with its app's icon and name. Each entry has its age, a close button, its summary,
 * body and image, and its action buttons, and its reply field (CMN-266). "No notifications" when it's empty, and "Clear all"
 * at the bottom. The Do Not Disturb row (CMN-263) comes later. It draws NtEntries and calls
 * back for what the user does, so that it knows nothing of D-Bus.
 */

#pragma once

#include "caminito-applet-popup.h"
#include "nt-state.h"

typedef struct _NtPopup NtPopup;

typedef struct
{
  /* A click on an entry, for "default", or one of its action buttons, for the action's
     key, with an xdg-activation token for the app to come forward with, or NULL. The popup
     has closed before it's called. */
  void (*invoke)    (guint32 id, const char *key, const char *token, gpointer data);
  /* text, typed in an entry's reply field and sent. The popup stays. */
  void (*reply)     (guint32 id, const char *text, gpointer data);
  /* An entry's close button. */
  void (*dismiss)   (guint32 id, gpointer data);
  /* A group's clear button, for its app's key. */
  void (*clear_app) (const char *app, gpointer data);
  /* "Clear all". The popup has closed before it's called. */
  void (*clear_all) (gpointer data);
} NtPopupActions;

/* The content of applet_popup's window, which it lives as long as. actions is copied. */
NtPopup *nt_popup_new    (CaminitoAppletPopup  *applet_popup,
                          const NtPopupActions *actions,
                          gpointer              data);
/* Draws entries, the NtEntries of the history newest first, with their ages at now, in
   seconds since the epoch, grouped but for the apps of ungrouped, NULL-terminated. Called on
   every change, shown or not. While the popup shows, the ages are brought up to date each
   minute. A group opened stays open. */
void     nt_popup_update (NtPopup           *popup,
                          const GPtrArray   *entries,
                          const char *const *ungrouped,
                          gint64             now);
void     nt_popup_free   (NtPopup *popup);
