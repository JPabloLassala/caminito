/*
 * The kept Dock items in the applet's settings (CMN-219).
 */

#include "dk-model.h"
#include "dk-store.h"

struct _DkStore
{
  GSettings      *settings;
  gulong          handler;
  GPtrArray      *kept;
  /* kept as the NULL-terminated array dk_store_get hands out. */
  GPtrArray      *strv;
  /* The store is writing the settings: their change signal is its own. */
  gboolean        writing;
  DkStoreChanged  changed;
  gpointer        data;
};

static void
rebuild_strv (DkStore *store)
{
  g_ptr_array_set_size (store->strv, 0);
  for (guint i = 0; i < store->kept->len; i++)
    g_ptr_array_add (store->strv, store->kept->pdata[i]);
  g_ptr_array_add (store->strv, NULL);
}

static void
load (DkStore *store)
{
  g_auto (GStrv) keys = g_settings_get_strv (store->settings, "kept");

  g_ptr_array_set_size (store->kept, 0);
  for (guint i = 0; keys[i] != NULL; i++)
    dk_kept_add (store->kept, keys[i]);
  rebuild_strv (store);
}

static gboolean
same (DkStore *store, const char *const *keys)
{
  guint i = 0;

  for (; keys[i] != NULL; i++)
    if (i >= store->kept->len || g_strcmp0 (keys[i], store->kept->pdata[i]) != 0)
      return FALSE;
  return i == store->kept->len;
}

static void
on_settings_changed (GSettings *settings, const char *key G_GNUC_UNUSED, gpointer data)
{
  DkStore *store = data;
  g_auto (GStrv) keys = NULL;

  if (store->writing)
    return;
  keys = g_settings_get_strv (settings, "kept");
  if (same (store, (const char *const *) keys))
    return;
  load (store);
  if (store->changed != NULL)
    store->changed (store->data);
}

DkStore *
dk_store_new (GSettings *settings)
{
  DkStore *store = g_new0 (DkStore, 1);

  store->kept = g_ptr_array_new_with_free_func (g_free);
  store->strv = g_ptr_array_new ();
  if (settings != NULL)
    {
      store->settings = g_object_ref (settings);
      store->handler = g_signal_connect (settings, "changed::kept",
                                         G_CALLBACK (on_settings_changed), store);
      load (store);
    }
  else
    rebuild_strv (store);
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
  g_ptr_array_unref (store->kept);
  g_ptr_array_unref (store->strv);
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
  return (const char *const *) store->strv->pdata;
}

gboolean
dk_store_is_kept (DkStore *store, const char *key)
{
  return g_strv_contains (dk_store_get (store), key);
}

/* Writes the change to the settings, and tells the applet. */
static gboolean
commit (DkStore *store, gboolean changed)
{
  if (!changed)
    return FALSE;
  rebuild_strv (store);
  if (store->settings != NULL)
    {
      store->writing = TRUE;
      g_settings_set_strv (store->settings, "kept", dk_store_get (store));
      store->writing = FALSE;
    }
  if (store->changed != NULL)
    store->changed (store->data);
  return TRUE;
}

gboolean
dk_store_keep (DkStore *store, const char *key)
{
  return commit (store, dk_kept_add (store->kept, key));
}

gboolean
dk_store_remove (DkStore *store, const char *key)
{
  return commit (store, dk_kept_remove (store->kept, key));
}

gboolean
dk_store_move (DkStore *store, const char *key, guint position)
{
  return commit (store, dk_kept_move (store->kept, key, position));
}
