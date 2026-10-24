/*
 * The Dock (CMN-219), and the Trash that ends its row (CMN-220).
 */

#include <gio/gdesktopappinfo.h>
#include <libxfce4windowing/libxfce4windowing.h>

#include "dk-dock.h"
#include "dk-item.h"
#include "dk-model.h"
#include "dk-resolve.h"
#include "dk-trash.h"

typedef struct
{
  XfwWindow *window;
  guint64    opened;
  guint64    activated;
} WindowInfo;

struct _DkDock
{
  /* The dock: the items' box, a separator and the Trash. */
  GtkWidget       *widget;
  /* The Dock items. */
  GtkWidget       *box;
  GtkWidget       *separator;
  DkTrash         *trash;
  DkItemView      *trash_view;
  /* The question or the error being shown. */
  GtkWidget       *dialog;
  DkStore         *store;
  DkCatalog       *catalog;
  XfwScreen       *screen;
  GAppInfoMonitor *monitor;
  char            *ignored;
  /* XfwWindow to WindowInfo. */
  GHashTable      *windows;
  /* An item's key to its DkItemView, whose widget the box holds. */
  GHashTable      *views;
  /* An entry's id to its GDesktopAppInfo, NULL for none. */
  GHashTable      *infos;
  guint64          clock;
  guint            refresh_id;
  guint            thickness;
  GtkPositionType  edge;
  /* The item's windows as they were when the button went down, for the click that follows:
     the user clicks on what they saw, whatever the press itself does to the focus. */
  GArray          *pressed;
  DkItemView      *pressed_view;
  /* The menu being shown. */
  GtkWidget       *menu;
  /* Where a drag would drop, as the number of items before it; -1 when none is over. */
  int              drop_slot;
};

#define MENU_TITLE_CHARS 36
#define TRASH_URI "trash:///"

static void schedule_refresh (DkDock *dock);
static gboolean on_item_button_press (GtkWidget *widget, GdkEventButton *event, gpointer data);
static gboolean on_item_button_release (GtkWidget *widget, GdkEventButton *event, gpointer data);

static gboolean
is_horizontal (const DkDock *dock)
{
  return dock->edge == GTK_POS_TOP || dock->edge == GTK_POS_BOTTOM;
}

/* The windows a Dock shows: those a window list would. Caminito already leaves out the ones
   that skip the taskbar. */
static gboolean
is_listed (XfwWindow *window)
{
  /* Wayland has no window types, and says so each time it's asked. */
  if (xfw_windowing_get () != XFW_WINDOWING_X11)
    return !xfw_window_is_skip_tasklist (window);
  switch (xfw_window_get_window_type (window))
    {
    case XFW_WINDOW_TYPE_NORMAL:
    case XFW_WINDOW_TYPE_DIALOG:
    case XFW_WINDOW_TYPE_UTILITY:
      return !xfw_window_is_skip_tasklist (window);
    default:
      return FALSE;
    }
}

/* An X11 window has two class names, a Wayland one its app id: the first that names an entry,
   or else the first. */
static const char *
window_app_id (DkDock *dock, XfwWindow *window)
{
  const char *const *ids = xfw_window_get_class_ids (window);

  if (ids == NULL || ids[0] == NULL)
    return "";
  for (guint i = 0; ids[i] != NULL; i++)
    if (dk_catalog_resolve (dock->catalog, ids[i]) != NULL)
      return ids[i];
  return ids[0];
}

static GDesktopAppInfo *
entry_info (DkDock *dock, const char *key)
{
  if (!g_hash_table_contains (dock->infos, key))
    {
      GDesktopAppInfo *info = g_path_is_absolute (key)
                                ? g_desktop_app_info_new_from_filename (key)
                                : g_desktop_app_info_new (key);

      g_hash_table_insert (dock->infos, g_strdup (key), info);
    }
  return g_hash_table_lookup (dock->infos, key);
}

/* What the pointer's event is in the dock's own terms. */
static guint32
event_time (const GdkEvent *event)
{
  guint32 time = event != NULL ? gdk_event_get_time (event) : GDK_CURRENT_TIME;

  return time != GDK_CURRENT_TIME ? time : (guint32) (g_get_monotonic_time () / 1000);
}

static XfwSeat *
first_seat (DkDock *dock)
{
  GList *seats = xfw_screen_get_seats (dock->screen);

  return seats != NULL ? seats->data : NULL;
}

static void
window_activate (DkDock *dock, XfwWindow *window, const GdkEvent *event)
{
  g_autoptr (GError) error = NULL;

  if (!xfw_window_activate (window, first_seat (dock), event_time (event), &error))
    g_warning ("can't activate the window: %s", error->message);
}

static void
launch_entry (DkDock *dock, const char *key, const char *action)
{
  GDesktopAppInfo *info = entry_info (dock, key);
  GdkDisplay *display = gdk_display_get_default ();
  g_autoptr (GdkAppLaunchContext) context = gdk_display_get_app_launch_context (display);
  g_autoptr (GError) error = NULL;

  if (info == NULL)
    {
      g_warning ("can't launch %s: no such desktop entry", key);
      return;
    }
  if (action != NULL)
    g_desktop_app_info_launch_action (info, action, G_APP_LAUNCH_CONTEXT (context));
  else if (!g_app_info_launch (G_APP_INFO (info), NULL, G_APP_LAUNCH_CONTEXT (context), &error))
    g_warning ("can't launch %s: %s", key, error->message);
}

