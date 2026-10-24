/*
 * The Dock's Trash (CMN-220), through GIO: whether it holds anything, as it changes, putting
 * files in it and emptying it. It names no toolkit. The Dock gives it `trash:///`, which
 * needs gvfs, as Caja does; a test gives it a directory.
 */

#pragma once

#include <gio/gio.h>

typedef struct _DkTrash DkTrash;

typedef void (*DkTrashChanged) (gpointer data);
/* error is NULL when it worked, and is the callee's to copy. */
typedef void (*DkTrashEmptied) (GError *error, gpointer data);
/* A file that couldn't be put in the Trash, once for each. */
typedef void (*DkTrashPutFailed) (const char *uri, GError *error, gpointer data);
/* After every file of a put has been put or has failed. */
typedef void (*DkTrashPutDone) (gpointer data);

/* The Trash that location is, which it watches. It is empty until the first look has
   answered, and stays so where location can't be read. */
DkTrash  *dk_trash_new         (GFile *location);
void      dk_trash_free        (DkTrash *trash);
/* Tells changed when the Trash turns full or empty. */
void      dk_trash_set_changed (DkTrash *trash, DkTrashChanged changed, gpointer data);
gboolean  dk_trash_is_full     (DkTrash *trash);
/* Deletes everything in it, folders with their contents, off the main thread. It goes on
   past a file it can't delete, and reports the first error. */
void      dk_trash_empty       (DkTrash *trash, DkTrashEmptied done, gpointer data);
/* Moves the files the URIs name (NULL-terminated) to the Trash. */
void      dk_trash_put         (DkTrash *trash, const char *const *uris,
                                DkTrashPutFailed failed, DkTrashPutDone done, gpointer data);
