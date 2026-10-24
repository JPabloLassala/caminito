/*
 * One Dock item's widget (CMN-219): the program's icon at the panel's thickness, Plasma's
 * indicator line on the panel's edge, the tooltip, the minimise target of each of the item's
 * windows, and the item as a drag source. What a click does is the dock's.
 */

#pragma once

#include <gtk/gtk.h>

typedef struct _DkItemView DkItemView;

/* The drag target the items are dragged by inside the dock. */
#define DK_DRAG_TARGET "application/x-caminito-dock-item"

/* A view and its widget, which the widget owns: the view lives as long as it. */
DkItemView *dk_item_view_new        (void);
GtkWidget  *dk_item_view_get_widget (DkItemView *view);
/* The view of an item's widget. */
DkItemView *dk_item_view_from_widget (GtkWidget *widget);

/* The item is thickness pixels square, on a panel at edge (GTK_POS_BOTTOM for a panel along
   the bottom of the screen). The indicator is on that edge. */
void        dk_item_view_set_geometry (DkItemView *view, guint thickness, GtkPositionType edge);
/* What the item shows. windows is the item's XfwWindows, most recent first, which the view
   takes references to; icon is the program's, or NULL for a generic one. active is whether
   the item has the focus. A key that isn't an entry's can't be dragged to be kept. */
void        dk_item_view_set          (DkItemView *view,
                                       const char *key,
                                       GIcon      *icon,
                                       const char *name,
                                       GPtrArray  *windows,
                                       gboolean    active,
                                       gboolean    draggable);
const char *dk_item_view_get_key      (DkItemView *view);
GPtrArray  *dk_item_view_get_windows  (DkItemView *view);
gboolean    dk_item_view_is_active    (DkItemView *view);
/* Tells the compositor where each window's item is, for the minimise animation. */
void        dk_item_view_update_rectangles (DkItemView *view);
