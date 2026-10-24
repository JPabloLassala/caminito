/*
 * The Dock's Removable drive items (CMN-240), without a panel: which mounts have one, the list
 * following the mounts as disks come and go, ejecting or unmounting one, and copying files to
 * it. GIO's mounts are faked here, with the drive, the volume and the volume monitor behind
 * them, as udisks and gvfs would show a USB disk, an SD card, a disc, a phone, an internal disk
 * and a network share.
 */

#include <gio/gio.h>
#include <glib/gstdio.h>

#include "dk-drive.h"

/* A drive, as udisks shows one. */

#define FAKE_TYPE_DRIVE (fake_drive_get_type ())
G_DECLARE_FINAL_TYPE (FakeDrive, fake_drive, FAKE, DRIVE, GObject)

struct _FakeDrive
{
  GObject  parent;
  gboolean removable;
  gboolean media_removable;
  gboolean can_eject;
};

static gboolean
fake_drive_is_removable (GDrive *drive)
{
  return FAKE_DRIVE (drive)->removable;
}

static gboolean
fake_drive_is_media_removable (GDrive *drive)
{
  return FAKE_DRIVE (drive)->media_removable;
}

static gboolean
fake_drive_can_eject (GDrive *drive)
{
  return FAKE_DRIVE (drive)->can_eject;
}

static void
fake_drive_iface_init (GDriveIface *iface)
{
  iface->is_removable = fake_drive_is_removable;
  iface->is_media_removable = fake_drive_is_media_removable;
  iface->can_eject = fake_drive_can_eject;
}

G_DEFINE_TYPE_WITH_CODE (FakeDrive, fake_drive, G_TYPE_OBJECT,
                         G_IMPLEMENT_INTERFACE (G_TYPE_DRIVE, fake_drive_iface_init))

static void
fake_drive_class_init (FakeDriveClass *klass G_GNUC_UNUSED)
{
}

static void
fake_drive_init (FakeDrive *drive G_GNUC_UNUSED)
{
}

static GDrive *
drive_new (gboolean removable, gboolean media_removable, gboolean can_eject)
{
  FakeDrive *drive = g_object_new (FAKE_TYPE_DRIVE, NULL);

  drive->removable = removable;
  drive->media_removable = media_removable;
  drive->can_eject = can_eject;
  return G_DRIVE (drive);
}

/* A volume, which only says its class. */

#define FAKE_TYPE_VOLUME (fake_volume_get_type ())
G_DECLARE_FINAL_TYPE (FakeVolume, fake_volume, FAKE, VOLUME, GObject)

struct _FakeVolume
{
  GObject  parent;
  char    *class;
};

static char *
fake_volume_get_identifier (GVolume *volume, const char *kind)
{
  return g_strcmp0 (kind, G_VOLUME_IDENTIFIER_KIND_CLASS) == 0
           ? g_strdup (FAKE_VOLUME (volume)->class) : NULL;
}

static void
fake_volume_iface_init (GVolumeIface *iface)
{
  iface->get_identifier = fake_volume_get_identifier;
}

G_DEFINE_TYPE_WITH_CODE (FakeVolume, fake_volume, G_TYPE_OBJECT,
                         G_IMPLEMENT_INTERFACE (G_TYPE_VOLUME, fake_volume_iface_init))

static void
fake_volume_finalize (GObject *object)
{
  g_free (FAKE_VOLUME (object)->class);
  G_OBJECT_CLASS (fake_volume_parent_class)->finalize (object);
}

static void
fake_volume_class_init (FakeVolumeClass *klass)
{
  G_OBJECT_CLASS (klass)->finalize = fake_volume_finalize;
}

static void
fake_volume_init (FakeVolume *volume G_GNUC_UNUSED)
{
}

static GVolume *
volume_new (const char *class)
{
  FakeVolume *volume = g_object_new (FAKE_TYPE_VOLUME, NULL);

  volume->class = g_strdup (class);
  return G_VOLUME (volume);
}

/* A mount: its root, its drive and volume, what can be done to it, and what was. */

#define FAKE_TYPE_MOUNT (fake_mount_get_type ())
G_DECLARE_FINAL_TYPE (FakeMount, fake_mount, FAKE, MOUNT, GObject)

struct _FakeMount
{
  GObject   parent;
  GFile    *root;
  GDrive   *drive;
  GVolume  *volume;
  gboolean  can_eject;
  gboolean  can_unmount;
  /* What an eject or an unmount fails with, if anything. */
  GError   *failure;
  /* "eject" or "unmount", once one is asked for. */
  const char *removed_by;
};

