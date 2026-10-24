/*
 * The Dock (CMN-219), the Kept folders after its separator (CMN-239), the Removable drive
 * items after them (CMN-240), and the Trash that ends its row (CMN-220).
 */

#include <gio/gdesktopappinfo.h>
#include <libxfce4windowing/libxfce4windowing.h>

#include "dk-activation.h"
#include "dk-dock.h"
#include "dk-drive.h"
#include "dk-feedback.h"
#include "dk-folder.h"
#include "dk-item.h"
#include "dk-launcher.h"
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
  /* The dock: the items' box, a separator, and the items after it. */
  GtkWidget       *widget;
  /* The Dock items. */
  GtkWidget       *box;
  GtkWidget       *separator;
  /* The items after the separator: the Kept folders (CMN-239), in the user's order, the
     Removable drive items (CMN-240), and then the Trash. */
  GtkWidget       *after;
  /* A Kept folder's URI to its DkItemView, whose widget `after` holds, and to the monitor that
     tells when the folder is deleted or its icon changes. */
  GHashTable      *folder_views;
  GHashTable      *folder_monitors;
  /* The removable drives' mounts, and a drive's key (its root's URI) to its DkItemView, whose
     widget `after` holds. */
  DkDrives        *drives;
  GHashTable      *drive_views;
  /* The folders that are gone are looked for again until they come back. */
  guint            gone_id;
  /* The popup grid of a folder's contents, and the URI of the folder it shows. */
  GtkWidget       *popup;
  char            *popup_uri;
  /* The work on files dropped on a folder or a drive, and the ejects, which a dock that is
     freed no longer hears the end of. */
  GCancellable    *cancellable;
  /* Where a Kept folder would be dropped, as the number of Kept folders before it; -1 when
     none is over. */
  int              after_slot;
  DkTrash         *trash;
  DkItemView      *trash_view;
  /* The question being shown. */
  GtkWidget       *dialog;
  /* The error being shown: one at a time, however many files a drop couldn't put. */
  GtkWidget       *error_dialog;
  DkStore         *store;
  DkCatalog       *catalog;
  /* The launch pulses and the attention bounces and highlights (CMN-221), and the source that
     refreshes the items when the next of them ends by itself. */
  DkFeedback      *feedback;
  guint            feedback_id;
  /* What programs tell their items through Unity's LauncherEntry signal (CMN-227), and the
     session bus subscriptions that bring it and tell when a sender leaves. */
  DkLauncher      *launcher;
  GDBusConnection *bus;
  guint            update_subscription;
  guint            owner_subscription;
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
#define LAUNCHER_ENTRY_INTERFACE "com.canonical.Unity.LauncherEntry"
#define TRASH_URI "trash:///"

static void schedule_refresh (DkDock *dock);
static void refresh_after (DkDock *dock);
static void popup_folder_menu (DkDock *dock, DkItemView *view, const GdkEvent *event);
static void popup_drive_menu (DkDock *dock, DkItemView *view, const GdkEvent *event);
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

/* The item pulses until a window of it opened after the launch maps (CMN-221). */
static void
launch_pulse (DkDock *dock, const char *key)
{
  dk_feedback_launch (dock->feedback, key, dock->clock, g_get_monotonic_time ());
  schedule_refresh (dock);
}

/* The token for the launch of the entry with key, from the dock's window. */
static char *
launch_token (DkDock *dock, const char *key)
{
  GtkWidget *toplevel = gtk_widget_get_toplevel (dock->box);
  g_autofree char *name = g_path_get_basename (key);
  g_autofree char *app_id = g_str_has_suffix (name, ".desktop")
                              ? g_strndup (name, strlen (name) - strlen (".desktop")) : NULL;

  return dk_activation_token_new (gtk_widget_get_window (toplevel), app_id);
}

/* The launch carries an xdg-activation token: the program maps its first window with it
   (CMN-221). GDK 3's launch context makes one only for an entry with StartupNotify=true, and
   GIO puts that one over any the environment has, so a launch with the dock's own token goes
   through a plain context, which makes none. */
static void
launch_entry (DkDock *dock, const char *key, const char *action)
{
  GDesktopAppInfo *info = entry_info (dock, key);
  g_autoptr (GAppLaunchContext) context = NULL;
  g_autoptr (GError) error = NULL;
  g_autofree char *token = NULL;

  if (info == NULL)
    {
      g_warning ("can't launch %s: no such desktop entry", key);
      return;
    }
  token = launch_token (dock, key);
  if (token != NULL)
    {
      context = g_app_launch_context_new ();
      /* The standard variable, and the one GTK 3 reads as the token on Wayland. */
      g_app_launch_context_setenv (context, "XDG_ACTIVATION_TOKEN", token);
      g_app_launch_context_setenv (context, "DESKTOP_STARTUP_ID", token);
    }
  else
    context = G_APP_LAUNCH_CONTEXT (gdk_display_get_app_launch_context (gdk_display_get_default ()));
  if (action != NULL)
    g_desktop_app_info_launch_action (info, action, context);
  else if (!g_app_info_launch (G_APP_INFO (info), NULL, context, &error))
    {
      g_warning ("can't launch %s: %s", key, error->message);
      return;
    }
  launch_pulse (dock, key);
}

static gboolean
on_feedback_end (gpointer data)
{
  DkDock *dock = data;

  dock->feedback_id = 0;
  schedule_refresh (dock);
  return G_SOURCE_REMOVE;
}

/* A pulse times out and a bounce ends with no event from a window, so the dock looks again
   then. */
static void
schedule_feedback_end (DkDock *dock, gint64 now)
{
  gint64 deadline = dk_feedback_deadline (dock->feedback, now);

  g_clear_handle_id (&dock->feedback_id, g_source_remove);
  if (deadline >= 0)
    dock->feedback_id = g_timeout_add (MAX (1, (deadline - now + 999) / 1000), on_feedback_end,
                                       dock);
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
  gint64 now;

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
        .urgent = xfw_window_is_urgent (info->window),
        .opened = info->opened,
        .activated = info->activated,
      };

      g_array_append_val (windows, window);
    }
  items = dk_items_build (dock->catalog, dk_store_get (dock->store),
                          (DkWindow *) windows->data, windows->len);
  now = g_get_monotonic_time ();
  dk_launcher_sync (dock->launcher, items);
  for (guint i = 0; i < items->len; i++)
    {
      const DkItem *item = &g_array_index (items, DkItem, i);

      dk_feedback_set_launcher_urgent (dock->feedback, item->key,
                                       dk_launcher_get (dock->launcher, item->key).urgent);
    }
  dk_feedback_update (dock->feedback, items, now);
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
      {
        DkFeedbackState state = dk_feedback_get (dock->feedback, item->key, now);

        DkLauncherEntry entry = dk_launcher_get (dock->launcher, item->key);

        dk_item_view_set_feedback (view, &state);
        dk_item_view_set_launcher (view, &entry);
      }
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
  refresh_after (dock);
  schedule_feedback_end (dock, now);
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

/* A program's Update signal, from any sender and any object path (CMN-227): (s app_uri,
   a{sv} properties). */
