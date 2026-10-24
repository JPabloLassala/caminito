/*
 * The Dock's Launcher entries (CMN-227).
 */

#include <math.h>
#include <string.h>

#include "dk-launcher.h"

typedef struct
{
  DkLauncherEntry shown;
  /* The unique name that sent the latest update. */
  char           *sender;
} Entry;

struct _DkLauncher
{
  /* A desktop entry's id to its Entry. */
  GHashTable *entries;
  /* The keys of the items that had a window at the last sync. */
  GHashTable *windowed;
};

static void
entry_free (gpointer data)
{
  Entry *entry = data;

  g_free (entry->sender);
  g_free (entry);
}

DkLauncher *
dk_launcher_new (void)
{
  DkLauncher *launcher = g_new0 (DkLauncher, 1);

  launcher->entries = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, entry_free);
  launcher->windowed = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  return launcher;
}

void
dk_launcher_free (DkLauncher *launcher)
{
  if (launcher == NULL)
    return;
  g_hash_table_unref (launcher->entries);
  g_hash_table_unref (launcher->windowed);
  g_free (launcher);
}

/* The properties of the signal's types; a value of another type is ignored. */
static void
take_properties (DkLauncherEntry *shown, GVariant *properties)
{
  g_autoptr (GVariant) count = g_variant_lookup_value (properties, "count",
                                                       G_VARIANT_TYPE_INT64);
  g_autoptr (GVariant) count_visible = g_variant_lookup_value (properties, "count-visible",
                                                               G_VARIANT_TYPE_BOOLEAN);
  g_autoptr (GVariant) progress = g_variant_lookup_value (properties, "progress",
                                                          G_VARIANT_TYPE_DOUBLE);
  g_autoptr (GVariant) progress_visible = g_variant_lookup_value (properties,
                                                                  "progress-visible",
                                                                  G_VARIANT_TYPE_BOOLEAN);
  g_autoptr (GVariant) urgent = g_variant_lookup_value (properties, "urgent",
                                                        G_VARIANT_TYPE_BOOLEAN);

  if (count != NULL)
    shown->count = g_variant_get_int64 (count);
  if (count_visible != NULL)
    shown->count_visible = g_variant_get_boolean (count_visible);
  /* A progress that is no number is no progress. */
  if (progress != NULL && !isnan (g_variant_get_double (progress)))
    shown->progress = CLAMP (g_variant_get_double (progress), 0.0, 1.0);
  if (progress_visible != NULL)
    shown->progress_visible = g_variant_get_boolean (progress_visible);
  if (urgent != NULL)
    shown->urgent = g_variant_get_boolean (urgent);
}

gboolean
dk_launcher_update (DkLauncher *launcher, const DkCatalog *catalog, const char *sender,
                    const char *app_uri, GVariant *properties)
{
  const char *id;
  Entry *entry;

  if (app_uri == NULL || !g_str_has_prefix (app_uri, DK_LAUNCHER_URI_PREFIX)
      || properties == NULL || !g_variant_is_of_type (properties, G_VARIANT_TYPE_VARDICT))
    return FALSE;
  id = dk_catalog_resolve (catalog, app_uri + strlen (DK_LAUNCHER_URI_PREFIX));
  if (id == NULL)
    return FALSE;
  entry = g_hash_table_lookup (launcher->entries, id);
  if (entry == NULL)
    {
      entry = g_new0 (Entry, 1);
      g_hash_table_insert (launcher->entries, g_strdup (id), entry);
    }
  take_properties (&entry->shown, properties);
  if (g_strcmp0 (entry->sender, sender) != 0)
    {
      g_free (entry->sender);
      entry->sender = g_strdup (sender);
    }
  return TRUE;
}

DkLauncherEntry
dk_launcher_get (const DkLauncher *launcher, const char *key)
{
  const Entry *entry = g_hash_table_lookup (launcher->entries, key);
  DkLauncherEntry nothing = { 0 };

  return entry != NULL ? entry->shown : nothing;
}

gboolean
dk_launcher_clear_sender (DkLauncher *launcher, const char *sender)
{
  GHashTableIter iter;
  gpointer value;
  gboolean cleared = FALSE;

  g_hash_table_iter_init (&iter, launcher->entries);
  while (g_hash_table_iter_next (&iter, NULL, &value))
    if (g_strcmp0 (((const Entry *) value)->sender, sender) == 0)
      {
        g_hash_table_iter_remove (&iter);
        cleared = TRUE;
      }
  return cleared;
}

gboolean
dk_launcher_clear_id (DkLauncher *launcher, const char *id)
{
  return g_hash_table_remove (launcher->entries, id);
}

void
dk_launcher_sync (DkLauncher *launcher, const GArray *items)
{
  GHashTable *windowed = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  GHashTableIter iter;
  gpointer key;

  for (guint i = 0; i < items->len; i++)
    {
      const DkItem *item = &g_array_index (items, DkItem, i);

      if (item->windows->len > 0)
        g_hash_table_add (windowed, g_strdup (item->key));
    }
  g_hash_table_iter_init (&iter, launcher->windowed);
  while (g_hash_table_iter_next (&iter, &key, NULL))
    if (!g_hash_table_contains (windowed, key))
      dk_launcher_clear_id (launcher, key);
  g_hash_table_unref (launcher->windowed);
  launcher->windowed = windowed;
}

char *
dk_launcher_badge_text (gint64 count)
{
  if (count <= 0)
    return NULL;
  if (count > DK_LAUNCHER_COUNT_CAP)
    return g_strdup_printf ("%d+", DK_LAUNCHER_COUNT_CAP);
  return g_strdup_printf ("%" G_GINT64_FORMAT, count);
}
