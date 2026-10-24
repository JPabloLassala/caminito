/*
 * The Dock's Removable drive items (CMN-240).
 */

#include "dk-drive.h"

/* gvfs's phones and cameras, which have no drive. */
static const char *const DEVICE_SCHEMES[] = { "mtp", "gphoto2", "afc", NULL };

/* gvfs's network shares, which can have no volume to say they are a network's. */
static const char *const NETWORK_SCHEMES[] = { "smb", "sftp", "ssh", "ftp", "ftps", "dav",
                                               "davs", "afp", "nfs", "network", "dns-sd",
                                               "google-drive", "onedrive", NULL };

static gboolean
is_network (GMount *mount, const char *scheme)
{
  g_autoptr (GVolume) volume = g_mount_get_volume (mount);
  g_autofree char *class = volume != NULL
    ? g_volume_get_identifier (volume, G_VOLUME_IDENTIFIER_KIND_CLASS) : NULL;

  return g_strcmp0 (class, "network") == 0
         || (scheme != NULL && g_strv_contains (NETWORK_SCHEMES, scheme));
}

gboolean
dk_drive_is_removable (GMount *mount)
{
  g_autoptr (GFile) root = NULL;
  g_autofree char *scheme = NULL;
  g_autoptr (GDrive) drive = NULL;

  if (g_mount_is_shadowed (mount) || !(g_mount_can_eject (mount) || g_mount_can_unmount (mount)))
    return FALSE;
  root = g_mount_get_root (mount);
  scheme = g_file_get_uri_scheme (root);
  if (is_network (mount, scheme))
    return FALSE;
  drive = g_mount_get_drive (mount);
  /* An internal disk's drive is neither removable nor ejectable, as udisks tells it. */
  if (drive != NULL)
    return g_drive_is_removable (drive) || g_drive_is_media_removable (drive)
           || g_drive_can_eject (drive);
  return scheme != NULL && g_strv_contains (DEVICE_SCHEMES, scheme);
}

/* The list. */

struct _DkDrives
{
  GVolumeMonitor  *monitor;
  /* Of GMount, in the order they were mounted, and their keys. */
  GPtrArray       *mounts;
  GStrv            keys;
  DkDrivesChanged  changed;
  gpointer         data;
};

static char *
key_of (GMount *mount)
{
  g_autoptr (GFile) root = g_mount_get_root (mount);

  return g_file_get_uri (root);
}

/* The mounts that stay keep their places, and those that are new come after them. */
static void
drives_sync (DkDrives *drives)
{
  GList *now = g_volume_monitor_get_mounts (drives->monitor);
  GPtrArray *mounts = g_ptr_array_new_with_free_func (g_object_unref);
  GStrvBuilder *keys = g_strv_builder_new ();

  for (guint i = 0; i < drives->mounts->len; i++)
    if (g_list_find (now, drives->mounts->pdata[i]) != NULL
        && dk_drive_is_removable (drives->mounts->pdata[i]))
      g_ptr_array_add (mounts, g_object_ref (drives->mounts->pdata[i]));
  for (GList *l = now; l != NULL; l = l->next)
    if (!g_ptr_array_find (mounts, l->data, NULL) && dk_drive_is_removable (l->data))
      g_ptr_array_add (mounts, g_object_ref (l->data));
  for (guint i = 0; i < mounts->len; i++)
    g_strv_builder_take (keys, key_of (mounts->pdata[i]));
  g_list_free_full (now, g_object_unref);
  g_ptr_array_unref (drives->mounts);
  drives->mounts = mounts;
  g_strfreev (drives->keys);
  drives->keys = g_strv_builder_end (keys);
  g_strv_builder_unref (keys);
}

static void
on_mounts_changed (GVolumeMonitor *monitor G_GNUC_UNUSED, GMount *mount G_GNUC_UNUSED,
                   gpointer data)
{
  DkDrives *drives = data;

  drives_sync (drives);
  if (drives->changed != NULL)
    drives->changed (drives->data);
}