/* Rebuilds the items from the windows and the kept items, and makes the widgets match. */
static gboolean
refresh (gpointer data)
{
  DkDock *dock = data;
  g_autoptr (GArray) windows = g_array_new (FALSE, TRUE, sizeof (DkWindow));
  g_autoptr (GArray) items = NULL;
  g_autoptr (GHashTable) wanted = g_hash_table_new (g_str_hash, g_str_equal);
  GHashTableIter iter;
  gpointer value;

  dock->refresh_id = 0;
  g_hash_table_iter_init (&iter, dock->windows);
  while (g_hash_table_iter_next (&iter, NULL, &value))
    {
      WindowInfo *info = value;
      DkWindow window = {
        .handle = info->window,
        .app_id = window_app_id (dock, info->window),
        .title = xfw_window_get_name (info->window),
        .minimised = xfw_window_is_minimized (info->window),
        .active = xfw_window_is_active (info->window),
        .opened = info->opened,
        .activated = info->activated,
      };

      g_array_append_val (windows, window);
    }
  items = dk_items_build (dock->catalog, dk_store_get (dock->store),
                          (DkWindow *) windows->data, windows->len);
  for (guint i = 0; i < items->len; i++)
    {
      const DkItem *item = &g_array_index (items, DkItem, i);
      DkItemView *view = g_hash_table_lookup (dock->views, item->key);
      g_autoptr (GPtrArray) item_windows = g_ptr_array_new ();
      GDesktopAppInfo *info = item->entry_id != NULL ? entry_info (dock, item->entry_id) : NULL;
      GIcon *icon = info != NULL ? g_app_info_get_icon (G_APP_INFO (info)) : NULL;
      g_autofree char *name = NULL;
      const DkWindow *first = item->windows->len > 0
                                ? &g_array_index (item->windows, DkWindow, 0) : NULL;

      for (guint j = 0; j < item->windows->len; j++)
        g_ptr_array_add (item_windows, g_array_index (item->windows, DkWindow, j).handle);
      if (info != NULL)
        name = g_strdup (g_app_info_get_display_name (G_APP_INFO (info)));
      else if (first != NULL)
        {
          XfwApplication *application = xfw_window_get_application (first->handle);

          name = g_strdup (application != NULL && xfw_application_get_name (application) != NULL
                             ? xfw_application_get_name (application) : first->app_id);
          if (icon == NULL)
            icon = xfw_window_get_gicon (first->handle);
        }
      else
        name = g_strdup (item->key);
      if (view == NULL)
        {
          view = dk_item_view_new ();
          g_hash_table_insert (dock->views, g_strdup (item->key), view);
          gtk_box_pack_start (GTK_BOX (dock->box), dk_item_view_get_widget (view), FALSE, FALSE, 0);
          gtk_widget_show (dk_item_view_get_widget (view));
          g_signal_connect (dk_item_view_get_widget (view), "button-press-event",
                            G_CALLBACK (on_item_button_press), dock);
          g_signal_connect (dk_item_view_get_widget (view), "button-release-event",
                            G_CALLBACK (on_item_button_release), dock);
        }
      dk_item_view_set_geometry (view, dock->thickness, dock->edge);
      dk_item_view_set (view, item->key, icon, name, item_windows,
                        first != NULL && first->active,
                        item->entry_id != NULL);
      gtk_box_reorder_child (GTK_BOX (dock->box), dk_item_view_get_widget (view), i);
      g_hash_table_add (wanted, item->key);
    }
  g_hash_table_iter_init (&iter, dock->views);
  while (g_hash_table_iter_next (&iter, NULL, &value))
    {
      DkItemView *view = value;

      if (!g_hash_table_contains (wanted, dk_item_view_get_key (view)))
        {
          if (dock->pressed_view == view)
            {
              g_clear_pointer (&dock->pressed, g_array_unref);
              dock->pressed_view = NULL;
            }
          gtk_widget_destroy (dk_item_view_get_widget (view));
          g_hash_table_iter_remove (&iter);
        }
    }
  return G_SOURCE_REMOVE;
}

static void
schedule_refresh (DkDock *dock)
{
  if (dock->refresh_id == 0)
    dock->refresh_id = g_idle_add (refresh, dock);
}

static void
on_window_changed (XfwWindow *window G_GNUC_UNUSED, gpointer data)
{
  schedule_refresh (data);
}

static void
on_window_state_changed (XfwWindow *window, XfwWindowState changed_mask,
                         XfwWindowState new_state, gpointer data)
{
  DkDock *dock = data;
  WindowInfo *info = g_hash_table_lookup (dock->windows, window);

  if (info != NULL && (changed_mask & new_state & XFW_WINDOW_STATE_ACTIVE))
    info->activated = ++dock->clock;
  schedule_refresh (dock);
}

static void window_forget (DkDock *dock, XfwWindow *window);

static void
on_window_closed (XfwWindow *window, gpointer data)
{
  window_forget (data, window);
}

static void
window_info_free (gpointer data)
{
  WindowInfo *info = data;

  g_object_unref (info->window);
  g_free (info);
}

static void
window_remember (DkDock *dock, XfwWindow *window)
{
  WindowInfo *info;

  if (g_hash_table_contains (dock->windows, window) || !is_listed (window)
      || (dock->ignored != NULL && g_strcmp0 (window_app_id (dock, window), dock->ignored) == 0))
    return;
  info = g_new0 (WindowInfo, 1);
  info->window = g_object_ref (window);
  info->opened = ++dock->clock;
  info->activated = xfw_window_is_active (window) ? ++dock->clock : 0;
  g_hash_table_insert (dock->windows, window, info);
  g_signal_connect (window, "name-changed", G_CALLBACK (on_window_changed), dock);
  g_signal_connect (window, "class-changed", G_CALLBACK (on_window_changed), dock);
  g_signal_connect (window, "icon-changed", G_CALLBACK (on_window_changed), dock);
  g_signal_connect (window, "state-changed", G_CALLBACK (on_window_state_changed), dock);
  g_signal_connect (window, "closed", G_CALLBACK (on_window_closed), dock);
  schedule_refresh (dock);
}

static void
window_forget (DkDock *dock, XfwWindow *window)
{
  g_signal_handlers_disconnect_by_data (window, dock);
  g_hash_table_remove (dock->windows, window);
  schedule_refresh (dock);
}

static void
on_window_opened (XfwScreen *screen G_GNUC_UNUSED, XfwWindow *window, gpointer data)
{
  window_remember (data, window);
}

static void
on_window_closed_on_screen (XfwScreen *screen G_GNUC_UNUSED, XfwWindow *window, gpointer data)
{
  window_forget (data, window);
}

/* The desktop entries changed on disk: an install, a removal or an edit. */
static void
on_apps_changed (GAppInfoMonitor *monitor G_GNUC_UNUSED, gpointer data)
{
  DkDock *dock = data;

  dk_catalog_free (dock->catalog);
  dock->catalog = dk_catalog_new_installed ();
  g_hash_table_remove_all (dock->infos);
  schedule_refresh (dock);
}