static void
on_launcher_update (GDBusConnection *connection G_GNUC_UNUSED, const char *sender,
                    const char *object_path G_GNUC_UNUSED,
                    const char *interface_name G_GNUC_UNUSED,
                    const char *signal_name G_GNUC_UNUSED, GVariant *parameters, gpointer data)
{
  DkDock *dock = data;
  const char *app_uri;
  g_autoptr (GVariant) properties = NULL;

  if (!g_variant_is_of_type (parameters, G_VARIANT_TYPE ("(sa{sv})")))
    return;
  g_variant_get (parameters, "(&s@a{sv})", &app_uri, &properties);
  if (dk_launcher_update (dock->launcher, dock->catalog, sender, app_uri, properties))
    schedule_refresh (dock);
}

/* A program that leaves the bus takes its badge, bar and urgency with it. */
static void
on_name_owner_changed (GDBusConnection *connection G_GNUC_UNUSED,
                       const char *sender G_GNUC_UNUSED,
                       const char *object_path G_GNUC_UNUSED,
                       const char *interface_name G_GNUC_UNUSED,
                       const char *signal_name G_GNUC_UNUSED, GVariant *parameters, gpointer data)
{
  DkDock *dock = data;
  const char *name, *old_owner, *new_owner;

  if (!g_variant_is_of_type (parameters, G_VARIANT_TYPE ("(sss)")))
    return;
  g_variant_get (parameters, "(&s&s&s)", &name, &old_owner, &new_owner);
  /* Senders are unique names; a well-known name's change is its owner's, which is not gone. */
  if (name[0] == ':' && new_owner[0] == '\0' && dk_launcher_clear_sender (dock->launcher, name))
    schedule_refresh (dock);
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
  if (g_hash_table_lookup (dock->folder_views, dk_item_view_get_key (view)) == view)
    {
      popup_folder_menu (dock, view, event);
      return;
    }
  if (g_hash_table_lookup (dock->drive_views, dk_item_view_get_key (view)) == view)
    {
      popup_drive_menu (dock, view, event);
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

/* The first error is shown until it is closed; those that come meanwhile are only logged. */
static void
report_error (DkDock *dock, const char *primary, const char *secondary)
{
  GtkWidget *dialog;

  if (dock->error_dialog != NULL)
    return;
  dialog = gtk_message_dialog_new (dialog_parent (dock), GTK_DIALOG_DESTROY_WITH_PARENT,
                                   GTK_MESSAGE_ERROR, GTK_BUTTONS_CLOSE, "%s", primary);
  gtk_message_dialog_format_secondary_text (GTK_MESSAGE_DIALOG (dialog), "%s", secondary);
  g_signal_connect (dialog, "response", G_CALLBACK (on_error_response), NULL);
  g_signal_connect (dialog, "destroy", G_CALLBACK (gtk_widget_destroyed), &dock->error_dialog);
  dock->error_dialog = dialog;
  gtk_widget_show (dialog);
}

static void
on_trash_emptied (GError *error, gpointer data)
{
  if (error != NULL)
    {
      g_warning ("can't empty the Trash: %s", error->message);
      report_error (data, "Can't empty the Trash", error->message);
    }
}

/* One emptying at a time: a second would find the first's files gone, and report it. */
static void
empty_trash (DkDock *dock)
{
  if (!dk_trash_is_emptying (dock->trash))
    dk_trash_empty (dock->trash, on_trash_emptied, dock);
}

static void
on_empty_response (GtkDialog *dialog, int response, gpointer data)
{
  DkDock *dock = data;

  gtk_widget_destroy (GTK_WIDGET (dialog));
  if (response == GTK_RESPONSE_ACCEPT)
    empty_trash (dock);
}

/* Whether Caja's preference is to ask before emptying, as it is unless the user said not to,
   or where Caja's settings aren't installed. */
static gboolean
should_confirm_empty (void)
{
  GSettingsSchemaSource *source = g_settings_schema_source_get_default ();
  g_autoptr (GSettingsSchema) schema = source != NULL
    ? g_settings_schema_source_lookup (source, "org.mate.caja.preferences", TRUE)
    : NULL;
  g_autoptr (GSettings) settings = NULL;

  if (schema == NULL || !g_settings_schema_has_key (schema, "confirm-trash"))
    return TRUE;
  settings = g_settings_new_full (schema, NULL, NULL);
  return g_settings_get_boolean (settings, "confirm-trash");
}

/* Emptying is for good, so it asks first, with Caja's words, unless Caja is set not to. */
static void
ask_to_empty (DkDock *dock)
{
  GtkWidget *dialog, *button;

  if (dock->dialog != NULL)
    {
      gtk_window_present (GTK_WINDOW (dock->dialog));
      return;
    }
  if (!should_confirm_empty ())
    {
      empty_trash (dock);
      return;
    }
  dialog = gtk_message_dialog_new (dialog_parent (dock), GTK_DIALOG_DESTROY_WITH_PARENT,
                                   GTK_MESSAGE_QUESTION, GTK_BUTTONS_NONE, "%s",
                                   "Empty all items from Trash?");
  gtk_message_dialog_format_secondary_text (
    GTK_MESSAGE_DIALOG (dialog), "%s",
    "All items in the Trash will be permanently deleted.");
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
open_in_caja (DkDock *dock G_GNUC_UNUSED, const char *uri)
{
  static const char *const handlers[] = { "caja-folder-handler.desktop",
                                          "caja-browser.desktop", NULL };
  g_autoptr (GList) uris = g_list_prepend (NULL, (gpointer) uri);
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
            g_warning ("can't open %s: %s", uri, error->message);
          return;
        }
    }
  if (!g_app_info_launch_default_for_uri (uri, G_APP_LAUNCH_CONTEXT (context), &error))
    g_warning ("can't open %s: %s", uri, error->message);
}

static void
open_trash (DkDock *dock)
{
  open_in_caja (dock, TRASH_URI);
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
  /* There is nothing to empty, as in Caja, or it is being emptied. */
  gtk_widget_set_sensitive (empty, dk_trash_is_full (dock->trash)
                                     && !dk_trash_is_emptying (dock->trash));
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

/* Whether a point in an item after the separator is over the part files are dropped on. The
   part along the dock at the start of the item, a quarter of it (and of a Kept folder's end
   too), is where a folder dropped from Caja is kept before it instead (CMN-239). */
static gboolean
over_item (DkDock *dock, GtkWidget *widget, int x, int y, gboolean both_ends)
{
  int along = is_horizontal (dock) ? x : y;
  int length = is_horizontal (dock) ? gtk_widget_get_allocated_width (widget)
                                    : gtk_widget_get_allocated_height (widget);

  return along >= length / 4 && (!both_ends || along < length - length / 4);
}

/* A drag of files over an item that takes them, the Trash or a Removable drive item, at the
   action it shows, preferred if the source offers it: the files are put there by the dock,
   so the source has nothing to delete either way. */
static gboolean
files_drag_motion (DkDock *dock, GtkWidget *widget, GdkDragContext *context, int x, int y,
                   guint time, GdkDragAction preferred)
{
  GdkDragAction offered = gdk_drag_context_get_actions (context);
  GdkAtom target = gtk_drag_dest_find_target (widget, context, NULL);
  GdkDragAction action = target == GDK_NONE ? 0
                         : offered & preferred ? preferred
                         : offered & GDK_ACTION_MOVE ? GDK_ACTION_MOVE
                         : offered & GDK_ACTION_COPY ? GDK_ACTION_COPY : 0;

  /* At its start a folder is kept before it: the items after the separator take it from here. */
  if (!over_item (dock, widget, x, y, FALSE))
    return FALSE;
  gdk_drag_status (context, action, time);
  return action != 0;
}

/* Files dropped on the Trash are moved to it. */
static gboolean
on_trash_drag_motion (GtkWidget *widget, GdkDragContext *context, int x,
                      int y, guint time, gpointer data)
{
  return files_drag_motion (data, widget, context, x, y, time, GDK_ACTION_MOVE);
}

/* The drop of files on the Trash or a Removable drive item, past the start where a folder is
   kept instead. */
static gboolean
on_files_drag_drop (GtkWidget *widget, GdkDragContext *context, int x,
                    int y, guint time, gpointer data)
{
  GdkAtom target = gtk_drag_dest_find_target (widget, context, NULL);

  if (target == GDK_NONE || !over_item (data, widget, x, y, FALSE))
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
  g_signal_connect (widget, "drag-drop", G_CALLBACK (on_files_drag_drop), dock);
  g_signal_connect (widget, "drag-data-received", G_CALLBACK (on_trash_drag_data_received),
                    dock);
  update_trash_view (dock);
}

/* The Kept folders (CMN-239). */

#define FOLDER_CELL_WIDTH 96
#define FOLDER_GRID_COLUMNS 5
#define FOLDER_GRID_MAX_HEIGHT 360
#define GONE_POLL_SECONDS 2

static DkItemView *
folder_view (DkDock *dock, const char *uri)
{
  return g_hash_table_lookup (dock->folder_views, uri);
}

/* Whether the widget is a Kept folder's, not a drive's or the Trash's. */
static gboolean
is_folder_widget (DkDock *dock, GtkWidget *widget)
{
  DkItemView *view = view_of (widget);

  return folder_view (dock, dk_item_view_get_key (view)) == view;
}

/* The name a folder that is gone is shown by: the last part of its URI. */
static char *
folder_fallback_name (GFile *folder)
{
  g_autofree char *name = g_file_get_basename (folder);

  return g_strdup (name != NULL ? name : "Folder");
}

static void
folder_not_available (DkDock *dock, const char *uri)
{
  g_autoptr (GFile) folder = g_file_new_for_uri (uri);
  g_autofree char *name = folder_fallback_name (folder);
  g_autofree char *primary = g_strdup_printf ("“%s” isn't available", name);

  report_error (dock, primary, "The folder is gone, or it is on a drive that isn't connected.");
}

gboolean
dk_dock_folder_is_gone (DkDock *dock G_GNUC_UNUSED, const char *uri)
{
  g_autoptr (GFile) folder = g_file_new_for_uri (uri);

  return !dk_folder_exists (folder);
}

GtkWidget *
dk_dock_get_folder (DkDock *dock, const char *uri)
{
  DkItemView *view = folder_view (dock, uri);

  return view != NULL ? dk_item_view_get_widget (view) : NULL;
}

GPtrArray *
dk_dock_get_folders (DkDock *dock)
{
  GPtrArray *uris = g_ptr_array_new_with_free_func (g_free);
  g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (dock->after));

  for (GList *l = children; l != NULL; l = l->next)
    if (is_folder_widget (dock, l->data))
      g_ptr_array_add (uris, g_strdup (dk_item_view_get_key (view_of (l->data))));
  return uris;
}

GtkWidget *
dk_dock_get_error_dialog (DkDock *dock)
{
  return dock->error_dialog;
}

/* The popup grid. */

static void
popup_close (DkDock *dock)
{
  if (dock->popup != NULL)
    gtk_widget_destroy (dock->popup);
  g_clear_pointer (&dock->popup_uri, g_free);
}

static void
on_popup_destroy (GtkWidget *popup, gpointer data)
{
  DkDock *dock = data;

  if (dock->popup == popup)
    {
      dock->popup = NULL;
      g_clear_pointer (&dock->popup_uri, g_free);
    }
}

/* After the popover has closed itself, with the item chosen in it activated. */
static gboolean
destroy_popup (gpointer popup)
{
  gtk_widget_destroy (popup);
  return G_SOURCE_REMOVE;
}

static void
on_popup_closed (GtkPopover *popup, gpointer data G_GNUC_UNUSED)
{
  g_idle_add_full (G_PRIORITY_LOW, destroy_popup, g_object_ref (popup), g_object_unref);
}

static void
on_popup_entry (GtkFlowBox *grid G_GNUC_UNUSED, GtkFlowBoxChild *child, gpointer data)
{
  DkDock *dock = data;
  GtkWidget *cell = gtk_bin_get_child (GTK_BIN (child));
  const char *uri = g_object_get_data (G_OBJECT (cell), "uri");

  /* A sub-folder opens in Caja, a file with its default program. */
  if (GPOINTER_TO_INT (g_object_get_data (G_OBJECT (cell), "is-folder")))
    open_in_caja (dock, uri);
  else
    {
      GdkDisplay *display = gdk_display_get_default ();
      g_autoptr (GdkAppLaunchContext) context = gdk_display_get_app_launch_context (display);
      g_autoptr (GError) error = NULL;

      if (!g_app_info_launch_default_for_uri (uri, G_APP_LAUNCH_CONTEXT (context), &error))
        {
          g_autoptr (GFile) file = g_file_new_for_uri (uri);
          g_autofree char *name = folder_fallback_name (file);
          g_autofree char *primary = g_strdup_printf ("Can't open “%s”", name);

          g_warning ("can't open %s: %s", uri, error->message);
          report_error (dock, primary, error->message);
        }
    }
  popup_close (dock);
}

static void
on_popup_open_in_caja (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  DkDock *dock = data;
  g_autofree char *uri = g_strdup (dock->popup_uri);

  if (uri != NULL)
    open_in_caja (dock, uri);
  popup_close (dock);
}

static GtkWidget *
popup_cell (const DkFolderEntry *entry)
{
  GtkWidget *cell = gtk_box_new (GTK_ORIENTATION_VERTICAL, 4);
  GtkWidget *image = gtk_image_new_from_gicon (
    entry->icon != NULL ? entry->icon : G_ICON (g_themed_icon_new (entry->is_folder ? "folder" : "text-x-generic")),
    GTK_ICON_SIZE_DIALOG);
  GtkWidget *label = gtk_label_new (entry->name);

  gtk_image_set_pixel_size (GTK_IMAGE (image), 48);
  gtk_label_set_ellipsize (GTK_LABEL (label), PANGO_ELLIPSIZE_MIDDLE);
  gtk_label_set_line_wrap (GTK_LABEL (label), TRUE);
  gtk_label_set_line_wrap_mode (GTK_LABEL (label), PANGO_WRAP_WORD_CHAR);
  gtk_label_set_lines (GTK_LABEL (label), 2);
  gtk_label_set_max_width_chars (GTK_LABEL (label), 12);
  gtk_label_set_justify (GTK_LABEL (label), GTK_JUSTIFY_CENTER);
  gtk_box_pack_start (GTK_BOX (cell), image, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (cell), label, FALSE, FALSE, 0);
  gtk_widget_set_size_request (cell, FOLDER_CELL_WIDTH, -1);
  gtk_widget_set_margin_top (cell, 4);
  gtk_widget_set_margin_bottom (cell, 4);
  gtk_widget_set_tooltip_text (cell, entry->name);
  g_object_set_data_full (G_OBJECT (cell), "uri", g_strdup (entry->uri), g_free);
  g_object_set_data (G_OBJECT (cell), "is-folder", GINT_TO_POINTER (entry->is_folder));
  return cell;
}

static GtkPositionType
popup_position (const DkDock *dock)
{
  /* Away from the panel's edge. */
  switch (dock->edge)
    {
    case GTK_POS_TOP:
      return GTK_POS_BOTTOM;
    case GTK_POS_LEFT:
      return GTK_POS_RIGHT;
    case GTK_POS_RIGHT:
      return GTK_POS_LEFT;
    default:
      return GTK_POS_TOP;
    }
}

/* The grid of the folder's contents, icon and name, folders first and then by name, and Open
   in Caja at the bottom. It doesn't navigate. */
static void
popup_open (DkDock *dock, DkItemView *view)
{
  const char *uri = dk_item_view_get_key (view);
  g_autoptr (GFile) folder = g_file_new_for_uri (uri);
  g_autoptr (GPtrArray) entries = NULL;
  g_autoptr (GError) error = NULL;
  GtkWidget *box, *scrolled, *grid, *button;

  entries = dk_folder_list (folder, &error);
  if (entries == NULL)
    {
      g_autofree char *name = folder_fallback_name (folder);
      g_autofree char *primary = g_strdup_printf ("Can't read “%s”", name);

      g_warning ("can't read %s: %s", uri, error->message);
      report_error (dock, primary, error->message);
      return;
    }
  popup_close (dock);
  dock->popup = gtk_popover_new (dk_item_view_get_widget (view));
  dock->popup_uri = g_strdup (uri);
  gtk_popover_set_position (GTK_POPOVER (dock->popup), popup_position (dock));
  /* The panel is no larger than the dock: the popup is a window of its own. */
  gtk_popover_set_constrain_to (GTK_POPOVER (dock->popup), GTK_POPOVER_CONSTRAINT_NONE);
  g_signal_connect (dock->popup, "closed", G_CALLBACK (on_popup_closed), dock);
  g_signal_connect (dock->popup, "destroy", G_CALLBACK (on_popup_destroy), dock);
  box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  gtk_container_set_border_width (GTK_CONTAINER (box), 8);
  if (entries->len > 0)
    {
      grid = gtk_flow_box_new ();
      gtk_flow_box_set_selection_mode (GTK_FLOW_BOX (grid), GTK_SELECTION_NONE);
      gtk_flow_box_set_activate_on_single_click (GTK_FLOW_BOX (grid), TRUE);
      gtk_flow_box_set_homogeneous (GTK_FLOW_BOX (grid), TRUE);
      gtk_flow_box_set_min_children_per_line (GTK_FLOW_BOX (grid), 1);
      gtk_flow_box_set_max_children_per_line (GTK_FLOW_BOX (grid),
                                              MIN (entries->len, FOLDER_GRID_COLUMNS));
      for (guint i = 0; i < entries->len; i++)
        gtk_container_add (GTK_CONTAINER (grid), popup_cell (entries->pdata[i]));
      g_signal_connect (grid, "child-activated", G_CALLBACK (on_popup_entry), dock);
      scrolled = gtk_scrolled_window_new (NULL, NULL);
      gtk_scrolled_window_set_policy (GTK_SCROLLED_WINDOW (scrolled), GTK_POLICY_NEVER,
                                      GTK_POLICY_AUTOMATIC);
      gtk_scrolled_window_set_propagate_natural_width (GTK_SCROLLED_WINDOW (scrolled), TRUE);
      gtk_scrolled_window_set_propagate_natural_height (GTK_SCROLLED_WINDOW (scrolled), TRUE);
      gtk_scrolled_window_set_max_content_height (GTK_SCROLLED_WINDOW (scrolled),
                                                  FOLDER_GRID_MAX_HEIGHT);
      gtk_container_add (GTK_CONTAINER (scrolled), grid);
      gtk_box_pack_start (GTK_BOX (box), scrolled, TRUE, TRUE, 0);
    }
  else
    {
      GtkWidget *empty = gtk_label_new ("This folder is empty.");

      gtk_widget_set_margin_top (empty, 12);
      gtk_widget_set_margin_bottom (empty, 12);
      gtk_box_pack_start (GTK_BOX (box), empty, FALSE, FALSE, 0);
    }
  button = gtk_button_new_with_label ("Open in Caja");
  g_signal_connect (button, "clicked", G_CALLBACK (on_popup_open_in_caja), dock);
  gtk_box_pack_end (GTK_BOX (box), button, FALSE, FALSE, 0);
  gtk_container_add (GTK_CONTAINER (dock->popup), box);
  gtk_widget_show_all (box);
  gtk_popover_popup (GTK_POPOVER (dock->popup));
}

/* A click: the grid of the folder's contents, or that the folder isn't available. */
void
dk_dock_open_folder (DkDock *dock, const char *uri)
{
  DkItemView *view = folder_view (dock, uri);

  if (view == NULL)
    return;
  if (dk_dock_folder_is_gone (dock, uri))
    folder_not_available (dock, uri);
  else if (dock->popup != NULL && g_strcmp0 (dock->popup_uri, uri) == 0)
    popup_close (dock);
  else
    popup_open (dock, view);
}

GtkWidget *
dk_dock_get_popup (DkDock *dock)
{
  return dock->popup;
}

/* The popup's cells as "name" and whether each is a folder, for the probe. */
GPtrArray *
dk_dock_get_popup_entries (DkDock *dock)
{
  GPtrArray *entries = g_ptr_array_new_with_free_func (g_free);
  GtkWidget *scrolled = NULL, *grid = NULL;
  g_autoptr (GList) children = NULL;

  if (dock->popup == NULL)
    return entries;
  {
    g_autoptr (GList) box = gtk_container_get_children (GTK_CONTAINER (gtk_bin_get_child (GTK_BIN (dock->popup))));

    for (GList *l = box; l != NULL; l = l->next)
      if (GTK_IS_SCROLLED_WINDOW (l->data))
        scrolled = l->data;
  }
  if (scrolled != NULL)
    grid = gtk_bin_get_child (GTK_BIN (gtk_bin_get_child (GTK_BIN (scrolled))));
  if (grid != NULL)
    children = gtk_container_get_children (GTK_CONTAINER (grid));
  for (GList *l = children; l != NULL; l = l->next)
    {
      GtkWidget *cell = gtk_bin_get_child (GTK_BIN (l->data));

      g_ptr_array_add (entries, g_strdup_printf ("%s%s", gtk_widget_get_tooltip_text (cell),
                                                 GPOINTER_TO_INT (g_object_get_data (G_OBJECT (cell), "is-folder")) ? "/" : ""));
    }
  return entries;
}

/* Its menu. */

static void
on_folder_menu_open (GtkMenuItem *menu_item, gpointer data)
{
  open_in_caja (data, g_object_get_data (G_OBJECT (menu_item), "uri"));
}

static void
on_folder_menu_remove (GtkMenuItem *menu_item, gpointer data)
{
  DkDock *dock = data;

  dk_store_remove_folder (dock->store, g_object_get_data (G_OBJECT (menu_item), "uri"));
}

static void
popup_folder_menu (DkDock *dock, DkItemView *view, const GdkEvent *event)
{
  const char *uri = dk_item_view_get_key (view);
  GtkWidget *menu = menu_begin (dock);
  GtkWidget *open = menu_item (menu, "Open in Caja", G_CALLBACK (on_folder_menu_open), dock);
  GtkWidget *remove = menu_item (menu, "Remove from Dock", G_CALLBACK (on_folder_menu_remove),
                                 dock);

  g_object_set_data_full (G_OBJECT (open), "uri", g_strdup (uri), g_free);
  g_object_set_data_full (G_OBJECT (remove), "uri", g_strdup (uri), g_free);
  /* Caja can't open what isn't there. */
  gtk_widget_set_sensitive (open, !dk_dock_folder_is_gone (dock, uri));
  menu_show (dock, menu, dk_item_view_get_widget (view), event);
}

/* A right-click on a Kept folder or a Removable drive item pops up its menu. */
static gboolean
on_after_button_press (GtkWidget *widget, GdkEventButton *event, gpointer data)
{
  if (event->button != 3)
    return GDK_EVENT_PROPAGATE;
  dk_dock_popup_menu (data, widget, (GdkEvent *) event);
  return GDK_EVENT_STOP;
}

static gboolean
on_folder_button_release (GtkWidget *widget, GdkEventButton *event, gpointer data)
{
  /* Released away from the item: a drag, or a click abandoned. */
  if (event->button != 1 || event->x < 0 || event->y < 0
      || event->x >= gtk_widget_get_allocated_width (widget)
      || event->y >= gtk_widget_get_allocated_height (widget))
    return GDK_EVENT_PROPAGATE;
  dk_dock_open_folder (data, dk_item_view_get_key (view_of (widget)));
  return GDK_EVENT_STOP;
}

/* Files dropped on it. */

static void
on_folder_put_failed (const char *uri, GError *error, gpointer data)
{
  g_autoptr (GFile) file = g_file_new_for_uri (uri);
  g_autofree char *name = g_file_get_basename (file);
  g_autofree char *primary = g_strdup_printf ("Can't put “%s” in the folder", name);

  g_warning ("can't put %s in a Kept folder: %s", uri, error->message);
  report_error (data, primary, error->message);
}

void
dk_dock_drop_on_folder (DkDock *dock, const char *folder_uri, const char *const *uris)
{
  g_autoptr (GFile) folder = g_file_new_for_uri (folder_uri);

  if (!dk_folder_exists (folder))
    {
      folder_not_available (dock, folder_uri);
      return;
    }
  dk_folder_put (folder, uris, dock->cancellable, on_folder_put_failed, NULL, dock, NULL);
}

/* The files are put in the folder here, so the source has nothing to delete, whichever
   action it offers. */
static gboolean
on_folder_drag_motion (GtkWidget *widget, GdkDragContext *context, int x, int y, guint time,
                       gpointer data)
{
  GdkDragAction offered = gdk_drag_context_get_actions (context);
  GdkAtom target = gtk_drag_dest_find_target (widget, context, NULL);
  GdkDragAction action = target == GDK_NONE ? 0
                         : offered & GDK_ACTION_MOVE ? GDK_ACTION_MOVE
                         : offered & GDK_ACTION_COPY ? GDK_ACTION_COPY : 0;

  /* At either end a folder is kept next to it, which the items' box decides. */
  if (!over_item (data, widget, x, y, TRUE))
    return FALSE;
  gdk_drag_status (context, action, time);
  return action != 0;
}

static gboolean
on_folder_drag_drop (GtkWidget *widget, GdkDragContext *context, int x, int y, guint time,
                     gpointer data)
{
  GdkAtom target = gtk_drag_dest_find_target (widget, context, NULL);

  if (target == GDK_NONE || !over_item (data, widget, x, y, TRUE))
    return FALSE;
  gtk_drag_get_data (widget, context, target, time);
  return TRUE;
}

static void
on_folder_drag_data_received (GtkWidget *widget, GdkDragContext *context, int x G_GNUC_UNUSED,
                              int y G_GNUC_UNUSED, GtkSelectionData *selection,
                              guint info G_GNUC_UNUSED, guint time, gpointer data)
{
  g_auto (GStrv) uris = gtk_selection_data_get_uris (selection);

  if (uris != NULL)
    dk_dock_drop_on_folder (data, dk_item_view_get_key (view_of (widget)),
                            (const char *const *) uris);
  gtk_drag_finish (context, uris != NULL, FALSE, time);
}

/* Keeping and reordering. */

void
dk_dock_keep_folders (DkDock *dock, const char *const *uris, guint position)
{
  for (guint i = 0; uris != NULL && uris[i] != NULL; i++)
    if (dk_folder_uri_is_folder (uris[i]) && !dk_store_is_folder_kept (dock->store, uris[i]))
      dk_store_move_folder (dock->store, uris[i], position++);
}

void
dk_dock_move_folder (DkDock *dock, const char *uri, guint position)
{
  if (dk_store_is_folder_kept (dock->store, uri))
    dk_store_move_folder (dock->store, uri, position);
}

/* The number of Kept folders before the point (along the dock, in the box of the items after
   the separator), and so where a drop goes: never past the last Kept folder. */
static int
after_slot_at (DkDock *dock, int x, int y)
{
  g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (dock->after));
  int slot = 0;

  for (GList *l = children; l != NULL && is_folder_widget (dock, l->data); l = l->next, slot++)
    {
      GtkAllocation a;

      gtk_widget_get_allocation (l->data, &a);
      if (is_horizontal (dock) ? x < a.x + a.width / 2 : y < a.y + a.height / 2)
        return slot;
    }
  return slot;
}

