/*
 * A Kept folder's folder (CMN-239).
 */

#include "dk-folder.h"

#define CUSTOM_ICON "metadata::custom-icon"

gboolean
dk_folder_exists (GFile *folder)
{
  return g_file_query_file_type (folder, G_FILE_QUERY_INFO_NONE, NULL) == G_FILE_TYPE_DIRECTORY;
}

gboolean
dk_folder_uri_is_folder (const char *uri)
{
  g_autoptr (GFile) file = g_file_new_for_uri (uri);

  return dk_folder_exists (file);
}

char *
dk_folder_display_name (GFile *folder)
{
  g_autoptr (GFileInfo) info = g_file_query_info (folder, G_FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                                                  G_FILE_QUERY_INFO_NONE, NULL, NULL);

  return info != NULL ? g_strdup (g_file_info_get_display_name (info)) : NULL;
}

/* Caja keeps a custom icon as a metadata attribute: the URI of an image, or an icon's name. */
static GIcon *
icon_of (GFileInfo *info)
{
  const char *custom = g_file_info_get_attribute_string (info, CUSTOM_ICON);

  if (custom != NULL && custom[0] != '\0')
    {
      g_autoptr (GFile) file = NULL;

      if (g_str_has_prefix (custom, "file:") || g_path_is_absolute (custom))
        {
          file = g_str_has_prefix (custom, "file:") ? g_file_new_for_uri (custom)
                                                    : g_file_new_for_path (custom);
          if (g_file_query_exists (file, NULL))
            return g_file_icon_new (file);
        }
      else
        return g_themed_icon_new (custom);
    }
  return g_file_info_get_icon (info) != NULL ? g_object_ref (g_file_info_get_icon (info)) : NULL;
}

GIcon *
dk_folder_icon (GFile *folder)
{
  g_autoptr (GFileInfo) info = g_file_query_info (
    folder, G_FILE_ATTRIBUTE_STANDARD_ICON "," CUSTOM_ICON, G_FILE_QUERY_INFO_NONE, NULL, NULL);

  return info != NULL ? icon_of (info) : NULL;
}

void
dk_folder_entry_free (DkFolderEntry *entry)
{
  g_free (entry->uri);
  g_free (entry->name);
  g_clear_object (&entry->icon);
  g_free (entry);
}

static gint
by_kind_then_name (gconstpointer a, gconstpointer b)
{
  const DkFolderEntry *ea = *(DkFolderEntry *const *) a, *eb = *(DkFolderEntry *const *) b;

  if (ea->is_folder != eb->is_folder)
    return ea->is_folder ? -1 : 1;
  return g_utf8_collate (ea->name, eb->name);
}

GPtrArray *
dk_folder_list (GFile *folder, GError **error)
{
  g_autoptr (GFileEnumerator) children = g_file_enumerate_children (
    folder,
    G_FILE_ATTRIBUTE_STANDARD_NAME "," G_FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME ","
    G_FILE_ATTRIBUTE_STANDARD_TYPE "," G_FILE_ATTRIBUTE_STANDARD_ICON ","
    G_FILE_ATTRIBUTE_STANDARD_IS_HIDDEN "," G_FILE_ATTRIBUTE_STANDARD_IS_BACKUP ","
    CUSTOM_ICON,
    G_FILE_QUERY_INFO_NONE, NULL, error);
  g_autoptr (GPtrArray) entries = NULL;
  GFileInfo *info;

  if (children == NULL)
    return NULL;
  entries = g_ptr_array_new_with_free_func ((GDestroyNotify) dk_folder_entry_free);
  while ((info = g_file_enumerator_next_file (children, NULL, error)) != NULL)
    {
      g_autoptr (GFileInfo) owned = info;
      g_autoptr (GFile) child = NULL;
      DkFolderEntry *entry;

      if (g_file_info_get_is_hidden (owned) || g_file_info_get_is_backup (owned))
        continue;
      child = g_file_get_child (folder, g_file_info_get_name (owned));
      entry = g_new0 (DkFolderEntry, 1);
      entry->uri = g_file_get_uri (child);
      entry->name = g_strdup (g_file_info_get_display_name (owned));
      entry->icon = icon_of (owned);
      entry->is_folder = g_file_info_get_file_type (owned) == G_FILE_TYPE_DIRECTORY;
      g_ptr_array_add (entries, entry);
    }
  if (error != NULL && *error != NULL)
    return NULL;
  g_ptr_array_sort (entries, by_kind_then_name);
  return g_steal_pointer (&entries);
}

/* Putting files in. */

static gboolean
same_filesystem (GFile *a, GFile *b)
{
  g_autoptr (GFileInfo) info_a = g_file_query_info (a, G_FILE_ATTRIBUTE_ID_FILESYSTEM,
                                                    G_FILE_QUERY_INFO_NOFOLLOW_SYMLINKS, NULL,
                                                    NULL);
  g_autoptr (GFileInfo) info_b = g_file_query_info (b, G_FILE_ATTRIBUTE_ID_FILESYSTEM,
                                                    G_FILE_QUERY_INFO_NONE, NULL, NULL);

  return info_a != NULL && info_b != NULL
         && g_file_info_get_attribute_string (info_a, G_FILE_ATTRIBUTE_ID_FILESYSTEM) != NULL
         && g_strcmp0 (g_file_info_get_attribute_string (info_a, G_FILE_ATTRIBUTE_ID_FILESYSTEM),
                       g_file_info_get_attribute_string (info_b, G_FILE_ATTRIBUTE_ID_FILESYSTEM))
              == 0;
}