static void
on_store_changed (gpointer data)
{
  schedule_refresh (data);
}

/* The item under a click, whose view the dock knows. */
static DkItemView *
view_of (GtkWidget *widget)
{
  return dk_item_view_from_widget (widget);
}

static void
pressed_window_clear (DkWindow *window)
{
  g_object_unref (window->handle);
}

/* The item as the model sees it, for a click: its windows, most recent first. It holds them,
   as one may close between the press and the release. */
static GArray *
item_for_click (DkDock *dock, DkItemView *view)
{
  g_autoptr (GArray) windows = g_array_new (FALSE, TRUE, sizeof (DkWindow));
  GPtrArray *views = dk_item_view_get_windows (view);

  g_array_set_clear_func (windows, (GDestroyNotify) pressed_window_clear);
  for (guint i = 0; i < views->len; i++)
    {
      WindowInfo *info = g_hash_table_lookup (dock->windows, views->pdata[i]);
      DkWindow window = {
        .handle = g_object_ref (views->pdata[i]),
        .minimised = xfw_window_is_minimized (views->pdata[i]),
        .active = xfw_window_is_active (views->pdata[i]),
        .opened = info != NULL ? info->opened : 0,
        /* The view holds them most recent first. */
        .activated = views->len - i,
      };

      g_array_append_val (windows, window);
    }
  return g_steal_pointer (&windows);
}

static void
item_click (DkDock *dock, DkItemView *view, const GdkEvent *event)
{
  g_autoptr (GArray) windows = dock->pressed_view == view && dock->pressed != NULL
                                 ? g_array_ref (dock->pressed) : item_for_click (dock, view);
  const char *key = dk_item_view_get_key (view);
  DkItem item = { .key = (char *) key, .windows = windows };
  const DkWindow *target = NULL;

  g_clear_pointer (&dock->pressed, g_array_unref);
  dock->pressed_view = NULL;

  switch (dk_item_click (&item, &target))
    {
    case DK_CLICK_LAUNCH:
      launch_entry (dock, key, NULL);
      break;
    case DK_CLICK_ACTIVATE:
      window_activate (dock, target->handle, event);
      break;
    case DK_CLICK_MINIMISE:
      {
        g_autoptr (GError) error = NULL;

        if (!xfw_window_set_minimized (target->handle, TRUE, &error))
          g_warning ("can't minimise the window: %s", error->message);
      }
      break;
    }
}

/* A new instance, by middle-click: launching an entry starts a new process; a program with
   no entry can't be launched. */
static void
item_new_instance (DkDock *dock, DkItemView *view)
{
  const char *key = dk_item_view_get_key (view);

  if (dk_key_is_entry (key))
    launch_entry (dock, key, NULL);
}

static void
on_menu_window (GtkMenuItem *menu_item, gpointer data)
{
  window_activate (data, g_object_get_data (G_OBJECT (menu_item), "window"),
                   gtk_get_current_event ());
}

static void
on_menu_action (GtkMenuItem *menu_item, gpointer data)
{
  launch_entry (data, g_object_get_data (G_OBJECT (menu_item), "key"),
                g_object_get_data (G_OBJECT (menu_item), "action"));
}

static void
on_menu_keep (GtkMenuItem *menu_item, gpointer data)
{
  DkDock *dock = data;

  dk_store_keep (dock->store, g_object_get_data (G_OBJECT (menu_item), "key"));
}

static void
on_menu_remove (GtkMenuItem *menu_item, gpointer data)
{
  DkDock *dock = data;

  dk_store_remove (dock->store, g_object_get_data (G_OBJECT (menu_item), "key"));
}

static void
on_menu_close_all (GtkMenuItem *menu_item G_GNUC_UNUSED, gpointer data)
{
  GPtrArray *windows = g_object_get_data (data, "windows");
  guint32 time = event_time (NULL);

  for (guint i = 0; i < windows->len; i++)
    xfw_window_close (windows->pdata[i], time, NULL);
}

static GtkWidget *
menu_item (GtkWidget *menu, const char *label, GCallback callback, gpointer data)
{
  GtkWidget *item = gtk_menu_item_new_with_label (label);

  g_signal_connect (item, "activate", callback, data);
  gtk_menu_shell_append (GTK_MENU_SHELL (menu), item);
  gtk_widget_show (item);
  return item;
}

static void
menu_separator (GtkWidget *menu)
{
  GtkWidget *separator = gtk_separator_menu_item_new ();

  gtk_menu_shell_append (GTK_MENU_SHELL (menu), separator);
  gtk_widget_show (separator);
}

static gboolean
menu_has_items (GtkWidget *menu)
{
  g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (menu));

  return children != NULL;
}

/* After the item that was chosen has been activated. */
static gboolean
destroy_menu (gpointer menu)
{
  gtk_widget_destroy (menu);
  return G_SOURCE_REMOVE;
}

static void
on_menu_deactivate (GtkMenuShell *menu, gpointer data)
{
  DkDock *dock = data;

  if (dock->menu == GTK_WIDGET (menu))
    dock->menu = NULL;
  g_idle_add_full (G_PRIORITY_LOW, destroy_menu, g_object_ref (menu), g_object_unref);
}

/* Gone with the item it's attached to. */
static void
on_menu_destroy (GtkWidget *menu, gpointer data)
{
  DkDock *dock = data;

  if (dock->menu == menu)
    dock->menu = NULL;
}

/* The menu opens away from the panel's edge. */
static void
menu_anchors (const DkDock *dock, GdkGravity *rect_anchor, GdkGravity *menu_anchor)
{
  switch (dock->edge)
    {
    case GTK_POS_TOP:
      *rect_anchor = GDK_GRAVITY_SOUTH;
      *menu_anchor = GDK_GRAVITY_NORTH;
      break;
    case GTK_POS_LEFT:
      *rect_anchor = GDK_GRAVITY_EAST;
      *menu_anchor = GDK_GRAVITY_WEST;
      break;
    case GTK_POS_RIGHT:
      *rect_anchor = GDK_GRAVITY_WEST;
      *menu_anchor = GDK_GRAVITY_EAST;
      break;
    default:
      *rect_anchor = GDK_GRAVITY_NORTH;
      *menu_anchor = GDK_GRAVITY_SOUTH;
    }
}

