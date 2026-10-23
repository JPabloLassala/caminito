/*
 * The Network applet (CMN-193, ADR 0041): an in-process mate-panel applet whose icon shows
 * NetworkManager's primary connection, wired or Wi-Fi by its signal, coming up, with no
 * internet behind it, or nothing up, airplane mode and NetworkManager not running, with a
 * lock over it while a VPN is up. It reads NetworkManager through libnm (nw-nm.c).
 * Left-click toggles the popup (nw-popup.c); its right-click menu has "Configure network
 * connections…", which runs nm-connection-editor.
 */

#include <mate-panel-applet.h>

#include "caminito-applet-icon.h"
#include "caminito-applet-popup.h"
#include "nw-nm.h"
#include "nw-popup.h"
#include "nw-state.h"

#define FACTORY_ID "CaminitoNetworkAppletFactory"
#define APPLET_ID  "CaminitoNetworkApplet"

typedef struct
{
  MatePanelApplet     *applet;
  GtkWidget           *icon;
  GtkWidget           *lock;
  NwNm                *nm;
  CaminitoAppletPopup *popup;
  NwPopup             *content;
} Applet;

static const char menu_xml[] = "<menuitem name='Configure' action='Configure'/>";

static void
update (Applet *applet)
{
  const NwStatus *status = nw_nm_status (applet->nm);
  g_autofree char *tooltip = nw_state_tooltip (status);

  caminito_applet_icon_set (applet->icon, nw_state_icon_names (status));
  gtk_widget_set_visible (applet->lock, status->running && status->vpn != NULL);
  gtk_widget_set_tooltip_text (GTK_WIDGET (applet->applet), tooltip);
  nw_popup_set_running (applet->content, status->running);
}

static void
on_changed (NwNm *nm G_GNUC_UNUSED, gpointer data)
{
  update (data);
}

/* The lock is half the icon, in its bottom right-hand corner. */
static void
on_change_size (MatePanelApplet *panel_applet G_GNUC_UNUSED, guint size, gpointer data)
{
  Applet *applet = data;

  gtk_image_set_pixel_size (GTK_IMAGE (applet->lock),
                            caminito_applet_icon_pixel_size (size) / 2);
}

/* A symbolic lock is the icon's colour, which over the icon would hide it: a halo of the
   background's colour around it keeps it apart. */
static void
add_halo (GtkWidget *lock)
{
  g_autoptr (GtkCssProvider) provider = gtk_css_provider_new ();

  gtk_css_provider_load_from_data (provider,
                                   "image { -gtk-icon-shadow:"
                                   " 1px 0 @theme_bg_color, -1px 0 @theme_bg_color,"
                                   " 0 1px @theme_bg_color, 0 -1px @theme_bg_color; }",
                                   -1, NULL);
  gtk_style_context_add_provider (gtk_widget_get_style_context (lock),
                                  GTK_STYLE_PROVIDER (provider),
                                  GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
}

static void
on_configure (GtkAction *action G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
  g_autoptr (GError) error = NULL;
  const char *argv[] = { "nm-connection-editor", NULL };

  if (!g_spawn_async (NULL, (char **) argv, NULL,
                      G_SPAWN_SEARCH_PATH | G_SPAWN_STDOUT_TO_DEV_NULL, NULL, NULL, NULL,
                      &error))
    g_warning ("can't open nm-connection-editor: %s", error->message);
}

static void
applet_free (gpointer data)
{
  Applet *applet = data;

  /* The popup's window goes with the applet. */
  nw_popup_free (applet->content);
  nw_nm_free (applet->nm);
  g_free (applet);
}

static gboolean
fill (MatePanelApplet *panel_applet)
{
  GtkActionGroup *actions;
  GtkAction *action;
  GtkWidget *overlay;
  Applet *applet = g_new0 (Applet, 1);

  applet->applet = panel_applet;
  mate_panel_applet_set_flags (panel_applet, MATE_PANEL_APPLET_EXPAND_MINOR);
  mate_panel_applet_set_background_widget (panel_applet, GTK_WIDGET (panel_applet));
  overlay = gtk_overlay_new ();
  /* The icon's size, not the panel's, so that the lock is in the icon's corner. */
  gtk_widget_set_halign (overlay, GTK_ALIGN_CENTER);
  gtk_widget_set_valign (overlay, GTK_ALIGN_CENTER);
  applet->icon = caminito_applet_icon_new (panel_applet);
  gtk_container_add (GTK_CONTAINER (overlay), applet->icon);
  applet->lock = gtk_image_new ();
  caminito_applet_icon_set (applet->lock, nw_state_lock_names ());
  gtk_widget_set_halign (applet->lock, GTK_ALIGN_END);
  gtk_widget_set_valign (applet->lock, GTK_ALIGN_END);
  gtk_widget_set_no_show_all (applet->lock, TRUE);
  add_halo (applet->lock);
  gtk_overlay_add_overlay (GTK_OVERLAY (overlay), applet->lock);
  gtk_container_add (GTK_CONTAINER (panel_applet), overlay);
  g_signal_connect (panel_applet, "change-size", G_CALLBACK (on_change_size), applet);
  on_change_size (panel_applet, mate_panel_applet_get_size (panel_applet), applet);

  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  /* mate-panel's applet menu still takes GtkActions. */
  actions = gtk_action_group_new ("Network Applet Actions");
  action = gtk_action_new ("Configure", "_Configure network connections…", NULL, NULL);
  g_signal_connect (action, "activate", G_CALLBACK (on_configure), NULL);
  gtk_action_group_add_action (actions, action);
  g_object_unref (action);
  G_GNUC_END_IGNORE_DEPRECATIONS
  mate_panel_applet_setup_menu (panel_applet, menu_xml, actions);
  g_object_unref (actions);

  g_object_set_data_full (G_OBJECT (panel_applet), "caminito-network-applet", applet,
                          applet_free);
  applet->popup = caminito_applet_popup_new (GTK_WIDGET (panel_applet));
  applet->content = nw_popup_new (applet->popup);
  applet->nm = nw_nm_new (on_changed, applet);
  gtk_widget_show_all (GTK_WIDGET (panel_applet));
  /* NetworkManager not running until libnm answers. */
  update (applet);
  return TRUE;
}

static gboolean
factory (MatePanelApplet *applet, const char *iid, gpointer data G_GNUC_UNUSED)
{
  if (g_strcmp0 (iid, APPLET_ID) != 0)
    return FALSE;
  return fill (applet);
}

MATE_PANEL_APPLET_IN_PROCESS_FACTORY (FACTORY_ID, PANEL_TYPE_APPLET, "Network", factory, NULL)