static gboolean
on_after_draw_after (GtkWidget *after, cairo_t *cr, gpointer data)
{
  DkDock *dock = data;
  g_autoptr (GList) children = NULL;
  GtkStyleContext *context = gtk_widget_get_style_context (after);
  GdkRGBA color;
  GtkAllocation a;
  GList *l;
  double at;

  if (dock->after_slot < 0)
    return FALSE;
  children = gtk_container_get_children (GTK_CONTAINER (dock->after));
  l = g_list_nth (children, dock->after_slot);
  if (l == NULL)
    return FALSE;
  gtk_widget_get_allocation (l->data, &a);
  at = is_horizontal (dock) ? a.x : a.y;
  if (!gtk_style_context_lookup_color (context, "theme_selected_bg_color", &color))
    gdk_rgba_parse (&color, "#3daee9");
  gdk_cairo_set_source_rgba (cr, &color);
  if (is_horizontal (dock))
    cairo_rectangle (cr, at - 1, 0, 2, gtk_widget_get_allocated_height (after));
  else
    cairo_rectangle (cr, 0, at - 1, gtk_widget_get_allocated_width (after), 2);
  cairo_fill (cr);
  return FALSE;
}

static void
on_after_drag_leave (GtkWidget *after, GdkDragContext *context G_GNUC_UNUSED,
                     guint time G_GNUC_UNUSED, gpointer data)
{
  DkDock *dock = data;

  dock->after_slot = -1;
  gtk_widget_queue_draw (after);
}

