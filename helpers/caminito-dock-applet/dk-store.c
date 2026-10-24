/*
 * The kept Dock items in the applet's settings (CMN-219), and the Kept folders (CMN-239).
 */

#include "dk-model.h"
#include "dk-store.h"

/* One ordered list of keys, kept in the settings key of its name. */
typedef struct
{
  const char *name;
  GPtrArray  *kept;
  /* kept as the NULL-terminated array dk_store_get hands out. */
  GPtrArray  *strv;
} List;

struct _DkStore
{
  GSettings      *settings;
  gulong          handler;
  List            items;
  List            folders;
  /* The store is writing the settings: their change signal is its own. */
  gboolean        writing;
  DkStoreChanged  changed;
  gpointer        data;
};

static void
list_init (List *list, const char *name)
{
  list->name = name;
  list->kept = g_ptr_array_new_with_free_func (g_free);
  list->strv = g_ptr_array_new ();
  g_ptr_array_add (list->strv, NULL);
}

static void
list_clear (List *list)
{
  g_ptr_array_unref (list->kept);
  g_ptr_array_unref (list->strv);
}

static void
rebuild_strv (List *list)
{
  g_ptr_array_set_size (list->strv, 0);
  for (guint i = 0; i < list->kept->len; i++)
    g_ptr_array_add (list->strv, list->kept->pdata[i]);
  g_ptr_array_add (list->strv, NULL);
}

static void
load (DkStore *store, List *list)
{
  g_auto (GStrv) keys = g_settings_get_strv (store->settings, list->name);

  g_ptr_array_set_size (list->kept, 0);
  for (guint i = 0; keys[i] != NULL; i++)
    dk_kept_add (list->kept, keys[i]);
  rebuild_strv (list);
}

static gboolean
same (List *list, const char *const *keys)
{
  guint i = 0;

  for (; keys[i] != NULL; i++)
    if (i >= list->kept->len || g_strcmp0 (keys[i], list->kept->pdata[i]) != 0)
      return FALSE;
  return i == list->kept->len;
}

static void
on_settings_changed (GSettings *settings, const char *key, gpointer data)
{
  DkStore *store = data;
  List *list = g_strcmp0 (key, store->folders.name) == 0 ? &store->folders : &store->items;
  g_auto (GStrv) keys = NULL;

  if (store->writing)
    return;
  keys = g_settings_get_strv (settings, list->name);
  if (same (list, (const char *const *) keys))
    return;
  load (store, list);
  if (store->changed != NULL)
    store->changed (store->data);
}

DkStore *
dk_store_new (GSettings *settings)
{
  DkStore *store = g_new0 (DkStore, 1);

  list_init (&store->items, "kept");
  list_init (&store->folders, "kept-folders");
  if (settings != NULL)
    {
      store->settings = g_object_ref (settings);
      store->handler = g_signal_connect (settings, "changed", G_CALLBACK (on_settings_changed),
                                         store);
      load (store, &store->items);
      load (store, &store->folders);
    }
  return store;
}

void
dk_store_free (DkStore *store)
{
  if (store == NULL)
    return;
  if (store->settings != NULL)
    {
      g_signal_handler_disconnect (store->settings, store->handler);
      g_object_unref (store->settings);
    }
  list_clear (&store->items);
  list_clear (&store->folders);
  g_free (store);
}

void
dk_store_set_changed (DkStore *store, DkStoreChanged changed, gpointer data)
{
  store->changed = changed;
  store->data = data;
}

const char *const *
dk_store_get (DkStore *store)
{
  return (const char *const *) store->items.strv->pdata;
}

const char *const *
dk_store_get_folders (DkStore *store)
{
  return (const char *const *) store->folders.strv->pdata;
}

gboolean
dk_store_is_kept (DkStore *store, const char *key)
{
  return g_strv_contains (dk_store_get (store), key);
}

gboolean
dk_store_is_folder_kept (DkStore *store, const char *uri)
{
  return g_strv_contains (dk_store_get_folders (store), uri);
}

/* Writes the change to the settings, and tells the applet. */
static gboolean
commit (DkStore *store, List *list, gboolean changed)
{
  if (!changed)
    return FALSE;
  rebuild_strv (list);
  if (store->settings != NULL)
    {
      store->writing = TRUE;
      g_settings_set_strv (store->settings, list->name,
                           (const char *const *) list->strv->pdata);
      store->writing = FALSE;
    }
  if (store->changed != NULL)
    store->changed (store->data);
  return TRUE;
}

gboolean
dk_store_keep (DkStore *store, const char *key)
{
  return commit (store, &store->items, dk_kept_add (store->items.kept, key));
}

gboolean
dk_store_remove (DkStore *store, const char *key)
{
  return commit (store, &store->items, dk_kept_remove (store->items.kept, key));
}

gboolean
dk_store_move (DkStore *store, const char *key, guint position)
{
  return commit (store, &store->items, dk_kept_move (store->items.kept, key, position));
}

gboolean
dk_store_keep_folder (DkStore *store, const char *uri)
{
  return commit (store, &store->folders, dk_kept_add (store->folders.kept, uri));
}

gboolean
dk_store_remove_folder (DkStore *store, const char *uri)
{
  return commit (store, &store->folders, dk_kept_remove (store->folders.kept, uri));
}

gboolean
dk_store_move_folder (DkStore *store, const char *uri, guint position)
{
  return commit (store, &store->folders, dk_kept_move (store->folders.kept, uri, position));
}