/* The menu, which the dock now holds, opens on the widget. */
static void
menu_show (DkDock *dock, GtkWidget *menu, GtkWidget *widget, const GdkEvent *event)
{
  GdkGravity rect_anchor, menu_anchor;

  g_signal_connect (menu, "deactivate", G_CALLBACK (on_menu_deactivate), dock);
  g_signal_connect (menu, "destroy", G_CALLBACK (on_menu_destroy), dock);
  menu_anchors (dock, &rect_anchor, &menu_anchor);
  gtk_menu_attach_to_widget (GTK_MENU (menu), widget, NULL);
  gtk_menu_popup_at_widget (GTK_MENU (menu), widget, rect_anchor, menu_anchor, event);
}

/* A new menu for the dock to hold, with the one being shown put away. */
static GtkWidget *
menu_begin (DkDock *dock)
{
  if (dock->menu != NULL)
    gtk_menu_popdown (GTK_MENU (dock->menu));
  dock->menu = gtk_menu_new ();
  return dock->menu;
}

static void popup_trash_menu (DkDock *dock, const GdkEvent *event);

void
dk_dock_popup_menu (DkDock *dock, GtkWidget *widget, const GdkEvent *event)
{
  DkItemView *view = view_of (widget);
  const char *key;
  GPtrArray *windows;
  GDesktopAppInfo *info;
  GtkWidget *menu;

  if (view == dock->trash_view)
    {
      popup_trash_menu (dock, event);
      return;
    }
  key = dk_item_view_get_key (view);
  windows = dk_item_view_get_windows (view);
  info = dk_key_is_entry (key) ? entry_info (dock, key) : NULL;
  menu = menu_begin (dock);
  /* The windows by title. */
  for (guint i = 0; i < windows->len; i++)
    {
      const char *title = xfw_window_get_name (windows->pdata[i]);
      GtkWidget *item = menu_item (menu, title != NULL && title[0] != '\0' ? title : "(untitled)",
                                   G_CALLBACK (on_menu_window), dock);

      gtk_label_set_ellipsize (GTK_LABEL (gtk_bin_get_child (GTK_BIN (item))), PANGO_ELLIPSIZE_END);
      gtk_label_set_max_width_chars (GTK_LABEL (gtk_bin_get_child (GTK_BIN (item))),
                                     MENU_TITLE_CHARS);
      g_object_set_data (G_OBJECT (item), "window", windows->pdata[i]);
    }
  /* The desktop entry's actions. */
  if (info != NULL)
    {
      const char *const *actions = g_desktop_app_info_list_actions (info);

      if (actions != NULL && actions[0] != NULL && menu_has_items (menu))
        menu_separator (menu);
      for (guint i = 0; actions != NULL && actions[i] != NULL; i++)
        {
          g_autofree char *name = g_desktop_app_info_get_action_name (info, actions[i]);
          GtkWidget *item = menu_item (menu, name, G_CALLBACK (on_menu_action), dock);

          g_object_set_data_full (G_OBJECT (item), "key", g_strdup (key), g_free);
          g_object_set_data_full (G_OBJECT (item), "action", g_strdup (actions[i]), g_free);
        }
    }
  /* A kept item can be removed even once its entry is gone, as an uninstall leaves it. */
  if (info != NULL || dk_store_is_kept (dock->store, key))
    {
      gboolean kept = dk_store_is_kept (dock->store, key);
      GtkWidget *item;

      if (menu_has_items (menu))
        menu_separator (menu);
      item = menu_item (menu, kept ? "Remove from Dock" : "Keep in Dock",
                        kept ? G_CALLBACK (on_menu_remove) : G_CALLBACK (on_menu_keep), dock);
      g_object_set_data_full (G_OBJECT (item), "key", g_strdup (key), g_free);
    }
  if (windows->len > 0)
    {
      if (menu_has_items (menu) && info == NULL && !dk_store_is_kept (dock->store, key))
        menu_separator (menu);
      menu_item (menu, "Close all windows", G_CALLBACK (on_menu_close_all), menu);
      /* The windows as they are now, for the menu's life. */
      {
        GPtrArray *held = g_ptr_array_new_with_free_func (g_object_unref);

        for (guint i = 0; i < windows->len; i++)
          g_ptr_array_add (held, g_object_ref (windows->pdata[i]));
        g_object_set_data_full (G_OBJECT (menu), "windows", held,
                                (GDestroyNotify) g_ptr_array_unref);
      }
    }
  if (!menu_has_items (menu))
    {
      gtk_widget_destroy (menu);
      dock->menu = NULL;
      return;
    }
  menu_show (dock, menu, widget, event);
}

static gboolean
on_item_button_press (GtkWidget *widget, GdkEventButton *event, gpointer data)
{
  DkDock *dock = data;

  if (event->button == 3)
    {
      dk_dock_popup_menu (dock, widget, (GdkEvent *) event);
      return GDK_EVENT_STOP;
    }
  g_clear_pointer (&dock->pressed, g_array_unref);
  dock->pressed_view = view_of (widget);
  dock->pressed = item_for_click (dock, dock->pressed_view);
  return GDK_EVENT_PROPAGATE;
}

static gboolean
on_item_button_release (GtkWidget *widget, GdkEventButton *event, gpointer data)
{
  DkDock *dock = data;
  DkItemView *view = view_of (widget);
  int width = gtk_widget_get_allocated_width (widget);
  int height = gtk_widget_get_allocated_height (widget);

  /* Released over another item or away from this one: a drag, or a click abandoned. */
  if (event->x < 0 || event->y < 0 || event->x >= width || event->y >= height)
    return GDK_EVENT_PROPAGATE;
  if (event->button == 1)
    item_click (dock, view, (GdkEvent *) event);
  else if (event->button == 2)
    item_new_instance (dock, view);
  else
    return GDK_EVENT_PROPAGATE;
  return GDK_EVENT_STOP;
}