static GFile *
fake_mount_get_root (GMount *mount)
{
  return g_object_ref (FAKE_MOUNT (mount)->root);
}

static char *
fake_mount_get_name (GMount *mount)
{
  return g_file_get_basename (FAKE_MOUNT (mount)->root);
}

static GDrive *
fake_mount_get_drive (GMount *mount)
{
  return FAKE_MOUNT (mount)->drive != NULL ? g_object_ref (FAKE_MOUNT (mount)->drive) : NULL;
}

static GVolume *
fake_mount_get_volume (GMount *mount)
{
  return FAKE_MOUNT (mount)->volume != NULL ? g_object_ref (FAKE_MOUNT (mount)->volume) : NULL;
}

static gboolean
fake_mount_can_eject (GMount *mount)
{
  return FAKE_MOUNT (mount)->can_eject;
}

static gboolean
fake_mount_can_unmount (GMount *mount)
{
  return FAKE_MOUNT (mount)->can_unmount;
}

static void
fake_mount_remove (GMount *mount, const char *how, GCancellable *cancellable,
                   GAsyncReadyCallback callback, gpointer data)
{
  FakeMount *fake = FAKE_MOUNT (mount);
  g_autoptr (GTask) task = g_task_new (mount, cancellable, callback, data);

  fake->removed_by = how;
  if (fake->failure != NULL)
    g_task_return_error (task, g_error_copy (fake->failure));
  else
    g_task_return_boolean (task, TRUE);
}

static void
fake_mount_eject (GMount *mount, GMountUnmountFlags flags G_GNUC_UNUSED,
                  GMountOperation *operation G_GNUC_UNUSED, GCancellable *cancellable,
                  GAsyncReadyCallback callback, gpointer data)
{
  fake_mount_remove (mount, "eject", cancellable, callback, data);
}

static void
fake_mount_unmount (GMount *mount, GMountUnmountFlags flags G_GNUC_UNUSED,
                    GMountOperation *operation G_GNUC_UNUSED, GCancellable *cancellable,
                    GAsyncReadyCallback callback, gpointer data)
{
  fake_mount_remove (mount, "unmount", cancellable, callback, data);
}

static gboolean
fake_mount_remove_finish (GMount *mount G_GNUC_UNUSED, GAsyncResult *result, GError **error)
{
  return g_task_propagate_boolean (G_TASK (result), error);
}

static void
fake_mount_iface_init (GMountIface *iface)
{
  iface->get_root = fake_mount_get_root;
  iface->get_name = fake_mount_get_name;
  iface->get_drive = fake_mount_get_drive;
  iface->get_volume = fake_mount_get_volume;
  iface->can_eject = fake_mount_can_eject;
  iface->can_unmount = fake_mount_can_unmount;
  iface->eject_with_operation = fake_mount_eject;
  iface->eject_with_operation_finish = fake_mount_remove_finish;
  iface->unmount_with_operation = fake_mount_unmount;
  iface->unmount_with_operation_finish = fake_mount_remove_finish;
}

G_DEFINE_TYPE_WITH_CODE (FakeMount, fake_mount, G_TYPE_OBJECT,
                         G_IMPLEMENT_INTERFACE (G_TYPE_MOUNT, fake_mount_iface_init))

static void
fake_mount_finalize (GObject *object)
{
  FakeMount *mount = FAKE_MOUNT (object);

  g_object_unref (mount->root);
  g_clear_object (&mount->drive);
  g_clear_object (&mount->volume);
  g_clear_error (&mount->failure);
  G_OBJECT_CLASS (fake_mount_parent_class)->finalize (object);
}

static void
fake_mount_class_init (FakeMountClass *klass)
{
  G_OBJECT_CLASS (klass)->finalize = fake_mount_finalize;
}

static void
fake_mount_init (FakeMount *mount G_GNUC_UNUSED)
{
}

/* The volume monitor, whose mounts the test sets, and tells as GIO's does. */

#define FAKE_TYPE_MONITOR (fake_monitor_get_type ())
G_DECLARE_FINAL_TYPE (FakeMonitor, fake_monitor, FAKE, MONITOR, GVolumeMonitor)

struct _FakeMonitor
{
  GVolumeMonitor  parent;
  GList          *mounts;
};

G_DEFINE_TYPE (FakeMonitor, fake_monitor, G_TYPE_VOLUME_MONITOR)