/* A folder from Caja is kept, a Kept folder is moved: both leave the source nothing to do. */
static gboolean
on_after_drag_motion (GtkWidget *after, GdkDragContext *context, int x, int y, guint time,
                      gpointer data)
{
  DkDock *dock = data;
  GdkAtom target = gtk_drag_dest_find_target (after, context, NULL);
  GdkDragAction offered = gdk_drag_context_get_actions (context);
  GdkDragAction action = target == GDK_NONE ? 0
                         : offered & GDK_ACTION_MOVE ? GDK_ACTION_MOVE
                         : offered & GDK_ACTION_COPY ? GDK_ACTION_COPY
                         : offered & GDK_ACTION_LINK ? GDK_ACTION_LINK : 0;

  gdk_drag_status (context, action, time);
  if (action == 0)
    {
      on_after_drag_leave (after, context, time, dock);
      return FALSE;
    }
  dock->after_slot = after_slot_at (dock, x, y);
  gtk_widget_queue_draw (after);
  return TRUE;
}

static gboolean
on_after_drag_drop (GtkWidget *after, GdkDragContext *context, int x G_GNUC_UNUSED,
                    int y G_GNUC_UNUSED, guint time, gpointer data G_GNUC_UNUSED)
{
  GdkAtom target = gtk_drag_dest_find_target (after, context, NULL);

  if (target == GDK_NONE)
    return FALSE;
  gtk_drag_get_data (after, context, target, time);
  return TRUE;
}

