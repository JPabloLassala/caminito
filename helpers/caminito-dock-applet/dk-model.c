/*
 * The Dock's items, and what a click on one does (CMN-219).
 */

#include "dk-model.h"

/* The prefix of an item's key that has no desktop entry, before its app id. */
#define WINDOW_KEY_PREFIX "window:"

static void
window_clear (DkWindow *window)
{
  g_free ((char *) window->app_id);
  g_free ((char *) window->title);
}

static gint
by_most_recent (gconstpointer a, gconstpointer b)
{
  const DkWindow *wa = a, *wb = b;

  if (wa->activated != wb->activated)
    return wa->activated < wb->activated ? 1 : -1;
  return wa->opened < wb->opened ? 1 : wa->opened > wb->opened ? -1 : 0;
}

void
dk_item_clear (DkItem *item)
{
  g_free (item->key);
  g_clear_pointer (&item->windows, g_array_unref);
}

static DkItem *
item_find (GArray *items, const char *key)
{
  for (guint i = 0; i < items->len; i++)
    if (g_strcmp0 (g_array_index (items, DkItem, i).key, key) == 0)
      return &g_array_index (items, DkItem, i);
  return NULL;
}

static DkItem *
item_add (GArray *items, const char *key, gboolean is_entry, gboolean kept)
{
  DkItem item = { .key = g_strdup (key), .kept = kept,
                  .windows = g_array_new (FALSE, TRUE, sizeof (DkWindow)) };

  g_array_set_clear_func (item.windows, (GDestroyNotify) window_clear);
  item.entry_id = is_entry ? item.key : NULL;
  g_array_append_val (items, item);
  return &g_array_index (items, DkItem, items->len - 1);
}

/* The first time any window of the item opened. */
static guint64
item_opened (const DkItem *item)
{
  guint64 opened = G_MAXUINT64;

  for (guint i = 0; i < item->windows->len; i++)
    opened = MIN (opened, g_array_index (item->windows, DkWindow, i).opened);
  return opened;
}

static gint
by_opened (gconstpointer a, gconstpointer b, gpointer data G_GNUC_UNUSED)
{
  guint64 oa = item_opened (a), ob = item_opened (b);

  return oa < ob ? -1 : oa > ob ? 1 : 0;
}

GArray *
dk_items_build (const DkCatalog *catalog, const char *const *kept,
                const DkWindow *windows, guint n_windows)
{
  GArray *items = g_array_new (FALSE, TRUE, sizeof (DkItem));
  guint n_kept;

  g_array_set_clear_func (items, (GDestroyNotify) dk_item_clear);
  for (guint i = 0; kept != NULL && kept[i] != NULL; i++)
    if (item_find (items, kept[i]) == NULL)
      item_add (items, kept[i], TRUE, TRUE);
  n_kept = items->len;
  for (guint i = 0; i < n_windows; i++)
    {
      const DkWindow *window = &windows[i];
      const char *entry = dk_catalog_resolve (catalog, window->app_id);
      g_autofree char *key = NULL;
      DkItem *item;
      DkWindow copy = *window;

      if (entry == NULL)
        key = g_strconcat (WINDOW_KEY_PREFIX, window->app_id != NULL ? window->app_id : "", NULL);
      item = item_find (items, entry != NULL ? entry : key);
      if (item == NULL)
        item = item_add (items, entry != NULL ? entry : key, entry != NULL, FALSE);
      copy.app_id = g_strdup (window->app_id);
      copy.title = g_strdup (window->title);
      g_array_append_val (item->windows, copy);
    }
  for (guint i = 0; i < items->len; i++)
    g_array_sort (g_array_index (items, DkItem, i).windows, by_most_recent);
  /* The kept items keep the user's order, the running ones the order they opened in. */
  for (guint i = n_kept + 1; i < items->len; i++)
    for (guint j = i; j > n_kept; j--)
      {
        DkItem *a = &g_array_index (items, DkItem, j - 1), *b = &g_array_index (items, DkItem, j);
        DkItem swap;

        if (by_opened (a, b, NULL) <= 0)
          break;
        swap = *a;
        *a = *b;
        *b = swap;
      }
  return items;
}

gboolean
dk_key_is_entry (const char *key)
{
  return !g_str_has_prefix (key, WINDOW_KEY_PREFIX);
}

DkClick
dk_item_click (const DkItem *item, const DkWindow **target)
{
  const DkWindow *recent;

  if (item->windows->len == 0)
    {
      *target = NULL;
      return DK_CLICK_LAUNCH;
    }
  recent = &g_array_index (item->windows, DkWindow, 0);
  *target = recent;
  if (item->windows->len == 1 && recent->active && !recent->minimised)
    return DK_CLICK_MINIMISE;
  return DK_CLICK_ACTIVATE;
}

static int
kept_index (GPtrArray *kept, const char *key)
{
  for (guint i = 0; i < kept->len; i++)
    if (g_strcmp0 (kept->pdata[i], key) == 0)
      return i;
  return -1;
}

gboolean
dk_kept_add (GPtrArray *kept, const char *key)
{
  if (kept_index (kept, key) >= 0)
    return FALSE;
  g_ptr_array_add (kept, g_strdup (key));
  return TRUE;
}

gboolean
dk_kept_remove (GPtrArray *kept, const char *key)
{
  int index = kept_index (kept, key);

  if (index < 0)
    return FALSE;
  g_ptr_array_remove_index (kept, index);
  return TRUE;
}

gboolean
dk_kept_move (GPtrArray *kept, const char *key, guint position)
{
  int index = kept_index (kept, key);
  char *moved;

  if (index >= 0 && (position == (guint) index || position == (guint) index + 1))
    return FALSE;
  if (index >= 0)
    {
      moved = g_ptr_array_steal_index (kept, index);
      if (position > (guint) index)
        position--;
    }
  else
    moved = g_strdup (key);
  g_ptr_array_insert (kept, MIN (position, kept->len), moved);
  return TRUE;
}