static gpointer
ref_copy (gconstpointer object, gpointer data G_GNUC_UNUSED)
{
  return g_object_ref ((gpointer) object);
}

static GList *
fake_monitor_get_mounts (GVolumeMonitor *monitor)
{
  return g_list_copy_deep (FAKE_MONITOR (monitor)->mounts, ref_copy, NULL);
}

static void
fake_monitor_finalize (GObject *object)
{
  g_list_free_full (FAKE_MONITOR (object)->mounts, g_object_unref);
  G_OBJECT_CLASS (fake_monitor_parent_class)->finalize (object);
}

static void
fake_monitor_class_init (FakeMonitorClass *klass)
{
  G_OBJECT_CLASS (klass)->finalize = fake_monitor_finalize;
  G_VOLUME_MONITOR_CLASS (klass)->get_mounts = fake_monitor_get_mounts;
}

static void
fake_monitor_init (FakeMonitor *monitor G_GNUC_UNUSED)
{
}

/* A disk is mounted: the monitor lists it, then tells. */
static void
monitor_mount (FakeMonitor *monitor, FakeMount *mount)
{
  monitor->mounts = g_list_append (monitor->mounts, g_object_ref (mount));
  g_signal_emit_by_name (monitor, "mount-added", mount);
}

/* It goes: unmounted or ejected. */
static void
monitor_unmount (FakeMonitor *monitor, FakeMount *mount)
{
  GList *link = g_list_find (monitor->mounts, mount);

  monitor->mounts = g_list_delete_link (monitor->mounts, link);
  g_signal_emit_by_name (monitor, "mount-removed", mount);
  g_object_unref (mount);
}

/* The mount at uri, which takes drive and volume. */
static FakeMount *
mount_new (const char *uri, GDrive *drive, GVolume *volume, gboolean can_eject,
           gboolean can_unmount)
{
  FakeMount *mount = g_object_new (FAKE_TYPE_MOUNT, NULL);

  mount->root = g_file_new_for_uri (uri);
  mount->drive = drive;
  mount->volume = volume;
  mount->can_eject = can_eject;
  mount->can_unmount = can_unmount;
  return mount;
}

static FakeMount *
usb_disk (const char *uri)
{
  return mount_new (uri, drive_new (TRUE, TRUE, TRUE), volume_new ("device"), TRUE, TRUE);
}

static FakeMount *
internal_disk (const char *uri)
{
  return mount_new (uri, drive_new (FALSE, FALSE, FALSE), volume_new ("device"), FALSE, TRUE);
}

static gboolean
is_removable (FakeMount *mount)
{
  g_autoptr (FakeMount) owned = mount;

  return dk_drive_is_removable (G_MOUNT (owned));
}

static void
test_a_usb_disk_an_sd_card_a_disc_and_a_phone_have_an_item (void)
{
  g_assert_true (is_removable (usb_disk ("file:///media/u/USB")));
  /* An SD card's reader stays, its card comes out. */
  g_assert_true (is_removable (mount_new ("file:///media/u/SD", drive_new (FALSE, TRUE, FALSE),
                                          volume_new ("device"), FALSE, TRUE)));
  g_assert_true (is_removable (mount_new ("file:///media/u/DISC", drive_new (FALSE, TRUE, TRUE),
                                          volume_new ("device"), TRUE, TRUE)));
  /* gvfs's phones and cameras have no drive. */
  g_assert_true (is_removable (mount_new ("mtp://Phone/", NULL, NULL, FALSE, TRUE)));
  g_assert_true (is_removable (mount_new ("gphoto2://Camera/", NULL, NULL, FALSE, TRUE)));
}

static void
test_an_internal_disk_and_a_network_share_have_none (void)
{
  g_assert_false (is_removable (internal_disk ("file:///mnt/data")));
  g_assert_false (is_removable (mount_new ("smb://server/share/", NULL, NULL, FALSE, TRUE)));
  g_assert_false (is_removable (mount_new ("sftp://host/", NULL, volume_new ("network"), FALSE,
                                           TRUE)));
  /* A share whose volume is a network's, whatever its scheme. */
  g_assert_false (is_removable (mount_new ("file:///net/nfs", NULL, volume_new ("network"),
                                           FALSE, TRUE)));
}