static void
on_after_drag_data_received (GtkWidget *after, GdkDragContext *context, int x, int y,
                             GtkSelectionData *selection, guint info G_GNUC_UNUSED, guint time,
                             gpointer data)
{
  DkDock *dock = data;
  g_auto (GStrv) uris = gtk_selection_data_get_uris (selection);
  gboolean done = FALSE;
  int slot = after_slot_at (dock, x, y);

  dock->after_slot = -1;
  gtk_widget_queue_draw (after);
  if (gtk_selection_data_get_data_type (selection)
      == gdk_atom_intern_static_string (DK_FOLDER_DRAG_TARGET))
    {
      g_autofree char *uri = g_strndup ((const char *) gtk_selection_data_get_data (selection),
                                        gtk_selection_data_get_length (selection));

      dk_dock_move_folder (dock, uri, slot);
      done = TRUE;
    }
  else if (uris != NULL)
    {
      /* Only folders are kept: a file dropped here, between the items, is refused. */
      for (guint i = 0; uris[i] != NULL; i++)
        done = done || dk_folder_uri_is_folder (uris[i]);
      dk_dock_keep_folders (dock, (const char *const *) uris, slot);
    }
  gtk_drag_finish (context, done, FALSE, time);
}

/* The Removable drive items (CMN-240). */

