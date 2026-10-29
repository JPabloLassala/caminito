/*
 * What the Notifications applet shows (CMN-260). See nt-state.h.
 */

#include "nt-state.h"

static void
action_free (gpointer data)
{
  NtAction *action = data;

  g_free (action->key);
  g_free (action->label);
  g_free (action);
}

void
nt_entry_free (NtEntry *entry)
{
  g_free (entry->app);
  g_free (entry->app_name);
  g_free (entry->app_icon);
  g_free (entry->summary);
  g_free (entry->body);
  g_free (entry->image_icon);
  g_free (entry->reply);
  g_clear_pointer (&entry->image_pixels, g_variant_unref);
  g_ptr_array_unref (entry->actions);
  g_free (entry);
}

static NtEntry *
entry_from_variant (GVariant *variant)
{
  NtEntry *entry = g_new0 (NtEntry, 1);
  g_autoptr (GVariant) image = NULL;
  g_autoptr (GVariantIter) actions = NULL;
  guint64 time;
  char *key, *label;

  g_variant_get (variant, "(usssssva(ss)bbst)", &entry->id, &entry->app, &entry->app_name,
                 &entry->app_icon, &entry->summary, &entry->body, &image, &actions,
                 &entry->has_default, &entry->resident, &entry->reply, &time);
  entry->time = (gint64) time;
  if (g_variant_is_of_type (image, G_VARIANT_TYPE_STRING))
    entry->image_icon = g_variant_dup_string (image, NULL);
  else if (g_variant_is_of_type (image, G_VARIANT_TYPE ("(iiibiiay)")))
    entry->image_pixels = g_variant_ref (image);
  entry->actions = g_ptr_array_new_with_free_func (action_free);
  while (g_variant_iter_next (actions, "(ss)", &key, &label))
    {
      NtAction *action = g_new0 (NtAction, 1);

      action->key = key;
      action->label = label;
      g_ptr_array_add (entry->actions, action);
    }
  return entry;
}

GPtrArray *
nt_entries_from_list (GVariant *reply)
{
  g_autoptr (GVariant) entries = NULL;
  GPtrArray *read_entries;
  GVariantIter iter;
  GVariant *child;

  if (!g_variant_is_of_type (reply, G_VARIANT_TYPE (NT_LIST_TYPE)))
    return NULL;
  entries = g_variant_get_child_value (reply, 0);
  read_entries = g_ptr_array_new_with_free_func ((GDestroyNotify) nt_entry_free);
  g_variant_iter_init (&iter, entries);
  while ((child = g_variant_iter_next_value (&iter)) != NULL)
    {
      g_ptr_array_add (read_entries, entry_from_variant (child));
      g_variant_unref (child);
    }
  return read_entries;
}

static void
group_free (gpointer data)
{
  NtGroup *group = data;

  g_ptr_array_unref (group->entries);
  g_free (group);
}

GPtrArray *
nt_state_groups (const GPtrArray *entries, const char *const *ungrouped)
{
  GPtrArray *groups = g_ptr_array_new_with_free_func (group_free);
  /* Each grouped app's NtGroup, by its key. */
  g_autoptr (GHashTable) by_app = g_hash_table_new (g_str_hash, g_str_equal);

  /* The entries are newest first, so a group is placed by the first entry met. */
  for (guint i = 0; i < entries->len; i++)
    {
      NtEntry *entry = g_ptr_array_index (entries, i);
      gboolean grouped = !g_strv_contains (ungrouped, entry->app);
      NtGroup *group = grouped ? g_hash_table_lookup (by_app, entry->app) : NULL;

      if (group == NULL)
        {
          group = g_new0 (NtGroup, 1);
          group->app = entry->app;
          group->grouped = grouped;
          group->entries = g_ptr_array_new ();
          g_ptr_array_add (groups, group);
          if (grouped)
            g_hash_table_insert (by_app, entry->app, group);
        }
      g_ptr_array_add (group->entries, entry);
    }
  return groups;
}

char *
nt_state_age (gint64 time, gint64 now)
{
  gint64 ago = MAX (now - time, 0);
  gint64 minutes = ago / 60, hours = minutes / 60, days = hours / 24;

  if (minutes == 0)
    return g_strdup ("now");
  if (hours == 0)
    return g_strdup_printf ("%" G_GINT64_FORMAT " min ago", minutes);
  if (days == 0)
    return g_strdup_printf ("%" G_GINT64_FORMAT " h ago", hours);
  if (days == 1)
    return g_strdup ("yesterday");
  return g_strdup_printf ("%" G_GINT64_FORMAT " days ago", days);
}

const char *const *
nt_state_icon_names (void)
{
  static const char *const names[] = {
    "notifications-symbolic",
    "xsi-notifications-symbolic",
    "caminito-bell-symbolic",
    NULL,
  };

  return names;
}

char *
nt_state_tooltip (guint n_entries)
{
  if (n_entries == 0)
    return g_strdup ("No notifications");
  return g_strdup_printf (n_entries == 1 ? "%u notification" : "%u notifications", n_entries);
}
