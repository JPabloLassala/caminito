/*
 * What the Notifications applet shows (CMN-260): the Notification history's entries as the
 * daemon's private API lists them (ADR 0051), an entry's age, the icon and the tooltip.
 * Nothing here talks to the daemon, so that the tests can build any history by hand.
 */

#pragma once

#include <glib.h>

/* One of an entry's actions, other than the default. */
typedef struct
{
  char *key;
  char *label;
} NtAction;

/* An entry of the Notification history, as List gives it. */
typedef struct
{
  guint32    id;
  /* The Notifying app's key, and the name it gives. */
  char      *app;
  char      *app_name;
  /* A path or a theme's name; empty for none. */
  char      *app_icon;
  char      *summary;
  /* Pango markup, which the daemon has made safe. */
  char      *body;
  /* The image: a path or a theme's name, or the pixels of image-data as (iiibiiay). NULL,
     or empty, for none. */
  char      *image_icon;
  GVariant  *image_pixels;
  /* Its NtActions, in the app's order. */
  GPtrArray *actions;
  gboolean   has_default;
  gboolean   resident;
  gboolean   reply;
  /* When it arrived, in seconds since the epoch. */
  gint64     time;
} NtEntry;

/* The D-Bus type of List's reply. */
#define NT_LIST_TYPE "(a(usssssva(ss)bbbt))"

void               nt_entry_free        (NtEntry *entry);
/* The NtEntries of List's reply, in its order, newest first; NULL if reply isn't
   NT_LIST_TYPE. */
GPtrArray         *nt_entries_from_list (GVariant *reply);

/* "now", "5 min ago", "3 h ago", "yesterday", "4 days ago": time's age at now, both in
   seconds since the epoch. */
char              *nt_state_age         (gint64 time,
                                         gint64 now);
/* The bell's themed names: Breeze's, XApp's, then Caminito's own in hicolor. Bells only: a
   theme is searched for every name before the themes it inherits from, and MATE's
   preferences-system-notifications-symbolic, which Adwaita's bell is called too, is a speech
   bubble. NULL-terminated, in static storage. */
const char *const *nt_state_icon_names  (void);
/* "No notifications", "1 notification", "3 notifications". */
char              *nt_state_tooltip     (guint n_entries);