/* The Trash. */

static GtkWindow *
dialog_parent (DkDock *dock)
{
  GtkWidget *toplevel = gtk_widget_get_toplevel (dock->widget);

  return GTK_IS_WINDOW (toplevel) ? GTK_WINDOW (toplevel) : NULL;
}

static void
on_error_response (GtkDialog *dialog, int response G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
  gtk_widget_destroy (GTK_WIDGET (dialog));
}

static void
report_error (DkDock *dock, const char *primary, const char *secondary)
{
  GtkWidget *dialog = gtk_message_dialog_new (dialog_parent (dock), 0, GTK_MESSAGE_ERROR,
                                              GTK_BUTTONS_CLOSE, "%s", primary);

  gtk_message_dialog_format_secondary_text (GTK_MESSAGE_DIALOG (dialog), "%s", secondary);
  g_signal_connect (dialog, "response", G_CALLBACK (on_error_response), NULL);
  gtk_widget_show (dialog);
}

static void
on_trash_emptied (GError *error, gpointer data)
{
  if (error != NULL)
    report_error (data, "Can't empty the Trash", error->message);
}

static void
on_empty_response (GtkDialog *dialog, int response, gpointer data)
{
  DkDock *dock = data;

  gtk_widget_destroy (GTK_WIDGET (dialog));
  if (response == GTK_RESPONSE_ACCEPT)
    dk_trash_empty (dock->trash, on_trash_emptied, dock);
}

/* Emptying is for good, so it asks first, with Caja's words. */
static void
ask_to_empty (DkDock *dock)
{
  GtkWidget *dialog, *button;

  if (dock->dialog != NULL)
    {
      gtk_window_present (GTK_WINDOW (dock->dialog));
      return;
    }
  dialog = gtk_message_dialog_new (dialog_parent (dock), GTK_DIALOG_DESTROY_WITH_PARENT,
                                   GTK_MESSAGE_QUESTION, GTK_BUTTONS_NONE, "%s",
                                   "Empty all items from Trash?");
  gtk_message_dialog_format_secondary_text (
    GTK_MESSAGE_DIALOG (dialog), "%s",
    "If you choose to empty the Trash, all items in it will be permanently lost. "
    "Please note that you can also delete them separately.");
  gtk_dialog_add_button (GTK_DIALOG (dialog), "_Cancel", GTK_RESPONSE_CANCEL);
  button = gtk_dialog_add_button (GTK_DIALOG (dialog), "Empty _Trash", GTK_RESPONSE_ACCEPT);
  gtk_style_context_add_class (gtk_widget_get_style_context (button), "destructive-action");
  gtk_dialog_set_default_response (GTK_DIALOG (dialog), GTK_RESPONSE_CANCEL);
  g_signal_connect (dialog, "response", G_CALLBACK (on_empty_response), dock);
  g_signal_connect (dialog, "destroy", G_CALLBACK (gtk_widget_destroyed), &dock->dialog);
  dock->dialog = dialog;
  gtk_widget_show (dialog);
}

/* In Caja, which is what opens folders on a Caminito Session; the default folder handler
   where there is none. */
static void
open_trash (DkDock *dock G_GNUC_UNUSED)
{
  static const char *const handlers[] = { "caja-folder-handler.desktop",
                                          "caja-browser.desktop", NULL };
  g_autoptr (GList) uris = g_list_prepend (NULL, (gpointer) TRASH_URI);
  GdkDisplay *display = gdk_display_get_default ();
  g_autoptr (GdkAppLaunchContext) context = gdk_display_get_app_launch_context (display);
  g_autoptr (GError) error = NULL;

  for (guint i = 0; handlers[i] != NULL; i++)
    {
      g_autoptr (GDesktopAppInfo) info = g_desktop_app_info_new (handlers[i]);

      if (info != NULL)
        {
          if (!g_app_info_launch_uris (G_APP_INFO (info), uris, G_APP_LAUNCH_CONTEXT (context),
                                       &error))
            g_warning ("can't open the Trash: %s", error->message);
          return;
        }
    }
  if (!g_app_info_launch_default_for_uri (TRASH_URI, G_APP_LAUNCH_CONTEXT (context), &error))
    g_warning ("can't open the Trash: %s", error->message);
}

static void
on_trash_open (GtkMenuItem *menu_item G_GNUC_UNUSED, gpointer data)
{
  open_trash (data);
}

static void
on_trash_empty (GtkMenuItem *menu_item G_GNUC_UNUSED, gpointer data)
{
  ask_to_empty (data);
}

static void
popup_trash_menu (DkDock *dock, const GdkEvent *event)
{
  GtkWidget *menu = menu_begin (dock);
  GtkWidget *empty;

  menu_item (menu, "Open", G_CALLBACK (on_trash_open), dock);
  empty = menu_item (menu, "Empty Trash", G_CALLBACK (on_trash_empty), dock);
  /* There is nothing to empty, as in Caja. */
  gtk_widget_set_sensitive (empty, dk_trash_is_full (dock->trash));
  menu_show (dock, menu, dk_item_view_get_widget (dock->trash_view), event);
}

static gboolean
on_trash_button_press (GtkWidget *widget G_GNUC_UNUSED, GdkEventButton *event, gpointer data)
{
  if (event->button != 3)
    return GDK_EVENT_PROPAGATE;
  popup_trash_menu (data, (GdkEvent *) event);
  return GDK_EVENT_STOP;
}

static gboolean
on_trash_button_release (GtkWidget *widget, GdkEventButton *event, gpointer data)
{
  /* Released away from the item: a click abandoned. */
  if (event->button != 1 || event->x < 0 || event->y < 0
      || event->x >= gtk_widget_get_allocated_width (widget)
      || event->y >= gtk_widget_get_allocated_height (widget))
    return GDK_EVENT_PROPAGATE;
  open_trash (data);
  return GDK_EVENT_STOP;
}

