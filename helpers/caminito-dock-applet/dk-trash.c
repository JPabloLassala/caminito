/*
 * The Dock's Trash (CMN-220).
 */

#include "dk-trash.h"

/* Reference counted: a look or a put under way holds the Trash past dk_trash_free. */
struct _DkTrash
{
  GFile          *location;
  GFileMonitor   *monitor;
  GCancellable   *cancellable;
  gboolean        full;
  /* A look is under way; a change came while it was, and wants another. */
  gboolean        looking;
  gboolean        stale;
  gboolean        emptying;
  DkTrashChanged  changed;
  gpointer        data;
};

static void look (DkTrash *trash);

static void
trash_clear (gpointer data)
{
  DkTrash *trash = data;

  g_clear_object (&trash->monitor);
  g_clear_object (&trash->cancellable);
  g_clear_object (&trash->location);
}

static void
trash_unref (DkTrash *trash)
{
  g_rc_box_release_full (trash, trash_clear);
}

static void
set_full (DkTrash *trash, gboolean full)
{
  if (trash->full == full)
    return;
  trash->full = full;
  if (trash->changed != NULL)
    trash->changed (trash->data);
}

/* A look ends with what it found, and another follows if something changed meanwhile. */
static void
look_done (DkTrash *trash, gboolean full)
{
  trash->looking = FALSE;
  if (g_cancellable_is_cancelled (trash->cancellable))
    return;
  set_full (trash, full);
  if (trash->stale)
    look (trash);
}

static void
on_first_files (GObject *source, GAsyncResult *result, gpointer data)
{
  DkTrash *trash = data;
  GFileEnumerator *enumerator = G_FILE_ENUMERATOR (source);
  GList *files = g_file_enumerator_next_files_finish (enumerator, result, NULL);

  g_file_enumerator_close_async (enumerator, G_PRIORITY_DEFAULT, NULL, NULL, NULL);
  look_done (trash, files != NULL);
  g_list_free_full (files, g_object_unref);
  trash_unref (trash);
}

static void
on_enumerated (GObject *source, GAsyncResult *result, gpointer data)
{
  DkTrash *trash = data;
  GFileEnumerator *enumerator = g_file_enumerate_children_finish (G_FILE (source), result, NULL);

  if (enumerator == NULL)
    {
      /* Where there is no Trash to read, such as without gvfs, there is nothing in it. */
      look_done (trash, FALSE);
      trash_unref (trash);
      return;
    }
  g_file_enumerator_next_files_async (enumerator, 1, G_PRIORITY_DEFAULT, trash->cancellable,
                                      on_first_files, trash);
  g_object_unref (enumerator);
}

/* Whether it holds anything: the first entry says. */
static void
look (DkTrash *trash)
{
  if (trash->looking)
    {
      trash->stale = TRUE;
      return;
    }
  trash->looking = TRUE;
  trash->stale = FALSE;
  g_file_enumerate_children_async (trash->location, G_FILE_ATTRIBUTE_STANDARD_NAME,
                                   G_FILE_QUERY_INFO_NOFOLLOW_SYMLINKS, G_PRIORITY_DEFAULT,
                                   trash->cancellable, on_enumerated, g_rc_box_acquire (trash));
}

static void
on_directory_changed (GFileMonitor *monitor G_GNUC_UNUSED, GFile *file G_GNUC_UNUSED,
                      GFile *other G_GNUC_UNUSED, GFileMonitorEvent event G_GNUC_UNUSED,
                      gpointer data)
{
  look (data);
}

DkTrash *
dk_trash_new (GFile *location)
{
  DkTrash *trash = g_rc_box_new0 (DkTrash);

  trash->location = g_object_ref (location);
  trash->cancellable = g_cancellable_new ();
  trash->monitor = g_file_monitor_directory (location, G_FILE_MONITOR_NONE, NULL, NULL);
  if (trash->monitor != NULL)
    g_signal_connect (trash->monitor, "changed", G_CALLBACK (on_directory_changed), trash);
  look (trash);
  return trash;
}

void
dk_trash_free (DkTrash *trash)
{
  if (trash == NULL)
    return;
  trash->changed = NULL;
  g_cancellable_cancel (trash->cancellable);
  if (trash->monitor != NULL)
    {
      g_signal_handlers_disconnect_by_data (trash->monitor, trash);
      g_file_monitor_cancel (trash->monitor);
    }
  trash_unref (trash);
}

void
dk_trash_set_changed (DkTrash *trash, DkTrashChanged changed, gpointer data)
{
  trash->changed = changed;
  trash->data = data;
}

gboolean
dk_trash_is_full (DkTrash *trash)
{
  return trash->full;
}

gboolean
dk_trash_is_emptying (DkTrash *trash)
{
  return trash->emptying;
}

/* Deletes what is in the folder, each child at once where it can be, and then the folder
   itself unless it is the Trash's own. gvfs's Trash deletes a trashed folder whole, and
   refuses to delete what is in one, so only a folder that isn't empty, as a plain directory
   is, is gone into first. Goes on past an error and keeps the first. */
