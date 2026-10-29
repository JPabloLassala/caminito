/*
 * What the Notifications applet shows (CMN-260): the Notification history's entries as the
 * daemon's private API lists them (ADR 0051), grouped by app (CMN-265), an entry's age, the
 * icon and the tooltip, and when Do Not Disturb ends (CMN-263).
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
  /* The reply field's placeholder (KDE's inline reply), or empty for none. */
  char      *reply;
  /* When it arrived, in seconds since the epoch. */
  gint64     time;
} NtEntry;

/* What the Notification center shows as one: an app's entries, or one entry of an app whose
   grouping is off. */
typedef struct
{
  /* The Notifying app's key, the entries'. */
  const char *app;
  /* Whether it's the app's group, rather than one entry standing alone. */
  gboolean    grouped;
  /* Its NtEntries, newest first, which belong to the entries it was made from. */
  GPtrArray  *entries;
} NtGroup;

/* The D-Bus type of List's reply. */
#define NT_LIST_TYPE "(a(usssssva(ss)bbst))"

void               nt_entry_free        (NtEntry *entry);
/* The NtEntries of List's reply, in its order, newest first; NULL if reply isn't
   NT_LIST_TYPE. */
GPtrArray         *nt_entries_from_list (GVariant *reply);

/* The NtGroups of entries, the history newest first, by their newest entries: each app's, but
   an entry of one of ungrouped, a NULL-terminated array of apps' keys, on its own. They
   last as long as entries. */
GPtrArray         *nt_state_groups      (const GPtrArray   *entries,
                                         const char *const *ungrouped);

/* "now", "5 min ago", "3 h ago", "yesterday", "4 days ago": time's age at now, both in
   seconds since the epoch. */
char              *nt_state_age         (gint64 time,
                                         gint64 now);
/* The bell's themed names: Breeze's, XApp's, then Caminito's own in hicolor, struck through
   during Do Not Disturb. Bells only: a theme is searched for every name before the themes it
   inherits from, and MATE's preferences-system-notifications-symbolic, which Adwaita's bell
   is called too, is a speech bubble. NULL-terminated, in static storage. */
const char *const *nt_state_icon_names  (gboolean do_not_disturb);
/* "Until 14:30", "Until tomorrow, 02:13" or, for an end of 0, "Until turned off": when Do
   Not Disturb ends, in seconds since the epoch, as it reads at now in zone. */
char              *nt_state_until       (guint64    end,
                                         gint64     now,
                                         GTimeZone *zone);
/* "No notifications", "1 notification", "3 notifications". */
char              *nt_state_tooltip     (guint n_entries);