/* The Trash is full or empty, and shows it. */
static void
update_trash_view (DkDock *dock)
{
  static const char *const full[] = { "user-trash-full", "user-trash", NULL };
  static const char *const empty[] = { "user-trash", NULL };
  g_autoptr (GIcon) icon = g_themed_icon_new_from_names ((char **) (dk_trash_is_full (dock->trash)
                                                                      ? full : empty),
                                                         -1);
  g_autoptr (GPtrArray) none = g_ptr_array_new ();

  dk_item_view_set_geometry (dock->trash_view, dock->thickness, dock->edge);
  dk_item_view_set (dock->trash_view, TRASH_URI, icon, "Trash", none, FALSE, FALSE);
}

static void
on_trash_changed (gpointer data)
{
  update_trash_view (data);
}

static void
on_put_failed (const char *uri, GError *error, gpointer data)
{
  g_autoptr (GFile) file = g_file_new_for_uri (uri);
  g_autofree char *name = g_file_get_basename (file);
  g_autofree char *primary = g_strdup_printf ("Can't move “%s” to the Trash", name);

  g_warning ("can't move %s to the Trash: %s", uri, error->message);
  report_error (data, primary, error->message);
}

void
dk_dock_drop_uris (DkDock *dock, const char *const *uris)
{
  dk_trash_put (dock->trash, uris, on_put_failed, NULL, dock);
}

gboolean
dk_dock_trash_is_full (DkDock *dock)
{
  return dk_trash_is_full (dock->trash);
}

GtkWidget *
dk_dock_get_trash (DkDock *dock)
{
  return dk_item_view_get_widget (dock->trash_view);
}

GtkWidget *
dk_dock_get_dialog (DkDock *dock)
{
  return dock->dialog;
}

/* Files dropped on the Trash go to it, whichever action the source offers: the files are
   put in the Trash here, so the source has nothing to delete. */
static gboolean
on_trash_drag_motion (GtkWidget *widget, GdkDragContext *context, int x G_GNUC_UNUSED,
                      int y G_GNUC_UNUSED, guint time, gpointer data G_GNUC_UNUSED)
{
  GdkDragAction offered = gdk_drag_context_get_actions (context);
  GdkAtom target = gtk_drag_dest_find_target (widget, context, NULL);
  GdkDragAction action = target == GDK_NONE ? 0
                         : offered & GDK_ACTION_MOVE ? GDK_ACTION_MOVE
                         : offered & GDK_ACTION_COPY ? GDK_ACTION_COPY : 0;

  gdk_drag_status (context, action, time);
  return action != 0;
}

static gboolean
on_trash_drag_drop (GtkWidget *widget, GdkDragContext *context, int x G_GNUC_UNUSED,
                    int y G_GNUC_UNUSED, guint time, gpointer data G_GNUC_UNUSED)
{
  GdkAtom target = gtk_drag_dest_find_target (widget, context, NULL);

  if (target == GDK_NONE)
    return FALSE;
  gtk_drag_get_data (widget, context, target, time);
  return TRUE;
}

static void
on_trash_drag_data_received (GtkWidget *widget G_GNUC_UNUSED, GdkDragContext *context,
                             int x G_GNUC_UNUSED, int y G_GNUC_UNUSED,
                             GtkSelectionData *selection, guint info G_GNUC_UNUSED, guint time,
                             gpointer data)
{
  g_auto (GStrv) uris = gtk_selection_data_get_uris (selection);

  if (uris != NULL)
    dk_dock_drop_uris (data, (const char *const *) uris);
  gtk_drag_finish (context, uris != NULL, FALSE, time);
}

static void
trash_new (DkDock *dock)
{
  static const GtkTargetEntry targets[] = {
    { (char *) "text/uri-list", 0, 0 },
  };
  g_autoptr (GFile) location = g_file_new_for_uri (TRASH_URI);
  GtkWidget *widget;

  dock->trash = dk_trash_new (location);
  dk_trash_set_changed (dock->trash, on_trash_changed, dock);
  dock->trash_view = dk_item_view_new ();
  widget = dk_item_view_get_widget (dock->trash_view);
  g_signal_connect (widget, "button-press-event", G_CALLBACK (on_trash_button_press), dock);
  g_signal_connect (widget, "button-release-event", G_CALLBACK (on_trash_button_release), dock);
  gtk_drag_dest_set (widget, 0, targets, G_N_ELEMENTS (targets),
                     GDK_ACTION_MOVE | GDK_ACTION_COPY);
  g_signal_connect (widget, "drag-motion", G_CALLBACK (on_trash_drag_motion), dock);
  g_signal_connect (widget, "drag-drop", G_CALLBACK (on_trash_drag_drop), dock);
  g_signal_connect (widget, "drag-data-received", G_CALLBACK (on_trash_drag_data_received),
                    dock);
  update_trash_view (dock);
}

GPtrArray *
dk_dock_get_keys (DkDock *dock)
{
  GPtrArray *keys = g_ptr_array_new_with_free_func (g_free);
  g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (dock->box));

  for (GList *l = children; l != NULL; l = l->next)
    g_ptr_array_add (keys, g_strdup (dk_item_view_get_key (view_of (l->data))));
  return keys;
}

GtkWidget *
dk_dock_get_item (DkDock *dock, const char *key)
{
  DkItemView *view = g_hash_table_lookup (dock->views, key);

  return view != NULL ? dk_item_view_get_widget (view) : NULL;
}

/* The number of items before the point (along the dock), and so where a drop goes. */
static int
slot_at (DkDock *dock, int x, int y)
{
  g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (dock->box));
  int slot = 0;

  for (GList *l = children; l != NULL; l = l->next, slot++)
    {
      GtkAllocation a;

      gtk_widget_get_allocation (l->data, &a);
      if (is_horizontal (dock) ? x < a.x + a.width / 2 : y < a.y + a.height / 2)
        return slot;
    }
  return slot;
}

/* Only the kept items are placed: the slot counts those before it. */
static guint
kept_position (DkDock *dock, int slot)
{
  const char *const *kept = dk_store_get (dock->store);
  guint position = 0;
  int i = 0;
  g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (dock->box));

  for (GList *l = children; l != NULL && i < slot; l = l->next, i++)
    if (g_strv_contains (kept, dk_item_view_get_key (view_of (l->data))))
      position++;
  return position;
}

void
dk_dock_drop_key (DkDock *dock, const char *key, int x, int y)
{
  if (key == NULL || !dk_key_is_entry (key))
    return;
  dk_store_move (dock->store, key, kept_position (dock, slot_at (dock, x, y)));
}

