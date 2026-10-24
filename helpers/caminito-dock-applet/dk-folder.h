/*
 * A Kept folder's folder (CMN-239), through GIO: its icon and name, its contents for the popup,
 * and putting files in it. It names no toolkit.
 */

#pragma once

#include <gio/gio.h>

typedef struct
{
  char     *uri;
  /* The name Caja shows. */
  char     *name;
  GIcon    *icon;
  gboolean  is_folder;
} DkFolderEntry;

/* Whether the folder is there to be read: it exists and is a folder. A folder on a drive
   that is not mounted is not. */
gboolean   dk_folder_exists       (GFile *folder);
/* The name Caja shows for the folder, or NULL where it can't be read. */
char      *dk_folder_display_name (GFile *folder);
/* The folder's icon: its custom icon in Caja if it has one, else its own. NULL where it
   can't be read. */
GIcon     *dk_folder_icon         (GFile *folder);
/* What the folder holds, as DkFolderEntry (which the array owns), folders first and then by
   name, without the hidden files. NULL, with the error, where it can't be read. */
GPtrArray *dk_folder_list         (GFile *folder, GError **error);
void       dk_folder_entry_free   (DkFolderEntry *entry);
/* Whether the files the URI names could be told apart: a folder (so it can be kept). */
gboolean   dk_folder_uri_is_folder (const char *uri);

/* A file that couldn't be put in the folder, once for each. */
typedef void (*DkFolderPutFailed) (const char *uri, GError *error, gpointer data);
/* After every file has been put or has failed. */
typedef void (*DkFolderPutDone) (gpointer data);

/* Puts the files the URIs name (NULL-terminated) in the folder, as Caja does: a move on the
   file system the folder is on, a copy (of a folder with everything in it) from another.
   It works off the main thread, and tells failed and done on it, unless cancellable has been
   cancelled. destroy, if given, frees data at the end either way. A file that is already in
   the folder is left where it is. */
void       dk_folder_put          (GFile *folder, const char *const *uris,
                                   GCancellable *cancellable,
                                   DkFolderPutFailed failed, DkFolderPutDone done,
                                   gpointer data, GDestroyNotify destroy);

G_DEFINE_AUTOPTR_CLEANUP_FUNC (DkFolderEntry, dk_folder_entry_free)
