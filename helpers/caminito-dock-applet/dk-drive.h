/*
 * The Dock's Removable drive items (CMN-240), through GIO's GVolumeMonitor: which mounts have
 * one, the list of them as disks are mounted and go, ejecting or unmounting one, and copying
 * files to one. It names no toolkit.
 */

#pragma once

#include <gio/gio.h>

#include "dk-folder.h"

/* Whether the mount is of a disk the user can eject or unmount, such as a USB disk, an SD
   card, a disc or a phone: not an internal disk, a network share, or a mount another stands
   for. */
gboolean dk_drive_is_removable (GMount *mount);

/* The removable drives' mounts, which follow the monitor's as disks are mounted and go. */
typedef struct _DkDrives DkDrives;

typedef void (*DkDrivesChanged) (gpointer data);

/* The drives of monitor, which it holds a reference to. */
DkDrives           *dk_drives_new         (GVolumeMonitor *monitor);
void                dk_drives_free        (DkDrives *drives);
/* Tells changed when a drive comes or goes, or a mount of one changes (its name, its icon). */
void                dk_drives_set_changed (DkDrives *drives, DkDrivesChanged changed,
                                           gpointer data);
/* Their keys, each the root's URI, in the order they were mounted: NULL-terminated, and the
   drives' until the next change. */
const char *const  *dk_drives_get_keys    (DkDrives *drives);
/* The mount of a key, which the drives own, or NULL. */
GMount             *dk_drives_lookup      (DkDrives *drives, const char *key);

/* error is NULL when it worked, and is the callee's to copy. */
typedef void (*DkDriveRemoved) (GError *error, gpointer data);

/* Ejects the disk if it can be ejected, and unmounts it otherwise, as its menu's Eject or
   Unmount says. Tells done on the main thread, unless cancellable has been cancelled: that
   only stops the telling, the eject goes on. An error the user was already shown, such as
   the busy disk's question gvfs asks, is not told again. destroy, if given, frees data at
   the end either way. */
void                dk_drive_remove       (GMount *mount, GCancellable *cancellable,
                                           DkDriveRemoved done, gpointer data,
                                           GDestroyNotify destroy);

/* Copies the files the URIs name (NULL-terminated) to the disk's root, a folder with
   everything in it, wherever they are: a drop on a disk leaves the files where they were.
   It is dk_folder_copy's, to the root. */
void                dk_drive_put          (GMount *mount, const char *const *uris,
                                           GCancellable *cancellable,
                                           DkFolderPutFailed failed, DkFolderPutDone done,
                                           gpointer data, GDestroyNotify destroy);