static void
delete_tree (GFile *file, gboolean keep, GError **first)
{
  g_autoptr (GFileEnumerator) children = NULL;
  g_autoptr (GError) error = NULL;
  GFileInfo *info;

  children = g_file_enumerate_children (file, G_FILE_ATTRIBUTE_STANDARD_NAME ","
                                              G_FILE_ATTRIBUTE_STANDARD_TYPE,
                                        G_FILE_QUERY_INFO_NOFOLLOW_SYMLINKS, NULL, &error);
  if (children != NULL)
    while ((info = g_file_enumerator_next_file (children, NULL, NULL)) != NULL)
      {
        g_autoptr (GFile) child = g_file_get_child (file, g_file_info_get_name (info));

        if (!g_file_delete (child, NULL, &error))
          {
            if (g_file_info_get_file_type (info) == G_FILE_TYPE_DIRECTORY
                && g_error_matches (error, G_IO_ERROR, G_IO_ERROR_NOT_EMPTY))
              {
                g_clear_error (&error);
                delete_tree (child, FALSE, first);
              }
            else if (*first == NULL)
              *first = g_steal_pointer (&error);
            g_clear_error (&error);
          }
        g_object_unref (info);
      }
  g_clear_error (&error);
  if (!keep && !g_file_delete (file, NULL, &error))
    {
      if (*first == NULL)
        *first = g_steal_pointer (&error);
    }
}

static void
empty_in_thread (GTask *task, gpointer source G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED,
                 GCancellable *cancellable G_GNUC_UNUSED)
{
  GFile *location = g_task_get_task_data (task);
  GError *first = NULL;

  delete_tree (location, TRUE, &first);
  if (first != NULL)
    g_task_return_error (task, first);
  else
    g_task_return_boolean (task, TRUE);
}

typedef struct
{
  DkTrash        *trash;
  DkTrashEmptied  done;
  gpointer        data;
} Emptying;

static void
on_emptied (GObject *source G_GNUC_UNUSED, GAsyncResult *result, gpointer data)
{
  Emptying *emptying = data;
  g_autoptr (GError) error = NULL;

  g_task_propagate_boolean (G_TASK (result), &error);
  emptying->trash->emptying = FALSE;
  /* Nothing is told once the Trash has been let go. */
  if (!g_cancellable_is_cancelled (emptying->trash->cancellable))
    {
      /* Whatever the monitor saw, the Trash is looked at now it is done. */
      look (emptying->trash);
      if (emptying->done != NULL)
        emptying->done (error, emptying->data);
    }
  trash_unref (emptying->trash);
  g_free (emptying);
}

void
dk_trash_empty (DkTrash *trash, DkTrashEmptied done, gpointer data)
{
  Emptying *emptying = g_new0 (Emptying, 1);
  g_autoptr (GTask) task = g_task_new (NULL, NULL, on_emptied, emptying);

  trash->emptying = TRUE;
  emptying->trash = g_rc_box_acquire (trash);
  emptying->done = done;
  emptying->data = data;
  g_task_set_task_data (task, g_object_ref (trash->location), g_object_unref);
  g_task_run_in_thread (task, empty_in_thread);
}

typedef struct
{
  DkTrash          *trash;
  DkTrashPutFailed  failed;
  DkTrashPutDone    done;
  gpointer          data;
  guint             remaining;
} Putting;

static void
put_finished (Putting *putting)
{
  if (--putting->remaining > 0)
    return;
  if (!g_cancellable_is_cancelled (putting->trash->cancellable))
    {
      look (putting->trash);
      if (putting->done != NULL)
        putting->done (putting->data);
    }
  trash_unref (putting->trash);
  g_free (putting);
}

static void
on_trashed (GObject *source, GAsyncResult *result, gpointer data)
{
  Putting *putting = data;
  g_autoptr (GError) error = NULL;

  if (!g_file_trash_finish (G_FILE (source), result, &error) && putting->failed != NULL
      && !g_cancellable_is_cancelled (putting->trash->cancellable))
    {
      g_autofree char *uri = g_file_get_uri (G_FILE (source));

      putting->failed (uri, error, putting->data);
    }
  put_finished (putting);
}

void
dk_trash_put (DkTrash *trash, const char *const *uris, DkTrashPutFailed failed,
              DkTrashPutDone done, gpointer data)
{
  Putting *putting = g_new0 (Putting, 1);

  putting->trash = g_rc_box_acquire (trash);
  putting->failed = failed;
  putting->done = done;
  putting->data = data;
  /* Counts the loop itself, so that no file finishing early ends the put. */
  putting->remaining = 1;
  for (guint i = 0; uris != NULL && uris[i] != NULL; i++)
    {
      g_autoptr (GFile) file = g_file_new_for_uri (uris[i]);

      putting->remaining++;
      g_file_trash_async (file, G_PRIORITY_DEFAULT, NULL, on_trashed, putting);
    }
  put_finished (putting);
}