static void
on_drives_changed (gpointer data)
{
  schedule_refresh (data);
}

static gboolean
on_drive_button_release (GtkWidget *widget, GdkEventButton *event, gpointer data)
{
  /* Released away from the item: a click abandoned. */
  if (event->button != 1 || event->x < 0 || event->y < 0
      || event->x >= gtk_widget_get_allocated_width (widget)
      || event->y >= gtk_widget_get_allocated_height (widget))
    return GDK_EVENT_PROPAGATE;
  open_in_caja (data, dk_item_view_get_key (view_of (widget)));
  return GDK_EVENT_STOP;
}

static void
on_drive_menu_open (GtkMenuItem *menu_item, gpointer data)
{
  open_in_caja (data, g_object_get_data (G_OBJECT (menu_item), "uri"));
}

/* What an eject or an unmount that fails reports. */
typedef struct
{
  DkDock   *dock;
  char     *uri;
  gboolean  eject;
  /* What the error says first: "Can't eject “USB”". */
  char     *primary;
} RemoveReport;

static void
remove_report_free (RemoveReport *report)
{
  g_free (report->uri);
  g_free (report->primary);
  g_free (report);
}

static void
on_drive_removed (GError *error, gpointer data)
{
  RemoveReport *report = data;

  if (error != NULL)
    {
      g_warning ("can't %s %s: %s", report->eject ? "eject" : "unmount", report->uri,
                 error->message);
      report_error (report->dock, report->primary, error->message);
    }
}

/* Eject, or Unmount, as the menu said. The item goes when the monitor tells the mount has. */
static void
on_drive_menu_remove (GtkMenuItem *menu_item, gpointer data)
{
  DkDock *dock = data;
  const char *uri = g_object_get_data (G_OBJECT (menu_item), "uri");
  GMount *mount = dk_drives_lookup (dock->drives, uri);
  g_autofree char *name = NULL;
  RemoveReport *report;

  /* Gone while the menu was open. */
  if (mount == NULL)
    return;
  name = g_mount_get_name (mount);
  report = g_new0 (RemoveReport, 1);
  report->dock = dock;
  report->uri = g_strdup (uri);
  report->eject = g_mount_can_eject (mount);
  report->primary = g_strdup_printf (report->eject ? "Can't eject “%s”" : "Can't unmount “%s”",
                                     name);
  dk_drive_remove (mount, dock->cancellable, on_drive_removed, report,
                   (GDestroyNotify) remove_report_free);
}

static void
popup_drive_menu (DkDock *dock, DkItemView *view, const GdkEvent *event)
{
  const char *uri = dk_item_view_get_key (view);
  GMount *mount = dk_drives_lookup (dock->drives, uri);
  GtkWidget *menu = menu_begin (dock);
  GtkWidget *open = menu_item (menu, "Open", G_CALLBACK (on_drive_menu_open), dock);
  GtkWidget *remove = menu_item (menu, mount != NULL && g_mount_can_eject (mount) ? "Eject"
                                                                                  : "Unmount",
                                 G_CALLBACK (on_drive_menu_remove), dock);

  g_object_set_data_full (G_OBJECT (open), "uri", g_strdup (uri), g_free);
  g_object_set_data_full (G_OBJECT (remove), "uri", g_strdup (uri), g_free);
  menu_show (dock, menu, dk_item_view_get_widget (view), event);
}

/* Files dropped on it are copied there. */

static void
on_drive_put_failed (const char *uri, GError *error, gpointer data)
{
  g_autoptr (GFile) file = g_file_new_for_uri (uri);
  g_autofree char *name = g_file_get_basename (file);
  g_autofree char *primary = g_strdup_printf ("Can't copy “%s” to the disk", name);

  g_warning ("can't copy %s to a removable drive: %s", uri, error->message);
  report_error (data, primary, error->message);
}

static void
drop_on_drive (DkDock *dock, const char *key, const char *const *uris)
{
  GMount *mount = dk_drives_lookup (dock->drives, key);

  if (mount != NULL)
    dk_drive_put (mount, uris, dock->cancellable, on_drive_put_failed, NULL, dock, NULL);
}

