/*
 * The Bluetooth applet (259, ADR 0035): an in-process mate-panel applet whose icon shows
 * whether Bluetooth is off, on, or connected, with the connected devices and their
 * battery in its tooltip. It reads BlueZ directly (bt-bluez.c), hides itself while there's
 * no adapter, and its right-click menu turns Bluetooth off or on and opens
 * caminito-bluetooth. Left-click opens the popup (260, bt-popup.c), which lists the
 * paired devices and connects them.
 */

#include <mate-panel-applet.h>

#include "bt-bluez.h"
#include "bt-popup.h"
#include "bt-rfkill.h"
#include "bt-state.h"
#include "caminito-applet-icon.h"
#include "caminito-applet-popup.h"

#define FACTORY_ID "CaminitoBluetoothAppletFactory"
#define APPLET_ID  "CaminitoBluetoothApplet"

typedef struct
{
  MatePanelApplet     *applet;
  GtkWidget           *icon;
  GtkAction           *toggle;
  BtBluez             *bluez;
  BtIconState          state;
  CaminitoAppletPopup *popup;
  BtPopup             *content;
} Applet;

static const char menu_xml[] =
  "<menuitem name='Toggle' action='Toggle'/>"
  "<menuitem name='Configure' action='Configure'/>";

static void
update (Applet *applet)
{
  g_autoptr (BtSnapshot) snapshot = bt_bluez_snapshot (applet->bluez);
  g_autofree char *tooltip = bt_state_tooltip (snapshot);

  applet->state = bt_state_icon (snapshot);
  bt_popup_update (applet->content, snapshot);
  /* No adapter, no icon: the applet hides itself, and comes back when one appears. */
  if (applet->state == BT_ICON_NONE)
    {
      caminito_applet_popup_hide (applet->popup);
      gtk_widget_hide (GTK_WIDGET (applet->applet));
      gtk_widget_set_tooltip_text (GTK_WIDGET (applet->applet), NULL);
      return;
    }
  caminito_applet_icon_set (applet->icon, bt_state_icon_names (applet->state));
  gtk_widget_show (GTK_WIDGET (applet->applet));
  gtk_widget_set_tooltip_text (GTK_WIDGET (applet->applet), tooltip);
  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  gtk_action_set_label (applet->toggle, applet->state == BT_ICON_OFF ? "Turn Bluetooth _on"
                                                                     : "Turn Bluetooth _off");
  G_GNUC_END_IGNORE_DEPRECATIONS
}

static void
on_changed (BtBluez *bluez G_GNUC_UNUSED, gpointer data)
{
  update (data);
}

static void
set_bluetooth (Applet *applet, gboolean on)
{
  g_autoptr (GError) error = NULL;

  /* Off is the rfkill block, which lasts across reboots, and bluetoothd powers the
     adapters off when it sees it. On lifts it and powers them. Where the device can't be
     written, power alone is the next best thing, till the next boot. */
  if (!bt_rfkill_set_blocked (BT_RFKILL_DEVICE, !on, &error))
    g_warning ("can't turn Bluetooth %s through rfkill, only power: %s", on ? "on" : "off",
               error->message);
  else if (!on)
    return;
  bt_bluez_set_powered (applet->bluez, on);
}

static void
on_toggle (GtkAction *action G_GNUC_UNUSED, gpointer data)
{
  Applet *applet = data;

  set_bluetooth (applet, applet->state == BT_ICON_OFF);
}

static void
open_bluetooth (const char *flag)
{
  g_autoptr (GError) error = NULL;
  const char *argv[] = { "caminito-bluetooth", flag, NULL };

  if (!g_spawn_async (NULL, (char **) argv, NULL,
                      G_SPAWN_SEARCH_PATH | G_SPAWN_STDOUT_TO_DEV_NULL, NULL, NULL, NULL,
                      &error))
    g_warning ("can't open caminito-bluetooth: %s", error->message);
}

static void
on_configure (GtkAction *action G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
  open_bluetooth (NULL);
}

/* What the popup's controls do (260). */

static void
on_popup_set_powered (gboolean on, gpointer data)
{
  set_bluetooth (data, on);
}

static void
on_connect_done (const char *path, const char *error, gpointer data)
{
  Applet *applet = data;

  if (error != NULL)
    g_warning ("can't change the connection of %s: %s", path, error);
  bt_popup_connect_done (applet->content, path, error);
}

static void
on_popup_set_connected (const char *path, gboolean connected, gpointer data)
{
  Applet *applet = data;

  bt_bluez_set_connected (applet->bluez, path, connected, on_connect_done, applet);
}

static void
on_popup_add_device (gpointer data G_GNUC_UNUSED)
{
  open_bluetooth ("--scan");
}

static void
on_popup_configure (gpointer data G_GNUC_UNUSED)
{
  open_bluetooth (NULL);
}

static void
applet_free (gpointer data)
{
  Applet *applet = data;

  /* The popup's own handlers can't run any more: its window goes with the applet. */
  bt_popup_free (applet->content);
  bt_bluez_free (applet->bluez);
  /* The panel's menu may hold the action longer than the applet lives. */
  g_signal_handlers_disconnect_by_data (applet->toggle, applet);
  g_object_unref (applet->toggle);
  g_free (applet);
}

static gboolean
fill (MatePanelApplet *panel_applet)
{
  GtkActionGroup *actions;
  GtkAction *configure;
  static const BtPopupActions popup_actions = {
    on_popup_set_powered, on_popup_set_connected, on_popup_add_device, on_popup_configure,
  };
  Applet *applet = g_new0 (Applet, 1);

  applet->applet = panel_applet;
  applet->state = BT_ICON_NONE;
  mate_panel_applet_set_flags (panel_applet, MATE_PANEL_APPLET_EXPAND_MINOR);
  mate_panel_applet_set_background_widget (panel_applet, GTK_WIDGET (panel_applet));
  applet->icon = caminito_applet_icon_new (panel_applet);
  gtk_container_add (GTK_CONTAINER (panel_applet), applet->icon);

  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  /* mate-panel's applet menu still takes GtkActions. */
  actions = gtk_action_group_new ("Bluetooth Applet Actions");
  applet->toggle = gtk_action_new ("Toggle", "Turn Bluetooth _off", NULL, NULL);
  g_signal_connect (applet->toggle, "activate", G_CALLBACK (on_toggle), applet);
  gtk_action_group_add_action (actions, applet->toggle);
  configure = gtk_action_new ("Configure", "_Configure Bluetooth…", NULL, "preferences-system");
  g_signal_connect (configure, "activate", G_CALLBACK (on_configure), NULL);
  gtk_action_group_add_action (actions, configure);
  g_object_unref (configure);
  G_GNUC_END_IGNORE_DEPRECATIONS
  mate_panel_applet_setup_menu (panel_applet, menu_xml, actions);
  g_object_unref (actions);

  g_object_set_data_full (G_OBJECT (panel_applet), "caminito-bluetooth-applet", applet,
                          applet_free);
  applet->popup = caminito_applet_popup_new (GTK_WIDGET (panel_applet));
  applet->content = bt_popup_new (applet->popup, &popup_actions, applet);
  applet->bluez = bt_bluez_new (on_changed, applet);
  /* Hidden until BlueZ says there's an adapter. */
  gtk_widget_show (applet->icon);
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

MATE_PANEL_APPLET_IN_PROCESS_FACTORY (FACTORY_ID, PANEL_TYPE_APPLET, "Bluetooth", factory, NULL)
