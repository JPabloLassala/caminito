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