/* Files dropped on it are copied there: a copy where the source offers one. */
static gboolean
on_drive_drag_motion (GtkWidget *widget, GdkDragContext *context, int x, int y, guint time,
                      gpointer data)
{
  return files_drag_motion (data, widget, context, x, y, time, GDK_ACTION_COPY);
}

static void
on_drive_drag_data_received (GtkWidget *widget, GdkDragContext *context, int x G_GNUC_UNUSED,
                             int y G_GNUC_UNUSED, GtkSelectionData *selection,
                             guint info G_GNUC_UNUSED, guint time, gpointer data)
{
  g_auto (GStrv) uris = gtk_selection_data_get_uris (selection);

  if (uris != NULL)
    drop_on_drive (data, dk_item_view_get_key (view_of (widget)), (const char *const *) uris);
  gtk_drag_finish (context, uris != NULL, FALSE, time);
}

static DkItemView *
drive_view_new (DkDock *dock, const char *key)
{
  static const GtkTargetEntry targets[] = {
    { (char *) "text/uri-list", 0, 0 },
  };
  DkItemView *view = dk_item_view_new ();
  GtkWidget *widget = dk_item_view_get_widget (view);

  g_hash_table_insert (dock->drive_views, g_strdup (key), view);
  gtk_box_pack_start (GTK_BOX (dock->after), widget, FALSE, FALSE, 0);
  gtk_widget_show (widget);
  g_signal_connect (widget, "button-press-event", G_CALLBACK (on_after_button_press), dock);
  g_signal_connect (widget, "button-release-event", G_CALLBACK (on_drive_button_release), dock);
  gtk_drag_dest_set (widget, 0, targets, G_N_ELEMENTS (targets),
                     GDK_ACTION_MOVE | GDK_ACTION_COPY);
  g_signal_connect (widget, "drag-motion", G_CALLBACK (on_drive_drag_motion), dock);
  g_signal_connect (widget, "drag-drop", G_CALLBACK (on_files_drag_drop), dock);
  g_signal_connect (widget, "drag-data-received", G_CALLBACK (on_drive_drag_data_received),
                    dock);
  return view;
}

/* The drive's item shows its mount's icon and name, at the slot among the items after the
   separator. It can't be kept or dragged. */
static void
drive_view_update (DkDock *dock, const char *key, int slot)
{
  GMount *mount = dk_drives_lookup (dock->drives, key);
  DkItemView *view = g_hash_table_lookup (dock->drive_views, key);
  g_autoptr (GIcon) icon = g_mount_get_icon (mount);
  g_autofree char *name = g_mount_get_name (mount);
  g_autoptr (GPtrArray) none = g_ptr_array_new ();

  if (view == NULL)
    view = drive_view_new (dock, key);
  dk_item_view_set_geometry (view, dock->thickness, dock->edge);
  dk_item_view_set (view, key, icon, name, none, FALSE, FALSE);
  gtk_box_reorder_child (GTK_BOX (dock->after), dk_item_view_get_widget (view), slot);
}

/* Makes the widgets of the Kept folders match the store: the icon, the name, and whether the
   folder is there. */

static void
on_folder_changed (GFileMonitor *monitor G_GNUC_UNUSED, GFile *file G_GNUC_UNUSED,
                   GFile *other G_GNUC_UNUSED, GFileMonitorEvent event G_GNUC_UNUSED,
                   gpointer data)
{
  schedule_refresh (data);
}

static gboolean
on_gone_poll (gpointer data)
{
  DkDock *dock = data;

  schedule_refresh (dock);
  return G_SOURCE_CONTINUE;
}

static DkItemView *
folder_view_new (DkDock *dock, const char *uri)
{
  static const GtkTargetEntry targets[] = {
    { (char *) "text/uri-list", 0, 0 },
  };
  DkItemView *view = dk_item_view_new ();
  GtkWidget *widget = dk_item_view_get_widget (view);

  g_hash_table_insert (dock->folder_views, g_strdup (uri), view);
  gtk_box_pack_start (GTK_BOX (dock->after), widget, FALSE, FALSE, 0);
  gtk_widget_show (widget);
  g_signal_connect (widget, "button-press-event", G_CALLBACK (on_after_button_press), dock);
  g_signal_connect (widget, "button-release-event", G_CALLBACK (on_folder_button_release), dock);
  gtk_drag_dest_set (widget, 0, targets, G_N_ELEMENTS (targets),
                     GDK_ACTION_MOVE | GDK_ACTION_COPY);
  g_signal_connect (widget, "drag-motion", G_CALLBACK (on_folder_drag_motion), dock);
  g_signal_connect (widget, "drag-drop", G_CALLBACK (on_folder_drag_drop), dock);
  g_signal_connect (widget, "drag-data-received", G_CALLBACK (on_folder_drag_data_received),
                    dock);
  return view;
}

