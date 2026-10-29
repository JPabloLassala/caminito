/*
 * The Notification center (CMN-260): the Notifications applet's popup, in the shared popup
 * window. The Notification history as a flat list, newest first, each entry with its app's
 * icon and name, its age, a close button, its summary, body and image, and its action
 * buttons; "No notifications" when it's empty; and "Clear all" at the bottom. On top is the
 * Do Not Disturb row (CMN-263): its switch, which turns it on until turned off, "Until 14:30"
 * while it's on, and the durations to turn it on for or change its end to. Grouping by app
 * (CMN-265) comes later. It draws NtEntries and calls back for what the user does, so that it
 * knows nothing of D-Bus.
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
  /* "Clear all". */
  void (*clear_all) (gpointer data);
  /* The switch or a duration: until is "one-hour", "four-hours", "tomorrow-morning",
     "until-turned-off" or "off". The popup stays open. */
  void (*set_do_not_disturb) (const char *until, gpointer data);
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
/* Shows Do Not Disturb as the daemon has it: on or off, and its end in seconds since the
   epoch, or 0 for none, as it reads at now in the local zone. Nothing is called back. */
void     nt_popup_set_do_not_disturb (NtPopup *popup,
                                      gboolean on,
                                      guint64  end,
                                      gint64   now);
void     nt_popup_free   (NtPopup *popup);