static void
test_a_mount_that_cant_be_ejected_or_unmounted_or_is_shadowed_has_none (void)
{
  g_autoptr (FakeMount) shadowed = usb_disk ("file:///media/u/USB");

  g_assert_false (is_removable (mount_new ("file:///media/u/USB", drive_new (TRUE, TRUE, FALSE),
                                           volume_new ("device"), FALSE, FALSE)));
  /* A gvfs mount that stands for another, which has the item. */
  g_mount_shadow (G_MOUNT (shadowed));
  g_assert_false (dk_drive_is_removable (G_MOUNT (shadowed)));
  g_mount_unshadow (G_MOUNT (shadowed));
}

static void
on_changed (gpointer data)
{
  (*(guint *) data)++;
}

static void
assert_keys (DkDrives *drives, const char *const *expected)
{
  const char *const *keys = dk_drives_get_keys (drives);

  g_assert_nonnull (keys);
  g_assert_cmpstrv (keys, expected);
}

static void
test_the_removable_mounts_at_the_start_have_items_in_the_monitors_order (void)
{
  g_autoptr (FakeMonitor) monitor = g_object_new (FAKE_TYPE_MONITOR, NULL);
  const char *const expected[] = { "file:///media/u/USB", "mtp://Phone/", NULL };
  DkDrives *drives;

  monitor->mounts = g_list_append (monitor->mounts, usb_disk ("file:///media/u/USB"));
  monitor->mounts = g_list_append (monitor->mounts, internal_disk ("file:///mnt/data"));
  monitor->mounts = g_list_append (monitor->mounts,
                                   mount_new ("mtp://Phone/", NULL, NULL, FALSE, TRUE));
  drives = dk_drives_new (G_VOLUME_MONITOR (monitor));
  assert_keys (drives, expected);
  g_assert_true (G_IS_MOUNT (dk_drives_lookup (drives, "mtp://Phone/")));
  g_assert_null (dk_drives_lookup (drives, "file:///mnt/data"));
  dk_drives_free (drives);
}

static void
test_an_item_comes_when_a_disk_is_mounted_and_goes_when_it_is_unmounted (void)
{
  g_autoptr (FakeMonitor) monitor = g_object_new (FAKE_TYPE_MONITOR, NULL);
  g_autoptr (FakeMount) usb = usb_disk ("file:///media/u/USB");
  g_autoptr (FakeMount) sd = mount_new ("file:///media/u/SD", drive_new (FALSE, TRUE, FALSE),
                                        volume_new ("device"), FALSE, TRUE);
  g_autoptr (FakeMount) internal = internal_disk ("file:///mnt/data");
  const char *const none[] = { NULL };
  const char *const usb_only[] = { "file:///media/u/USB", NULL };
  const char *const usb_then_sd[] = { "file:///media/u/USB", "file:///media/u/SD", NULL };
  const char *const sd_only[] = { "file:///media/u/SD", NULL };
  DkDrives *drives = dk_drives_new (G_VOLUME_MONITOR (monitor));
  guint changes = 0;

  dk_drives_set_changed (drives, on_changed, &changes);
  assert_keys (drives, none);
  monitor_mount (monitor, usb);
  assert_keys (drives, usb_only);
  g_assert_cmpuint (changes, ==, 1);
  monitor_mount (monitor, internal);
  assert_keys (drives, usb_only);
  /* In the order they were mounted. */
  monitor_mount (monitor, sd);
  assert_keys (drives, usb_then_sd);
  g_assert_cmpuint (changes, >=, 2);
  monitor_unmount (monitor, usb);
  assert_keys (drives, sd_only);
  g_assert_null (dk_drives_lookup (drives, "file:///media/u/USB"));
  monitor_unmount (monitor, sd);
  assert_keys (drives, none);
  dk_drives_free (drives);
}

static void
test_a_mount_that_turns_shadowed_loses_its_item (void)
{
  g_autoptr (FakeMonitor) monitor = g_object_new (FAKE_TYPE_MONITOR, NULL);
  g_autoptr (FakeMount) usb = usb_disk ("file:///media/u/USB");
  const char *const none[] = { NULL };
  const char *const usb_only[] = { "file:///media/u/USB", NULL };
  DkDrives *drives = dk_drives_new (G_VOLUME_MONITOR (monitor));

  monitor_mount (monitor, usb);
  assert_keys (drives, usb_only);
  g_mount_shadow (G_MOUNT (usb));
  g_signal_emit_by_name (monitor, "mount-changed", usb);
  assert_keys (drives, none);
  g_mount_unshadow (G_MOUNT (usb));
  g_signal_emit_by_name (monitor, "mount-changed", usb);
  assert_keys (drives, usb_only);
  dk_drives_free (drives);
}