static gboolean
on_box_draw_after (GtkWidget *box, cairo_t *cr, gpointer data)
{
  DkDock *dock = data;
  g_autoptr (GList) children = NULL;
  GtkStyleContext *context = gtk_widget_get_style_context (box);
  GdkRGBA color;
  GtkAllocation a;
  double at;
  GList *l;
  int slot;

  if (dock->drop_slot < 0)
    return FALSE;
  children = gtk_container_get_children (GTK_CONTAINER (dock->box));
  slot = dock->drop_slot;
  l = g_list_nth (children, slot);
  if (l != NULL)
    {
      gtk_widget_get_allocation (l->data, &a);
      at = is_horizontal (dock) ? a.x : a.y;
    }
  else if (children != NULL)
    {
      gtk_widget_get_allocation (g_list_last (children)->data, &a);
      at = is_horizontal (dock) ? a.x + a.width : a.y + a.height;
    }
  else
    return FALSE;
  if (!gtk_style_context_lookup_color (context, "theme_selected_bg_color", &color))
    gdk_rgba_parse (&color, "#3daee9");
  gdk_cairo_set_source_rgba (cr, &color);
  if (is_horizontal (dock))
    cairo_rectangle (cr, at - 1, 0, 2, gtk_widget_get_allocated_height (box));
  else
    cairo_rectangle (cr, 0, at - 1, gtk_widget_get_allocated_width (box), 2);
  cairo_fill (cr);
  return FALSE;
}

static void
on_drag_leave (GtkWidget *box, GdkDragContext *context G_GNUC_UNUSED, guint time G_GNUC_UNUSED,
               gpointer data)
{
  DkDock *dock = data;

  dock->drop_slot = -1;
  gtk_widget_queue_draw (box);
}

/* An item is moved; a launcher from the main menu, which offers only a copy, is copied. */
static gboolean
on_drag_motion (GtkWidget *box, GdkDragContext *context, int x, int y, guint time, gpointer data)
{
  DkDock *dock = data;
  GdkAtom target = gtk_drag_dest_find_target (box, context, NULL);
  GdkDragAction offered = gdk_drag_context_get_actions (context);
  GdkDragAction action = target == GDK_NONE ? 0
                         : offered & GDK_ACTION_MOVE ? GDK_ACTION_MOVE
                         : offered & GDK_ACTION_COPY ? GDK_ACTION_COPY : 0;

  gdk_drag_status (context, action, time);
  if (action == 0)
    {
      on_drag_leave (box, context, time, dock);
      return FALSE;
    }
  dock->drop_slot = slot_at (dock, x, y);
  gtk_widget_queue_draw (box);
  return TRUE;
}

static gboolean
on_drag_drop (GtkWidget *box, GdkDragContext *context, int x G_GNUC_UNUSED,
              int y G_GNUC_UNUSED, guint time, gpointer data G_GNUC_UNUSED)
{
  GdkAtom target = gtk_drag_dest_find_target (box, context, NULL);

  if (target == GDK_NONE)
    return FALSE;
  gtk_drag_get_data (box, context, target, time);
  return TRUE;
}

/* A launcher dragged from the main menu comes as the URI of its desktop entry. The key is
   the entry's id where it has one, and its path where it hasn't. */
static char *
key_of_uri (const char *uri)
{
  g_autofree char *path = g_filename_from_uri (uri, NULL, NULL);
  g_autoptr (GDesktopAppInfo) info = NULL;

  if (path == NULL || !g_str_has_suffix (path, ".desktop"))
    return NULL;
  info = g_desktop_app_info_new_from_filename (path);
  if (info == NULL)
    return NULL;
  return g_strdup (g_app_info_get_id (G_APP_INFO (info)) != NULL
                     ? g_app_info_get_id (G_APP_INFO (info)) : path);
}

static void
on_drag_data_received (GtkWidget *box G_GNUC_UNUSED, GdkDragContext *context, int x, int y,
                       GtkSelectionData *selection, guint info G_GNUC_UNUSED, guint time,
                       gpointer data)
{
  DkDock *dock = data;
  g_autofree char *internal = NULL;
  g_auto (GStrv) uris = gtk_selection_data_get_uris (selection);
  gboolean done = FALSE;

  dock->drop_slot = -1;
  gtk_widget_queue_draw (dock->box);
  if (gtk_selection_data_get_data_type (selection) == gdk_atom_intern_static_string (DK_DRAG_TARGET))
    {
      internal = g_strndup ((const char *) gtk_selection_data_get_data (selection),
                            gtk_selection_data_get_length (selection));
      dk_dock_drop_key (dock, internal, x, y);
      done = TRUE;
    }
  else
    for (guint i = 0; uris != NULL && uris[i] != NULL; i++)
      {
        g_autofree char *key = key_of_uri (uris[i]);

        if (key != NULL)
          {
            dk_dock_drop_key (dock, key, x, y);
            done = TRUE;
          }
      }
  gtk_drag_finish (context, done, FALSE, time);
}

void
dk_dock_ignore_app_id (DkDock *dock, const char *app_id)
{
  GList *windows;

  g_free (dock->ignored);
  dock->ignored = g_strdup (app_id);
  for (windows = xfw_screen_get_windows (dock->screen); windows != NULL; windows = windows->next)
    if (g_strcmp0 (window_app_id (dock, windows->data), app_id) == 0)
      window_forget (dock, windows->data);
}

GtkWidget *
dk_dock_get_menu (DkDock *dock)
{
  return dock->menu;
}

GtkWidget *
dk_dock_get_widget (DkDock *dock)
{
  return dock->widget;
}