/* g_file_copy doesn't go into folders: this does, and keeps what g_file_copy keeps. */
static gboolean
copy_tree (GFile *source, GFile *target, GError **error)
{
  GFileType type = g_file_query_file_type (source, G_FILE_QUERY_INFO_NOFOLLOW_SYMLINKS, NULL);
  g_autoptr (GFileEnumerator) children = NULL;
  GFileInfo *info;

  if (type != G_FILE_TYPE_DIRECTORY)
    return g_file_copy (source, target, G_FILE_COPY_NOFOLLOW_SYMLINKS | G_FILE_COPY_ALL_METADATA,
                        NULL, NULL, NULL, error);
  if (!g_file_make_directory (target, NULL, error))
    return FALSE;
  children = g_file_enumerate_children (source, G_FILE_ATTRIBUTE_STANDARD_NAME,
                                        G_FILE_QUERY_INFO_NOFOLLOW_SYMLINKS, NULL, error);
  if (children == NULL)
    return FALSE;
  while ((info = g_file_enumerator_next_file (children, NULL, error)) != NULL)
    {
      g_autoptr (GFileInfo) owned = info;
      g_autoptr (GFile) from = g_file_get_child (source, g_file_info_get_name (owned));
      g_autoptr (GFile) to = g_file_get_child (target, g_file_info_get_name (owned));

      if (!copy_tree (from, to, error))
        return FALSE;
    }
  return error == NULL || *error == NULL;
}

typedef struct
{
  char **uris;
  GFile *folder;
  /* Of Failure, in the order the files were. */
  GPtrArray *failures;
} Put;

typedef struct
{
  char   *uri;
  GError *error;
} Failure;

static void
failure_free (Failure *failure)
{
  g_free (failure->uri);
  g_clear_error (&failure->error);
  g_free (failure);
}

static void
put_free (Put *put)
{
  g_strfreev (put->uris);
  g_object_unref (put->folder);
  g_ptr_array_unref (put->failures);
  g_free (put);
}

static void
put_one (Put *put, const char *uri)
{
  g_autoptr (GFile) source = g_file_new_for_uri (uri);
  g_autofree char *name = g_file_get_basename (source);
  g_autoptr (GFile) parent = g_file_get_parent (source);
  g_autoptr (GFile) target = g_file_get_child (put->folder, name);
  GError *error = NULL;
  gboolean done;

  /* Dropped back where it came from. */
  if (parent != NULL && g_file_equal (parent, put->folder))
    return;
  if (same_filesystem (source, put->folder))
    done = g_file_move (source, target, G_FILE_COPY_NOFOLLOW_SYMLINKS | G_FILE_COPY_NO_FALLBACK_FOR_MOVE,
                        NULL, NULL, NULL, &error);
  else
    done = copy_tree (source, target, &error);
  if (!done)
    {
      Failure *failure = g_new0 (Failure, 1);

      failure->uri = g_strdup (uri);
      failure->error = error;
      g_ptr_array_add (put->failures, failure);
    }
}

static void
put_in_thread (GTask *task, gpointer source G_GNUC_UNUSED, gpointer data,
               GCancellable *cancellable G_GNUC_UNUSED)
{
  Put *put = data;

  for (guint i = 0; put->uris[i] != NULL; i++)
    put_one (put, put->uris[i]);
  g_task_return_boolean (task, TRUE);
}

typedef struct
{
  DkFolderPutFailed failed;
  DkFolderPutDone   done;
  gpointer          data;
  GDestroyNotify    destroy;
} Told;

static void
on_put (GObject *source G_GNUC_UNUSED, GAsyncResult *result, gpointer data)
{
  Told *told = data;
  GTask *task = G_TASK (result);
  Put *put = g_task_get_task_data (task);

  if (!g_cancellable_is_cancelled (g_task_get_cancellable (task)))
    {
      for (guint i = 0; told->failed != NULL && i < put->failures->len; i++)
        {
          Failure *failure = put->failures->pdata[i];

          told->failed (failure->uri, failure->error, told->data);
        }
      if (told->done != NULL)
        told->done (told->data);
    }
  if (told->destroy != NULL)
    told->destroy (told->data);
  g_free (told);
}

void
dk_folder_put (GFile *folder, const char *const *uris, GCancellable *cancellable,
               DkFolderPutFailed failed, DkFolderPutDone done, gpointer data,
               GDestroyNotify destroy)
{
  Put *put = g_new0 (Put, 1);
  Told *told = g_new0 (Told, 1);
  g_autoptr (GTask) task = g_task_new (NULL, cancellable, on_put, told);

  put->uris = uris != NULL ? g_strdupv ((char **) uris) : g_new0 (char *, 1);
  put->folder = g_object_ref (folder);
  put->failures = g_ptr_array_new_with_free_func ((GDestroyNotify) failure_free);
  told->failed = failed;
  told->done = done;
  told->data = data;
  told->destroy = destroy;
  g_task_set_task_data (task, put, (GDestroyNotify) put_free);
  g_task_run_in_thread (task, put_in_thread);
}