static void
refresh_after (DkDock *dock)
{
  g_autoptr (GArray) items = dk_after_build (dk_store_get_folders (dock->store),
                                             dk_drives_get_keys (dock->drives));
  g_autoptr (GHashTable) wanted = g_hash_table_new (g_str_hash, g_str_equal);
  g_autoptr (GHashTable) wanted_drives = g_hash_table_new (g_str_hash, g_str_equal);
  g_autoptr (GPtrArray) none = g_ptr_array_new ();
  GHashTableIter iter;
  gpointer key, value;
  gboolean any_gone = FALSE;
  int slot = 0;

  for (guint i = 0; i < items->len; i++)
    {
      const DkAfterItem *item = &g_array_index (items, DkAfterItem, i);
      g_autoptr (GFile) folder = NULL;
      g_autoptr (GIcon) icon = NULL;
      g_autofree char *name = NULL;
      DkItemView *view;
      gboolean gone;

      if (item->kind == DK_AFTER_REMOVABLE_DRIVE)
        {
          drive_view_update (dock, item->key, slot++);
          g_hash_table_add (wanted_drives, item->key);
          continue;
        }
      if (item->kind != DK_AFTER_KEPT_FOLDER)
        continue;
      view = folder_view (dock, item->key);
      if (view == NULL)
        view = folder_view_new (dock, item->key);
      folder = g_file_new_for_uri (item->key);
      gone = !dk_folder_exists (folder);
      any_gone = any_gone || gone;
      if (!gone)
        {
          icon = dk_folder_icon (folder);
          name = dk_folder_display_name (folder);
        }
      if (icon == NULL)
        icon = g_themed_icon_new ("folder");
      if (name == NULL)
        name = folder_fallback_name (folder);
      dk_item_view_set_geometry (view, dock->thickness, dock->edge);
      dk_item_view_set (view, item->key, icon, name, none, FALSE, FALSE);
      dk_item_view_set_drag (view, DK_FOLDER_DRAG_TARGET);
      dk_item_view_set_dimmed (view, gone);
      gtk_box_reorder_child (GTK_BOX (dock->after), dk_item_view_get_widget (view), slot++);
      g_hash_table_add (wanted, item->key);
      /* A folder that is there is watched for its deletion and a new icon; one that is gone
         is looked for again. */
      if (gone)
        g_hash_table_remove (dock->folder_monitors, item->key);
      else if (!g_hash_table_contains (dock->folder_monitors, item->key))
        {
          GFileMonitor *monitor = g_file_monitor_file (folder, G_FILE_MONITOR_WATCH_MOVES, NULL,
                                                       NULL);

          if (monitor != NULL)
            {
              g_signal_connect (monitor, "changed", G_CALLBACK (on_folder_changed), dock);
              g_hash_table_insert (dock->folder_monitors, g_strdup (item->key), monitor);
            }
        }
    }
  g_hash_table_iter_init (&iter, dock->folder_views);
  while (g_hash_table_iter_next (&iter, &key, &value))
    if (!g_hash_table_contains (wanted, key))
      {
        if (g_strcmp0 (dock->popup_uri, key) == 0)
          popup_close (dock);
        g_hash_table_remove (dock->folder_monitors, key);
        gtk_widget_destroy (dk_item_view_get_widget (value));
        g_hash_table_iter_remove (&iter);
      }
  g_hash_table_iter_init (&iter, dock->drive_views);
  while (g_hash_table_iter_next (&iter, &key, &value))
    if (!g_hash_table_contains (wanted_drives, key))
      {
        gtk_widget_destroy (dk_item_view_get_widget (value));
        g_hash_table_iter_remove (&iter);
      }
  if (any_gone && dock->gone_id == 0)
    dock->gone_id = g_timeout_add_seconds (GONE_POLL_SECONDS, on_gone_poll, dock);
  else if (!any_gone)
    g_clear_handle_id (&dock->gone_id, g_source_remove);
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

void
dk_dock_set_launch_timeout (DkDock *dock, gint64 timeout)
{
  dk_feedback_set_launch_timeout (dock->feedback, timeout);
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
  gtk_orientable_set_orientation (GTK_ORIENTABLE (dock->after),
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
  dock->feedback = dk_feedback_new ();
  dock->launcher = dk_launcher_new ();
  dock->thickness = 48;
  dock->edge = GTK_POS_BOTTOM;
  dock->drop_slot = -1;
  dock->after_slot = -1;
  dock->cancellable = g_cancellable_new ();
  dock->folder_views = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  dock->folder_monitors = g_hash_table_new_full (g_str_hash, g_str_equal, g_free,
                                                 g_object_unref);
  dock->drive_views = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  {
    g_autoptr (GVolumeMonitor) monitor = g_volume_monitor_get ();

    dock->drives = dk_drives_new (monitor);
    dk_drives_set_changed (dock->drives, on_drives_changed, dock);
  }
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
  dock->after = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 0);
  gtk_box_pack_start (GTK_BOX (dock->after), dk_item_view_get_widget (dock->trash_view), FALSE,
                      FALSE, 0);
  gtk_box_pack_start (GTK_BOX (dock->widget), dock->box, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (dock->widget), dock->separator, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (dock->widget), dock->after, FALSE, FALSE, 0);
  gtk_widget_show_all (dock->widget);
  {
    /* Folders from Caja are kept, and Kept folders reordered, in the items after the
       separator: the part of an item that isn't over it, and the space between. */
    static const GtkTargetEntry after_targets[] = {
      { (char *) DK_FOLDER_DRAG_TARGET, GTK_TARGET_SAME_APP, 0 },
      { (char *) "text/uri-list", 0, 1 },
    };

    g_signal_connect_after (dock->after, "draw", G_CALLBACK (on_after_draw_after), dock);
    gtk_drag_dest_set (dock->after, 0, after_targets, G_N_ELEMENTS (after_targets),
                       GDK_ACTION_MOVE | GDK_ACTION_COPY | GDK_ACTION_LINK);
    g_signal_connect (dock->after, "drag-motion", G_CALLBACK (on_after_drag_motion), dock);
    g_signal_connect (dock->after, "drag-leave", G_CALLBACK (on_after_drag_leave), dock);
    g_signal_connect (dock->after, "drag-drop", G_CALLBACK (on_after_drag_drop), dock);
    g_signal_connect (dock->after, "drag-data-received",
                      G_CALLBACK (on_after_drag_data_received), dock);
  }
  g_object_set_data_full (G_OBJECT (dock->box), "dk-dock", dock, NULL);
  g_signal_connect_after (dock->box, "draw", G_CALLBACK (on_box_draw_after), dock);
  gtk_drag_dest_set (dock->box, 0, targets, G_N_ELEMENTS (targets),
                     GDK_ACTION_MOVE | GDK_ACTION_COPY);
  g_signal_connect (dock->box, "drag-motion", G_CALLBACK (on_drag_motion), dock);
  g_signal_connect (dock->box, "drag-leave", G_CALLBACK (on_drag_leave), dock);
  g_signal_connect (dock->box, "drag-drop", G_CALLBACK (on_drag_drop), dock);
  g_signal_connect (dock->box, "drag-data-received", G_CALLBACK (on_drag_data_received), dock);
  dk_store_set_changed (store, on_store_changed, dock);
  {
    g_autoptr (GError) error = NULL;

    dock->bus = g_bus_get_sync (G_BUS_TYPE_SESSION, NULL, &error);
    if (dock->bus == NULL)
      g_warning ("can't reach the session bus, so no badges or progress bars: %s",
                 error->message);
    else
      {
        dock->update_subscription = g_dbus_connection_signal_subscribe (
          dock->bus, NULL, LAUNCHER_ENTRY_INTERFACE, "Update", NULL, NULL,
          G_DBUS_SIGNAL_FLAGS_NONE, on_launcher_update, dock, NULL);
        dock->owner_subscription = g_dbus_connection_signal_subscribe (
          dock->bus, "org.freedesktop.DBus", "org.freedesktop.DBus", "NameOwnerChanged",
          "/org/freedesktop/DBus", NULL, G_DBUS_SIGNAL_FLAGS_NONE, on_name_owner_changed, dock,
          NULL);
      }
  }
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
  g_clear_handle_id (&dock->feedback_id, g_source_remove);
  if (dock->bus != NULL)
    {
      g_dbus_connection_signal_unsubscribe (dock->bus, dock->update_subscription);
      g_dbus_connection_signal_unsubscribe (dock->bus, dock->owner_subscription);
      g_object_unref (dock->bus);
    }
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
  if (dock->error_dialog != NULL)
    gtk_widget_destroy (dock->error_dialog);
  /* The files being put in a folder are let go of: nothing is told once the dock is. */
  g_cancellable_cancel (dock->cancellable);
  g_object_unref (dock->cancellable);
  g_clear_handle_id (&dock->gone_id, g_source_remove);
  if (dock->popup != NULL)
    {
      g_signal_handlers_disconnect_by_data (dock->popup, dock);
      gtk_widget_destroy (dock->popup);
    }
  g_free (dock->popup_uri);
  g_hash_table_unref (dock->folder_monitors);
  g_hash_table_unref (dock->folder_views);
  dk_drives_free (dock->drives);
  g_hash_table_unref (dock->drive_views);
  dk_trash_free (dock->trash);
  g_clear_object (&dock->screen);
  g_object_unref (dock->monitor);
  g_hash_table_unref (dock->windows);
  g_hash_table_unref (dock->views);
  g_hash_table_unref (dock->infos);
  g_clear_pointer (&dock->pressed, g_array_unref);
  dk_catalog_free (dock->catalog);
  dk_feedback_free (dock->feedback);
  dk_launcher_free (dock->launcher);
  g_free (dock->ignored);
  g_free (dock);
}