typedef struct
{
  gboolean  done;
  GError   *error;
} Removed;

static void
on_removed (GError *error, gpointer data)
{
  Removed *removed = data;

  removed->done = TRUE;
  removed->error = error != NULL ? g_error_copy (error) : NULL;
}

static gboolean
on_timeout (gpointer data)
{
  *(gboolean *) data = TRUE;
  return G_SOURCE_REMOVE;
}

static void
run_until (const gboolean *done)
{
  gboolean timed_out = FALSE;
  guint id = g_timeout_add_seconds (5, on_timeout, &timed_out);

  while (!*done && !timed_out)
    g_main_context_iteration (NULL, TRUE);
  g_assert_false (timed_out);
  g_source_remove (id);
}

static void
test_a_disk_that_can_be_ejected_is_ejected_and_another_unmounted (void)
{
  g_autoptr (FakeMount) usb = usb_disk ("file:///media/u/USB");
  g_autoptr (FakeMount) sd = mount_new ("file:///media/u/SD", drive_new (FALSE, TRUE, FALSE),
                                        volume_new ("device"), FALSE, TRUE);
  Removed ejected = { 0 }, unmounted = { 0 };

  dk_drive_remove (G_MOUNT (usb), NULL, on_removed, &ejected, NULL);
  run_until (&ejected.done);
  g_assert_no_error (ejected.error);
  g_assert_cmpstr (usb->removed_by, ==, "eject");
  dk_drive_remove (G_MOUNT (sd), NULL, on_removed, &unmounted, NULL);
  run_until (&unmounted.done);
  g_assert_no_error (unmounted.error);
  g_assert_cmpstr (sd->removed_by, ==, "unmount");
}

static void
test_a_disk_that_is_busy_tells_why (void)
{
  g_autoptr (FakeMount) usb = usb_disk ("file:///media/u/USB");
  Removed removed = { 0 };

  usb->failure = g_error_new_literal (G_IO_ERROR, G_IO_ERROR_BUSY, "Target is busy");
  dk_drive_remove (G_MOUNT (usb), NULL, on_removed, &removed, NULL);
  run_until (&removed.done);
  g_assert_error (removed.error, G_IO_ERROR, G_IO_ERROR_BUSY);
  g_clear_error (&removed.error);
}

static void
test_an_error_the_user_was_already_shown_is_not_told_again (void)
{
  g_autoptr (FakeMount) usb = usb_disk ("file:///media/u/USB");
  Removed removed = { 0 };

  usb->failure = g_error_new_literal (G_IO_ERROR, G_IO_ERROR_FAILED_HANDLED, "handled");
  dk_drive_remove (G_MOUNT (usb), NULL, on_removed, &removed, NULL);
  run_until (&removed.done);
  g_assert_no_error (removed.error);
}

typedef struct
{
  gboolean   done;
  GPtrArray *failed;
} Copying;

static void
on_copy_failed (const char *uri, GError *error G_GNUC_UNUSED, gpointer data)
{
  g_ptr_array_add (((Copying *) data)->failed, g_strdup (uri));
}

static void
on_copied (gpointer data)
{
  ((Copying *) data)->done = TRUE;
}

static char *
write_file (const char *dir, const char *name)
{
  char *path = g_build_filename (dir, name, NULL);
  g_autofree char *parent = g_path_get_dirname (path);

  g_assert_cmpint (g_mkdir_with_parents (parent, 0700), ==, 0);
  g_assert_true (g_file_set_contents (path, "content", -1, NULL));
  return path;
}

static gboolean
exists_in (const char *dir, const char *name)
{
  g_autofree char *path = g_build_filename (dir, name, NULL);

  return g_file_test (path, G_FILE_TEST_EXISTS);
}

