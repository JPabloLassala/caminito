/*
 * The Power applet (262, ADR 0035): an in-process mate-panel applet whose icon shows
 * UPower's display device, a battery glyph by its percentage, a charging variant, or a plug
 * with no battery, with the percentage and the time in its tooltip. It reads UPower
 * directly (pw-upower.c). "Show percentage" is its own preference and puts the percentage
 * beside the glyph. Left-click opens the popup (pw-popup.c), which lists the batteries
 * (263) and sets their charge limit through UPower; its right-click menu has
 * Preferences and "Configure Power Management…", which run mate-power-preferences.
 */

#include <mate-panel-applet.h>
#include <mate-panel-applet-gsettings.h>

#include "caminito-applet-icon.h"
#include "caminito-applet-popup.h"
#include "pw-popup.h"
#include "pw-state.h"
#include "pw-upower.h"

#define FACTORY_ID       "CaminitoPowerAppletFactory"
#define APPLET_ID        "CaminitoPowerApplet"
#define SCHEMA_ID        "org.mate.panel.applet.caminito-power"
#define KEY_SHOW_PERCENT "show-percentage"

typedef struct
{
  MatePanelApplet     *applet;
  GtkWidget           *box;
  GtkWidget           *icon;
  GtkWidget           *label;
  PwUpower            *upower;
  CaminitoAppletPopup *popup;
  PwPopup             *content;
  /* The applet's own preferences, NULL where the schema isn't installed: the preference
     then lasts as long as the applet. */
  GSettings           *settings;
  gboolean             show_percentage;
  GtkWidget           *preferences;
  GtkWidget           *preferences_check;
} Applet;

static const char menu_xml[] =
  "<menuitem name='Preferences' action='Preferences'/>"
  "<menuitem name='Configure' action='Configure'/>";

static void
update (Applet *applet)
{
  PwDisplay display;
  g_autofree char *tooltip = NULL;
  g_autofree char *percentage = NULL;
  g_autoptr (GPtrArray) devices = NULL;

  pw_upower_display (applet->upower, &display);
  devices = pw_upower_devices (applet->upower);
  pw_popup_update (applet->content, devices);
  tooltip = pw_state_tooltip (&display);
  percentage = pw_state_percentage (&display);
  caminito_applet_icon_set (applet->icon, pw_state_icon_names (&display));
  gtk_widget_set_tooltip_text (GTK_WIDGET (applet->applet), tooltip);
  gtk_label_set_text (GTK_LABEL (applet->label), percentage);
  gtk_widget_set_visible (applet->label, applet->show_percentage && percentage != NULL);
}

static void
on_changed (PwUpower *upower G_GNUC_UNUSED, gpointer data)
{
  update (data);
}

static void on_show_percentage_toggled (GtkToggleButton *check, gpointer data);

static void
set_show_percentage (Applet *applet, gboolean show)
{
  if (applet->settings != NULL)
    {
      /* "changed" comes back to on_setting_changed, which updates. */
      g_settings_set_boolean (applet->settings, KEY_SHOW_PERCENT, show);
      return;
    }
  applet->show_percentage = show;
  update (applet);
}

static void
on_setting_changed (GSettings *settings, const char *key G_GNUC_UNUSED, gpointer data)
{
  Applet *applet = data;

  applet->show_percentage = g_settings_get_boolean (settings, KEY_SHOW_PERCENT);
  /* The dialog follows a change from elsewhere, dconf or another instance of the applet. */
  if (applet->preferences_check != NULL)
    {
      g_signal_handlers_block_by_func (applet->preferences_check, on_show_percentage_toggled,
                                       applet);
      gtk_toggle_button_set_active (GTK_TOGGLE_BUTTON (applet->preferences_check),
                                    applet->show_percentage);
      g_signal_handlers_unblock_by_func (applet->preferences_check, on_show_percentage_toggled,
                                         applet);
    }
  update (applet);
}

static void
on_orient_changed (MatePanelApplet *panel_applet G_GNUC_UNUSED,
                   guint            orient,
                   gpointer         data)
{
  Applet *applet = data;
  gboolean vertical = orient == MATE_PANEL_APPLET_ORIENT_LEFT
                      || orient == MATE_PANEL_APPLET_ORIENT_RIGHT;

  gtk_orientable_set_orientation (GTK_ORIENTABLE (applet->box),
                                  vertical ? GTK_ORIENTATION_VERTICAL
                                           : GTK_ORIENTATION_HORIZONTAL);
}

static void
configure_power_management (void)
{
  g_autoptr (GError) error = NULL;
  const char *argv[] = { "mate-power-preferences", NULL };

  if (!g_spawn_async (NULL, (char **) argv, NULL,
                      G_SPAWN_SEARCH_PATH | G_SPAWN_STDOUT_TO_DEV_NULL, NULL, NULL, NULL,
                      &error))
    g_warning ("can't open mate-power-preferences: %s", error->message);
}

static void
on_configure (GtkAction *action G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
  configure_power_management ();
}

static void
on_popup_configure (gpointer data G_GNUC_UNUSED)
{
  configure_power_management ();
}

static void
on_limit_done (const char *path, const char *error, gpointer data)
{
  Applet *applet = data;

  if (error != NULL)
    g_warning ("can't change the charge limit of %s: %s", path, error);
  pw_popup_limit_done (applet->content, path, error);
}

static void
on_popup_set_limit (const char *path, gboolean enable, gpointer data)
{
  Applet *applet = data;

  pw_upower_enable_charge_threshold (applet->upower, path, enable, on_limit_done, applet);
}

static void
on_show_percentage_toggled (GtkToggleButton *check, gpointer data)
{
  set_show_percentage (data, gtk_toggle_button_get_active (check));
}

