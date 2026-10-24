/*
 * The Dock's Launcher entries (CMN-227), without a panel: what programs tell it about their
 * Dock items through Unity's com.canonical.Unity.LauncherEntry Update signal, a count for a
 * badge, a progress for a bar and whether they are urgent, kept for each desktop entry with
 * the unique bus name of the sender that told it.
 */

#pragma once

#include <glib.h>

#include "dk-model.h"
#include "dk-resolve.h"

/* The prefix of the signal's app_uri. */
#define DK_LAUNCHER_URI_PREFIX "application://"
/* The most a badge counts to before it reads "99+". */
#define DK_LAUNCHER_COUNT_CAP 99

typedef struct _DkLauncher DkLauncher;

typedef struct
{
  gint64   count;
  gboolean count_visible;
  /* 0.0 to 1.0. */
  double   progress;
  gboolean progress_visible;
  gboolean urgent;
} DkLauncherEntry;

DkLauncher *dk_launcher_new  (void);
void        dk_launcher_free (DkLauncher *launcher);

/* The Update signal of sender (its unique name) for app_uri with properties (an a{sv}): finds
   the desktop entry the uri names and changes the properties the signal carries, and no
   others. An entry with no Dock item yet is kept all the same. Returns whether the uri named
   an entry. */
gboolean    dk_launcher_update (DkLauncher      *launcher,
                                const DkCatalog *catalog,
                                const char      *sender,
                                const char      *app_uri,
                                GVariant        *properties);

/* What the entry with the desktop entry id key shows: nothing, for one never told. */
DkLauncherEntry dk_launcher_get (const DkLauncher *launcher, const char *key);

/* The sender has left the bus, or the entry's item has no window left. They return whether
   anything was cleared. */
gboolean    dk_launcher_clear_sender (DkLauncher *launcher, const char *sender);
gboolean    dk_launcher_clear_id     (DkLauncher *launcher, const char *id);

/* Clears the entries of the items that had a window at the last sync and have none now,
   with the dock's items (dk_items_build's). */
void        dk_launcher_sync (DkLauncher *launcher, const GArray *items);

/* The text of a badge: the count, "99+" above 99, and NULL for a count of 0 or less. */
char       *dk_launcher_badge_text (gint64 count);