static void
test_files_dropped_on_a_disk_are_copied_to_it (void)
{
  g_autofree char *home = g_dir_make_tmp ("caminito-drive-home-XXXXXX", NULL);
  g_autofree char *disk = g_dir_make_tmp ("caminito-drive-disk-XXXXXX", NULL);
  g_autofree char *disk_uri = g_filename_to_uri (disk, NULL, NULL);
  g_autofree char *file = write_file (home, "photo.jpg");
  g_autofree char *inner = write_file (home, "album/one.jpg");
  g_autofree char *file_uri = g_filename_to_uri (file, NULL, NULL);
  g_autofree char *album = g_build_filename (home, "album", NULL);
  g_autofree char *album_uri = g_filename_to_uri (album, NULL, NULL);
  g_autoptr (FakeMount) mount = mount_new (disk_uri, drive_new (TRUE, TRUE, TRUE),
                                           volume_new ("device"), TRUE, TRUE);
  const char *const uris[] = { file_uri, album_uri, NULL };
  Copying copying = { FALSE, g_ptr_array_new_with_free_func (g_free) };

  dk_drive_put (G_MOUNT (mount), uris, NULL, on_copy_failed, on_copied, &copying, NULL);
  run_until (&copying.done);
  g_assert_cmpuint (copying.failed->len, ==, 0);
  g_assert_true (exists_in (disk, "photo.jpg"));
  g_assert_true (exists_in (disk, "album/one.jpg"));
  /* A copy, though the disk here is on the same file system as the files: they stay. */
  g_assert_true (g_file_test (file, G_FILE_TEST_EXISTS));
  g_assert_true (g_file_test (inner, G_FILE_TEST_EXISTS));
  g_ptr_array_unref (copying.failed);
}

static void
test_a_file_that_is_already_on_the_disk_fails_and_stays (void)
{
  g_autofree char *home = g_dir_make_tmp ("caminito-drive-home-XXXXXX", NULL);
  g_autofree char *disk = g_dir_make_tmp ("caminito-drive-disk-XXXXXX", NULL);
  g_autofree char *disk_uri = g_filename_to_uri (disk, NULL, NULL);
  g_autofree char *file = write_file (home, "notes.txt");
  g_autofree char *there = write_file (disk, "notes.txt");
  g_autofree char *file_uri = g_filename_to_uri (file, NULL, NULL);
  g_autoptr (FakeMount) mount = mount_new (disk_uri, drive_new (TRUE, TRUE, TRUE),
                                           volume_new ("device"), TRUE, TRUE);
  const char *const uris[] = { file_uri, NULL };
  Copying copying = { FALSE, g_ptr_array_new_with_free_func (g_free) };

  dk_drive_put (G_MOUNT (mount), uris, NULL, on_copy_failed, on_copied, &copying, NULL);
  run_until (&copying.done);
  g_assert_cmpuint (copying.failed->len, ==, 1);
  g_assert_cmpstr (copying.failed->pdata[0], ==, file_uri);
  g_assert_true (g_file_test (file, G_FILE_TEST_EXISTS));
  g_ptr_array_unref (copying.failed);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/drive/usb-sd-disc-and-phone-have-an-item",
                   test_a_usb_disk_an_sd_card_a_disc_and_a_phone_have_an_item);
  g_test_add_func ("/drive/internal-disk-and-network-share-have-none",
                   test_an_internal_disk_and_a_network_share_have_none);
  g_test_add_func ("/drive/cant-be-ejected-or-unmounted-or-shadowed-has-none",
                   test_a_mount_that_cant_be_ejected_or_unmounted_or_is_shadowed_has_none);
  g_test_add_func ("/drives/the-removable-mounts-at-the-start-in-the-monitors-order",
                   test_the_removable_mounts_at_the_start_have_items_in_the_monitors_order);
  g_test_add_func ("/drives/an-item-comes-with-a-mount-and-goes-with-an-unmount",
                   test_an_item_comes_when_a_disk_is_mounted_and_goes_when_it_is_unmounted);
  g_test_add_func ("/drives/a-mount-that-turns-shadowed-loses-its-item",
                   test_a_mount_that_turns_shadowed_loses_its_item);
  g_test_add_func ("/drive/remove/ejects-what-can-be-ejected-and-unmounts-the-rest",
                   test_a_disk_that_can_be_ejected_is_ejected_and_another_unmounted);
  g_test_add_func ("/drive/remove/a-busy-disk-tells-why", test_a_disk_that_is_busy_tells_why);
  g_test_add_func ("/drive/remove/an-error-already-shown-is-not-told-again",
                   test_an_error_the_user_was_already_shown_is_not_told_again);
  g_test_add_func ("/drive/put/files-dropped-on-a-disk-are-copied-to-it",
                   test_files_dropped_on_a_disk_are_copied_to_it);
  g_test_add_func ("/drive/put/a-file-already-on-the-disk-fails-and-stays",
                   test_a_file_that_is_already_on_the_disk_fails_and_stays);
  return g_test_run ();
}