void
dk_dock_set_layout (DkDock *dock, guint thickness, GtkPositionType edge)
{
  dock->thickness = thickness;
  dock->edge = edge;
  /* The items stay square in a panel or a window that is larger than they are. */
  gtk_widget_set_halign (dock->widget, GTK_ALIGN_START);
  gtk_widget_set_valign (dock->widget, GTK_ALIGN_START);
  gtk_orientable_set_orientation (GTK_ORIENTABLE (dock->widget),
                                  is_horizontal (dock) ? GTK_ORIENTATION_HORIZONTAL
                                                       : GTK_ORIENTATION_VERTICAL);
  gtk_orientable_set_orientation (GTK_ORIENTABLE (dock->box),
                                  is_horizontal (dock) ? GTK_ORIENTATION_HORIZONTAL
                                                       : GTK_ORIENTATION_VERTICAL);
  /* The separator's line is across the row, a little shorter than the items are thick. */
  gtk_orientable_set_orientation (GTK_ORIENTABLE (dock->separator),
                                  is_horizontal (dock) ? GTK_ORIENTATION_VERTICAL
                                                       : GTK_ORIENTATION_HORIZONTAL);
  gtk_widget_set_halign (dock->separator, is_horizontal (dock) ? GTK_ALIGN_CENTER
                                                               : GTK_ALIGN_FILL);
  gtk_widget_set_valign (dock->separator, is_horizontal (dock) ? GTK_ALIGN_FILL
                                                               : GTK_ALIGN_CENTER);
  gtk_widget_set_margin_top (dock->separator, is_horizontal (dock) ? thickness / 5 : 6);
  gtk_widget_set_margin_bottom (dock->separator, is_horizontal (dock) ? thickness / 5 : 6);
  gtk_widget_set_margin_start (dock->separator, is_horizontal (dock) ? 6 : thickness / 5);
  gtk_widget_set_margin_end (dock->separator, is_horizontal (dock) ? 6 : thickness / 5);
  update_trash_view (dock);
  schedule_refresh (dock);
}

DkDock *
dk_dock_new (DkStore *store)
{
  static const GtkTargetEntry targets[] = {
    { (char *) DK_DRAG_TARGET, GTK_TARGET_SAME_APP, 0 },
    { (char *) "text/uri-list", 0, 1 },
  };
  DkDock *dock = g_new0 (DkDock, 1);
  GList *windows;

  dock->store = store;
  dock->catalog = dk_catalog_new_installed ();
  dock->thickness = 48;
  dock->edge = GTK_POS_BOTTOM;
  dock->drop_slot = -1;
  dock->windows = g_hash_table_new_full (NULL, NULL, NULL, window_info_free);
  dock->views = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  dock->infos = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, g_object_unref);
  dock->widget = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 0);
  dock->box = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 0);
  dock->separator = gtk_separator_new (GTK_ORIENTATION_VERTICAL);
  /* The theme's separator is a border, which a dark panel hides: the item's own colour,
     faint, as the indicator's is. */
  {
    g_autoptr (GtkCssProvider) css = gtk_css_provider_new ();

    gtk_css_provider_load_from_data (css, "separator { background-color: currentColor; "
                                          "opacity: 0.35; min-width: 1px; min-height: 1px; }",
                                     -1, NULL);
    gtk_style_context_add_provider (gtk_widget_get_style_context (dock->separator),
                                    GTK_STYLE_PROVIDER (css),
                                    GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
  }
  trash_new (dock);
  gtk_box_pack_start (GTK_BOX (dock->widget), dock->box, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (dock->widget), dock->separator, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (dock->widget), dk_item_view_get_widget (dock->trash_view), FALSE,
                      FALSE, 0);
  gtk_widget_show_all (dock->widget);
  g_object_set_data_full (G_OBJECT (dock->box), "dk-dock", dock, NULL);
  g_signal_connect_after (dock->box, "draw", G_CALLBACK (on_box_draw_after), dock);
  gtk_drag_dest_set (dock->box, 0, targets, G_N_ELEMENTS (targets),
                     GDK_ACTION_MOVE | GDK_ACTION_COPY);
  g_signal_connect (dock->box, "drag-motion", G_CALLBACK (on_drag_motion), dock);
  g_signal_connect (dock->box, "drag-leave", G_CALLBACK (on_drag_leave), dock);
  g_signal_connect (dock->box, "drag-drop", G_CALLBACK (on_drag_drop), dock);
  g_signal_connect (dock->box, "drag-data-received", G_CALLBACK (on_drag_data_received), dock);
  dk_store_set_changed (store, on_store_changed, dock);
  dock->monitor = g_app_info_monitor_get ();
  g_signal_connect (dock->monitor, "changed", G_CALLBACK (on_apps_changed), dock);
  dock->screen = xfw_screen_get_default ();
  g_signal_connect (dock->screen, "window-opened", G_CALLBACK (on_window_opened), dock);
  g_signal_connect (dock->screen, "window-closed", G_CALLBACK (on_window_closed_on_screen), dock);
  for (windows = xfw_screen_get_windows (dock->screen); windows != NULL; windows = windows->next)
    window_remember (dock, windows->data);
  schedule_refresh (dock);
  return dock;
}

void
dk_dock_free (DkDock *dock)
{
  GHashTableIter iter;
  gpointer window;

  if (dock == NULL)
    return;
  if (dock->refresh_id != 0)
    g_source_remove (dock->refresh_id);
  g_signal_handlers_disconnect_by_data (dock->screen, dock);
  g_signal_handlers_disconnect_by_data (dock->monitor, dock);
  g_hash_table_iter_init (&iter, dock->windows);
  while (g_hash_table_iter_next (&iter, &window, NULL))
    g_signal_handlers_disconnect_by_data (window, dock);
  dk_store_set_changed (dock->store, NULL, NULL);
  /* Its items' handlers are the dock's. */
  if (dock->menu != NULL)
    {
      g_signal_handlers_disconnect_by_data (dock->menu, dock);
      gtk_widget_destroy (dock->menu);
    }
  if (dock->dialog != NULL)
    {
      g_signal_handlers_disconnect_by_data (dock->dialog, dock);
      gtk_widget_destroy (dock->dialog);
    }
  dk_trash_free (dock->trash);
  g_clear_object (&dock->screen);
  g_object_unref (dock->monitor);
  g_hash_table_unref (dock->windows);
  g_hash_table_unref (dock->views);
  g_hash_table_unref (dock->infos);
  g_clear_pointer (&dock->pressed, g_array_unref);
  dk_catalog_free (dock->catalog);
  g_free (dock->ignored);
  g_free (dock);
}
