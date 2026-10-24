/*
 * The Dock (CMN-219): a row, or a column, of Dock items for the windows libxfce4windowing
 * lists and the kept items in the store. It makes the items (dk-model.h), draws them
 * (dk-item.h), and does what a click, the context menu and a drag do. The applet puts it in
 * the panel; the probe puts it in a window of its own.
 */

#pragma once

#include <gtk/gtk.h>

#include "dk-store.h"

typedef struct _DkDock DkDock;

/* The dock for store, which it doesn't own. */
DkDock    *dk_dock_new         (DkStore *store);
void       dk_dock_free        (DkDock *dock);
GtkWidget *dk_dock_get_widget  (DkDock *dock);
/* The items are thickness pixels square, in a row for a panel along the top or the bottom
   of the screen (edge says which) and in a column otherwise. */
void       dk_dock_set_layout  (DkDock *dock, guint thickness, GtkPositionType edge);

/* Leaves out the windows of an app id: the probe's own window, which no panel's applet has. */
void       dk_dock_ignore_app_id (DkDock *dock, const char *app_id);

/* How long an item pulses for a program that maps no window, in microseconds: the dock's ten
   seconds, shortened for a test that waits for the end. */
void       dk_dock_set_launch_timeout (DkDock *dock, gint64 timeout);

/* The item widgets' keys, in order, and the first one for key. */
GPtrArray *dk_dock_get_keys    (DkDock *dock);
GtkWidget *dk_dock_get_item    (DkDock *dock, const char *key);
/* What a drop of key at a point in the dock's widget does: it keeps the program at the
   position there, or moves the kept item to it. The probe's way to a drag, and the drag's
   own. */
void       dk_dock_drop_key    (DkDock *dock, const char *key, int x, int y);
/* The context menu of the item, popped up as a right-click on it does. */
void       dk_dock_popup_menu  (DkDock *dock, GtkWidget *item, const GdkEvent *event);
/* The menu being shown, or NULL. */
GtkWidget *dk_dock_get_menu    (DkDock *dock);

/* The Trash (CMN-220), after a separator at the end of the row. Its icon is full or empty,
   a click opens it in Caja, and its menu is Open and Empty Trash, which asks first. */
GtkWidget *dk_dock_get_trash    (DkDock *dock);
gboolean   dk_dock_trash_is_full (DkDock *dock);
/* What a drop of files on the Trash does: moves the URIs' files to it. The probe's way to
   a drag, and the drag's own. */
void       dk_dock_drop_uris   (DkDock *dock, const char *const *uris);
/* The confirmation of emptying being shown, or NULL. Answer it with gtk_dialog_response
   (GTK_RESPONSE_ACCEPT to empty). */
GtkWidget *dk_dock_get_dialog  (DkDock *dock);

/* The Kept folders (CMN-239), after the separator and before the Trash. Their widgets are
   named by the folder's URI. */
GPtrArray *dk_dock_get_folders      (DkDock *dock);
GtkWidget *dk_dock_get_folder       (DkDock *dock, const char *uri);
/* The folder is gone (deleted, or on a drive that isn't there), and its item is dimmed. */
gboolean   dk_dock_folder_is_gone   (DkDock *dock, const char *uri);
/* What a click on a Kept folder does: shows the grid of its contents, or says that the folder
   isn't available. The probe's way to a click. */
void       dk_dock_open_folder      (DkDock *dock, const char *uri);
/* The grid being shown, or NULL; its cells' names, a folder's with a slash after; and the
   error being shown, or NULL. */
GtkWidget *dk_dock_get_popup        (DkDock *dock);
GPtrArray *dk_dock_get_popup_entries (DkDock *dock);
GtkWidget *dk_dock_get_error_dialog (DkDock *dock);
/* What a drop of folders (those of the URIs that are) between the Kept folders does: keeps
   them at the position, counted in Kept folders; and what a drop of a Kept folder does: moves
   it there. The probe's way to a drag, and the drag's own. */
void       dk_dock_keep_folders     (DkDock *dock, const char *const *uris, guint position);
void       dk_dock_move_folder      (DkDock *dock, const char *uri, guint position);
/* What a drop of files on a Kept folder does: moves them in on its file system, copies them
   in from another. */
void       dk_dock_drop_on_folder   (DkDock *dock, const char *folder_uri,
                                     const char *const *uris);