DkDrives *
dk_drives_new (GVolumeMonitor *monitor)
{
  static const char *const signals[] = { "mount-added", "mount-removed", "mount-changed", NULL };
  DkDrives *drives = g_new0 (DkDrives, 1);

  drives->monitor = g_object_ref (monitor);
  drives->mounts = g_ptr_array_new_with_free_func (g_object_unref);
  for (guint i = 0; signals[i] != NULL; i++)
    g_signal_connect (monitor, signals[i], G_CALLBACK (on_mounts_changed), drives);
  drives_sync (drives);
  return drives;
}

void
dk_drives_free (DkDrives *drives)
{
  if (drives == NULL)
    return;
  g_signal_handlers_disconnect_by_data (drives->monitor, drives);
  g_object_unref (drives->monitor);
  g_ptr_array_unref (drives->mounts);
  g_strfreev (drives->keys);
  g_free (drives);
}

void
dk_drives_set_changed (DkDrives *drives, DkDrivesChanged changed, gpointer data)
{
  drives->changed = changed;
  drives->data = data;
}

const char *const *
dk_drives_get_keys (DkDrives *drives)
{
  return (const char *const *) drives->keys;
}

GMount *
dk_drives_lookup (DkDrives *drives, const char *key)
{
  for (guint i = 0; key != NULL && drives->keys[i] != NULL; i++)
    if (g_strcmp0 (drives->keys[i], key) == 0)
      return drives->mounts->pdata[i];
  return NULL;
}

/* Ejecting or unmounting. */

typedef struct
{
  GCancellable   *cancellable;
  /* Asked once: the mount may say otherwise once it has gone. */
  gboolean        eject;
  DkDriveRemoved  done;
  gpointer        data;
  GDestroyNotify  destroy;
} Removing;

static void
on_removed (GObject *source, GAsyncResult *result, gpointer data)
{
  Removing *removing = data;
  g_autoptr (GError) error = NULL;
  GMount *mount = G_MOUNT (source);

  if (!(removing->eject ? g_mount_eject_with_operation_finish (mount, result, &error)
                                  : g_mount_unmount_with_operation_finish (mount, result, &error))
      && g_error_matches (error, G_IO_ERROR, G_IO_ERROR_FAILED_HANDLED))
    g_clear_error (&error);
  if (removing->done != NULL
      && (removing->cancellable == NULL || !g_cancellable_is_cancelled (removing->cancellable)))
    removing->done (error, removing->data);
  if (removing->destroy != NULL)
    removing->destroy (removing->data);
  g_clear_object (&removing->cancellable);
  g_free (removing);
}

void
dk_drive_remove (GMount *mount, GCancellable *cancellable, DkDriveRemoved done, gpointer data,
                 GDestroyNotify destroy)
{
  Removing *removing = g_new0 (Removing, 1);

  removing->cancellable = cancellable != NULL ? g_object_ref (cancellable) : NULL;
  removing->done = done;
  removing->data = data;
  removing->destroy = destroy;
  removing->eject = g_mount_can_eject (mount);
  /* No mount operation: the dock shows the error itself, as the Trash does. */
  if (removing->eject)
    g_mount_eject_with_operation (mount, G_MOUNT_UNMOUNT_NONE, NULL, NULL, on_removed, removing);
  else
    g_mount_unmount_with_operation (mount, G_MOUNT_UNMOUNT_NONE, NULL, NULL, on_removed,
                                    removing);
}

void
dk_drive_put (GMount *mount, const char *const *uris, GCancellable *cancellable,
              DkFolderPutFailed failed, DkFolderPutDone done, gpointer data,
              GDestroyNotify destroy)
{
  g_autoptr (GFile) root = g_mount_get_root (mount);

  dk_folder_copy (root, uris, cancellable, failed, done, data, destroy);
}