static void
on_preferences_response (GtkDialog *dialog, int response G_GNUC_UNUSED, gpointer data)
{
  Applet *applet = data;

  gtk_widget_destroy (GTK_WIDGET (dialog));
  applet->preferences = NULL;
  applet->preferences_check = NULL;
}

static void
on_preferences (GtkAction *action G_GNUC_UNUSED, gpointer data)
{
  Applet *applet = data;
  GtkWidget *dialog;
  GtkWidget *check;

  if (applet->preferences != NULL)
    {
      gtk_window_present (GTK_WINDOW (applet->preferences));
      return;
    }
  dialog = gtk_dialog_new_with_buttons ("Power Preferences", NULL, 0, "_Close", GTK_RESPONSE_CLOSE,
                                        NULL);
  gtk_window_set_resizable (GTK_WINDOW (dialog), FALSE);
  gtk_container_set_border_width (GTK_CONTAINER (gtk_dialog_get_content_area (GTK_DIALOG (dialog))),
                                  12);
  check = gtk_check_button_new_with_mnemonic ("_Show percentage");
  gtk_toggle_button_set_active (GTK_TOGGLE_BUTTON (check), applet->show_percentage);
  applet->preferences_check = check;
  g_signal_connect (check, "toggled", G_CALLBACK (on_show_percentage_toggled), applet);
  gtk_container_add (GTK_CONTAINER (gtk_dialog_get_content_area (GTK_DIALOG (dialog))), check);
  g_signal_connect (dialog, "response", G_CALLBACK (on_preferences_response), applet);
  gtk_widget_show_all (dialog);
  applet->preferences = dialog;
}

static void
applet_free (gpointer data)
{
  Applet *applet = data;

  /* The popup's own handlers can't run any more: its window goes with the applet. */
  pw_popup_free (applet->content);
  pw_upower_free (applet->upower);
  if (applet->preferences != NULL)
    gtk_widget_destroy (applet->preferences);
  if (applet->settings != NULL)
    {
      g_signal_handlers_disconnect_by_data (applet->settings, applet);
      g_object_unref (applet->settings);
    }
  g_free (applet);
}

/* The applet's settings, or NULL where its schema isn't installed, which would abort. */
static GSettings *
settings_new (MatePanelApplet *panel_applet)
{
  GSettingsSchemaSource *source = g_settings_schema_source_get_default ();
  g_autoptr (GSettingsSchema) schema = NULL;

  if (source != NULL)
    schema = g_settings_schema_source_lookup (source, SCHEMA_ID, TRUE);
  if (schema == NULL)
    {
      g_warning ("the schema %s isn't installed: \"Show percentage\" won't be kept",
                 SCHEMA_ID);
      return NULL;
    }
  return mate_panel_applet_settings_new (panel_applet, (char *) SCHEMA_ID);
}

static gboolean
fill (MatePanelApplet *panel_applet)
{
  static const PwPopupActions popup_actions = { on_popup_configure, on_popup_set_limit };
  GtkActionGroup *actions;
  GtkAction *action;
  Applet *applet = g_new0 (Applet, 1);

  applet->applet = panel_applet;
  mate_panel_applet_set_flags (panel_applet, MATE_PANEL_APPLET_EXPAND_MINOR);
  mate_panel_applet_set_background_widget (panel_applet, GTK_WIDGET (panel_applet));
  applet->box = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 2);
  applet->icon = caminito_applet_icon_new (panel_applet);
  applet->label = gtk_label_new (NULL);
  gtk_box_pack_start (GTK_BOX (applet->box), applet->icon, TRUE, TRUE, 0);
  gtk_box_pack_start (GTK_BOX (applet->box), applet->label, FALSE, FALSE, 0);
  gtk_container_add (GTK_CONTAINER (panel_applet), applet->box);
  g_signal_connect (panel_applet, "change-orient", G_CALLBACK (on_orient_changed), applet);
  on_orient_changed (panel_applet, mate_panel_applet_get_orient (panel_applet), applet);

  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  /* mate-panel's applet menu still takes GtkActions. */
  actions = gtk_action_group_new ("Power Applet Actions");
  action = gtk_action_new ("Preferences", "_Preferences", NULL, "preferences-system");
  g_signal_connect (action, "activate", G_CALLBACK (on_preferences), applet);
  gtk_action_group_add_action (actions, action);
  g_object_unref (action);
  action = gtk_action_new ("Configure", "_Configure Power Management…", NULL, NULL);
  g_signal_connect (action, "activate", G_CALLBACK (on_configure), NULL);
  gtk_action_group_add_action (actions, action);
  g_object_unref (action);
  G_GNUC_END_IGNORE_DEPRECATIONS
  mate_panel_applet_setup_menu (panel_applet, menu_xml, actions);
  g_object_unref (actions);

  g_object_set_data_full (G_OBJECT (panel_applet), "caminito-power-applet", applet, applet_free);
  applet->settings = settings_new (panel_applet);
  if (applet->settings != NULL)
    {
      applet->show_percentage = g_settings_get_boolean (applet->settings, KEY_SHOW_PERCENT);
      g_signal_connect (applet->settings, "changed::" KEY_SHOW_PERCENT,
                        G_CALLBACK (on_setting_changed), applet);
    }
  applet->popup = caminito_applet_popup_new (GTK_WIDGET (panel_applet));
  applet->content = pw_popup_new (applet->popup, &popup_actions, applet);
  applet->upower = pw_upower_new (on_changed, applet);
  gtk_widget_show_all (GTK_WIDGET (panel_applet));
  /* A plug until UPower answers, which is what a computer with no battery shows too. */
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

MATE_PANEL_APPLET_IN_PROCESS_FACTORY (FACTORY_ID, PANEL_TYPE_APPLET, "Power", factory, NULL)
