/*
 * The app id to desktop entry resolution (CMN-219), which a Dock item is made from.
 */

#include <gio/gdesktopappinfo.h>

#include "dk-resolve.h"

typedef struct
{
  char *id;
  /* NULL where the entry has none. */
  char *wm_class;
} Entry;

struct _DkCatalog
{
  GPtrArray *entries;
};

static void
entry_free (gpointer data)
{
  Entry *entry = data;

  g_free (entry->id);
  g_free (entry->wm_class);
  g_free (entry);
}

DkCatalog *
dk_catalog_new (void)
{
  DkCatalog *catalog = g_new0 (DkCatalog, 1);

  catalog->entries = g_ptr_array_new_with_free_func (entry_free);
  return catalog;
}

void
dk_catalog_free (DkCatalog *catalog)
{
  if (catalog == NULL)
    return;
  g_ptr_array_unref (catalog->entries);
  g_free (catalog);
}

void
dk_catalog_add (DkCatalog *catalog, const char *id, const char *startup_wm_class)
{
  Entry *entry;

  for (guint i = 0; i < catalog->entries->len; i++)
    if (g_strcmp0 (((Entry *) catalog->entries->pdata[i])->id, id) == 0)
      return;
  entry = g_new0 (Entry, 1);
  entry->id = g_strdup (id);
  if (startup_wm_class != NULL && startup_wm_class[0] != '\0')
    entry->wm_class = g_strdup (startup_wm_class);
  g_ptr_array_add (catalog->entries, entry);
}

DkCatalog *
dk_catalog_new_installed (void)
{
  DkCatalog *catalog = dk_catalog_new ();
  GList *all = g_app_info_get_all ();

  for (GList *l = all; l != NULL; l = l->next)
    {
      GAppInfo *info = l->data;

      if (!G_IS_DESKTOP_APP_INFO (info) || g_app_info_get_id (info) == NULL)
        continue;
      dk_catalog_add (catalog, g_app_info_get_id (info),
                      g_desktop_app_info_get_startup_wm_class (G_DESKTOP_APP_INFO (info)));
    }
  g_list_free_full (all, g_object_unref);
  return catalog;
}

const char *
dk_catalog_resolve (const DkCatalog *catalog, const char *app_id)
{
  g_autofree char *stem = NULL;

  if (app_id == NULL || app_id[0] == '\0')
    return NULL;
  stem = g_str_has_suffix (app_id, ".desktop")
           ? g_strndup (app_id, strlen (app_id) - strlen (".desktop"))
           : g_strdup (app_id);
  for (guint i = 0; i < catalog->entries->len; i++)
    {
      const Entry *entry = catalog->entries->pdata[i];
      gsize id_length = strlen (entry->id) - strlen (".desktop");

      if (g_str_has_suffix (entry->id, ".desktop") && id_length == strlen (stem)
          && g_ascii_strncasecmp (entry->id, stem, id_length) == 0)
        return entry->id;
    }
  for (guint i = 0; i < catalog->entries->len; i++)
    {
      const Entry *entry = catalog->entries->pdata[i];

      if (entry->wm_class != NULL && g_ascii_strcasecmp (entry->wm_class, app_id) == 0)
        return entry->id;
    }
  return NULL;
}
