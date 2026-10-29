/*
 * The Notifications applet (CMN-260, ADR 0051): an in-process mate-panel applet whose icon is
 * a bell, with a dot on it while the Notification history isn't empty, and whose popup is the
 * Notification center (nt-popup.c), which groups the history by app (CMN-265). It reads the
 * history over caminito-notifications's private API (nt-daemon.c), and hides itself while the
 * daemon isn't on the bus, as under X11 MATE. Left-click toggles the popup. The
 * struck-through bell and middle-click (CMN-263), and the settings gear (CMN-261), come
 * later.
 */

#include <mate-panel-applet.h>

#include "caminito-applet-icon.h"
#include "caminito-applet-popup.h"
#include "nt-daemon.h"
#include "nt-popup.h"
#include "nt-state.h"

#define FACTORY_ID "CaminitoNotificationsAppletFactory"
#define APPLET_ID  "CaminitoNotificationsApplet"

typedef struct
{
  MatePanelApplet     *applet;
  GtkWidget           *icon;
  GtkWidget           *dot;
  NtDaemon            *daemon;
  CaminitoAppletPopup *popup;
  NtPopup             *content;
} Applet;

static void
update (Applet *applet)
{
  const GPtrArray *entries = nt_daemon_entries (applet->daemon);
  g_autofree char *tooltip = NULL;

  nt_popup_update (applet->content, entries, nt_daemon_ungrouped (applet->daemon),
                   g_get_real_time () / G_USEC_PER_SEC);
  /* No daemon, no icon: the applet comes back with it. */
  if (!nt_daemon_running (applet->daemon))
    {
      caminito_applet_popup_hide (applet->popup);
      gtk_widget_hide (GTK_WIDGET (applet->applet));
      return;
    }
  gtk_widget_set_visible (applet->dot, entries->len > 0);
  tooltip = nt_state_tooltip (entries->len);
  gtk_widget_set_tooltip_text (GTK_WIDGET (applet->applet), tooltip);
  gtk_widget_show (GTK_WIDGET (applet->applet));
}

static void
on_changed (NtDaemon *daemon G_GNUC_UNUSED, gpointer data)
{
  update (data);
}

/* The dot is the theme's selection colour, which is what themes use to draw the eye, in a
   ring of the panel's background that keeps it apart from the bell. */
static gboolean
on_draw_dot (GtkWidget *dot, cairo_t *cr, gpointer data G_GNUC_UNUSED)
{
  GtkStyleContext *style = gtk_widget_get_style_context (dot);
  double size = MIN (gtk_widget_get_allocated_width (dot), gtk_widget_get_allocated_height (dot));
  GdkRGBA fill = { 0.88, 0.11, 0.14, 1.0 }, ring = { 0, 0, 0, 0 };

  gtk_style_context_lookup_color (style, "theme_selected_bg_color", &fill);
  gtk_style_context_lookup_color (style, "theme_bg_color", &ring);
  cairo_arc (cr, size / 2, size / 2, size / 2, 0, 2 * G_PI);
  gdk_cairo_set_source_rgba (cr, &ring);
  cairo_fill (cr);
  cairo_arc (cr, size / 2, size / 2, size / 2 - 1, 0, 2 * G_PI);
  gdk_cairo_set_source_rgba (cr, &fill);
  cairo_fill (cr);
  return TRUE;
}

/* The dot is a third of the icon, in its top right-hand corner. */
static void
on_change_size (MatePanelApplet *panel_applet G_GNUC_UNUSED, guint size, gpointer data)
{
  Applet *applet = data;
  int dot = MAX (6, caminito_applet_icon_pixel_size (size) / 3 + 1);

  gtk_widget_set_size_request (applet->dot, dot, dot);
}

static void
invoke (guint32 id, const char *key, gpointer data)
{
  Applet *applet = data;

  nt_daemon_invoke_action (applet->daemon, id, key);
}

static void
dismiss (guint32 id, gpointer data)
{
  Applet *applet = data;

  nt_daemon_dismiss (applet->daemon, id);
}

static void
clear_app (const char *app, gpointer data)
{
  Applet *applet = data;

  nt_daemon_clear_app (applet->daemon, app);
}

static void
clear_all (gpointer data)
{
  Applet *applet = data;

  nt_daemon_clear_all (applet->daemon);
}

static const NtPopupActions popup_actions = {
  .invoke = invoke,
  .dismiss = dismiss,
  .clear_app = clear_app,
  .clear_all = clear_all,
};

static void
applet_free (gpointer data)
{
  Applet *applet = data;

  /* The popup's window goes with the applet. */
  nt_daemon_free (applet->daemon);
  nt_popup_free (applet->content);
  g_free (applet);
}

static gboolean
fill (MatePanelApplet *panel_applet)
{
  GtkWidget *overlay;
  Applet *applet = g_new0 (Applet, 1);

  applet->applet = panel_applet;
  mate_panel_applet_set_flags (panel_applet, MATE_PANEL_APPLET_EXPAND_MINOR);
  mate_panel_applet_set_background_widget (panel_applet, GTK_WIDGET (panel_applet));
  overlay = gtk_overlay_new ();
  /* The icon's size, not the panel's, so that the dot is in the icon's corner. */
  gtk_widget_set_halign (overlay, GTK_ALIGN_CENTER);
  gtk_widget_set_valign (overlay, GTK_ALIGN_CENTER);
  applet->icon = caminito_applet_icon_new (panel_applet);
  caminito_applet_icon_set (applet->icon, nt_state_icon_names ());
  gtk_container_add (GTK_CONTAINER (overlay), applet->icon);
  applet->dot = gtk_drawing_area_new ();
  gtk_widget_set_halign (applet->dot, GTK_ALIGN_END);
  gtk_widget_set_valign (applet->dot, GTK_ALIGN_START);
  gtk_widget_set_no_show_all (applet->dot, TRUE);
  g_signal_connect (applet->dot, "draw", G_CALLBACK (on_draw_dot), NULL);
  gtk_overlay_add_overlay (GTK_OVERLAY (overlay), applet->dot);
  gtk_container_add (GTK_CONTAINER (panel_applet), overlay);
  g_signal_connect (panel_applet, "change-size", G_CALLBACK (on_change_size), applet);
  on_change_size (panel_applet, mate_panel_applet_get_size (panel_applet), applet);

  g_object_set_data_full (G_OBJECT (panel_applet), "caminito-notifications-applet", applet,
                          applet_free);
  applet->popup = caminito_applet_popup_new (GTK_WIDGET (panel_applet));
  applet->content = nt_popup_new (applet->popup, &popup_actions, applet);
  applet->daemon = nt_daemon_new (on_changed, applet);
  /* Hidden until the daemon is found on the bus. */
  gtk_widget_show_all (overlay);
  gtk_widget_hide (GTK_WIDGET (panel_applet));
  return TRUE;
}

static gboolean
factory (MatePanelApplet *applet, const char *iid, gpointer data G_GNUC_UNUSED)
{
  if (g_strcmp0 (iid, APPLET_ID) != 0)
    return FALSE;
  return fill (applet);
}

MATE_PANEL_APPLET_IN_PROCESS_FACTORY (FACTORY_ID, PANEL_TYPE_APPLET, "Notifications", factory,
                                      NULL)
