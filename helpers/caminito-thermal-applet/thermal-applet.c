/*
 * The Thermal applet (267, ADR 0035): an in-process mate-panel applet, a thermometer with the
 * panel sensors' temperatures over the fastest fan's speed, in the manner of TG Pro's menu
 * bar item. It reads libsensors (th-source.c) every 2 s, or every 1 s while its popup is
 * open, and th-model.c makes of that what it shows. Below a 32 px panel it's one line, and
 * on a vertical panel the first panel sensor as a bare number. Left-click toggles the
 * popup, its tiles and Show all sensors (268, th-popup.c), whose pins choose the panel
 * sensors; right-click has Preferences, the units and the thresholds, and About. The
 * applet's GSettings keep all of these, and whether Show all is open (269).
 */

#include <math.h>
#include <mate-panel-applet.h>
#include <mate-panel-applet-gsettings.h>

#include "caminito-applet-icon.h"
#include "caminito-applet-popup.h"
#include "th-model.h"
#include "th-popup.h"
#include "th-source.h"

#define FACTORY_ID "CaminitoThermalAppletFactory"
#define APPLET_ID  "CaminitoThermalApplet"
#define NAME       "Temperatures and Fans"
#define SCHEMA_ID  "org.mate.panel.applet.caminito-thermal"

#define KEY_PANEL_SENSORS "panel-sensors"
#define KEY_UNITS         "units"
#define KEY_WARM          "warm-threshold"
#define KEY_HOT           "hot-threshold"
#define KEY_SHOW_ALL      "show-all"

/* The units key's values. */
#define UNITS_CELSIUS    0
#define UNITS_FAHRENHEIT 1

/* The thresholds' range in °C, the schema's. */
#define THRESHOLD_MIN 30.0
#define THRESHOLD_MAX 130.0

/* Two lines from a panel this tall. */
#define TWO_LINES_FROM 32

static const char *const thermometer[] = {
  "xsi-temperature-symbolic",
  "temperature-normal-symbolic",
  "sensors-temperature-symbolic",
  /* The applet's own, in hicolor: neither MATE's themes nor Adwaita have any of the above. */
  "caminito-thermometer-symbolic",
  NULL,
};

typedef struct
{
  MatePanelApplet     *applet;
  GtkWidget           *box;
  GtkWidget           *icon;
  GtkWidget           *top;
  GtkWidget           *bottom;
  CaminitoAppletPopup *popup;
  ThPopup             *content;
  ThModel             *model;
  guint                timeout;
  guint                ticks;
  /* The applet's own preferences, NULL where the schema isn't installed: they're then
     kept until mate-panel restarts. */
  GSettings           *settings;
  gboolean             fahrenheit;
  /* °C. */
  double               warm;
  double               hot;
  GtkWidget           *preferences;
  GtkWidget           *units;
  GtkWidget           *warm_spin;
  GtkWidget           *hot_spin;
  GtkWidget           *warm_unit;
  GtkWidget           *hot_unit;
} Applet;

static const char menu_xml[] =
  "<menuitem name='Preferences' action='Preferences'/>"
  "<menuitem name='About' action='About'/>";

static gboolean
is_vertical (Applet *applet)
{
  MatePanelAppletOrient orient = mate_panel_applet_get_orient (applet->applet);

  return orient == MATE_PANEL_APPLET_ORIENT_LEFT || orient == MATE_PANEL_APPLET_ORIENT_RIGHT;
}

/* Shows what the model has, the way the panel's size and orientation want it. */
static void
show (Applet *applet)
{
  g_autofree char *temperatures = th_model_panel_temperatures (applet->model);
  g_autofree char *fans = th_model_panel_fans (applet->model);
  g_autofree char *tooltip = th_model_tooltip (applet->model);
  gboolean vertical = is_vertical (applet);

  gtk_orientable_set_orientation (GTK_ORIENTABLE (applet->box),
                                  vertical ? GTK_ORIENTATION_VERTICAL
                                           : GTK_ORIENTATION_HORIZONTAL);
  if (vertical)
    {
      g_autofree char *bare = th_model_panel_bare (applet->model);

      gtk_label_set_markup (GTK_LABEL (applet->top), bare);
      gtk_widget_hide (applet->bottom);
    }
  else if (mate_panel_applet_get_size (applet->applet) >= TWO_LINES_FROM && fans != NULL)
    {
      g_autofree char *top = g_strdup_printf ("<span size='small'>%s</span>", temperatures);
      g_autofree char *bottom = g_markup_printf_escaped ("<span size='small'>%s</span>", fans);

      gtk_label_set_markup (GTK_LABEL (applet->top), top);
      gtk_label_set_markup (GTK_LABEL (applet->bottom), bottom);
      gtk_widget_show (applet->bottom);
    }
  else
    {
      g_autofree char *line = NULL;
      g_autofree char *escaped = fans != NULL ? g_markup_escape_text (fans, -1) : NULL;

      line = escaped != NULL ? g_strdup_printf ("%s  %s", temperatures, escaped)
                             : g_strdup (temperatures);
      gtk_label_set_markup (GTK_LABEL (applet->top), line);
      gtk_widget_hide (applet->bottom);
    }
  gtk_widget_set_tooltip_text (GTK_WIDGET (applet->applet), tooltip);
}

/* The panel and the popup, from what the model has now. */
static void
refresh (Applet *applet)
{
  show (applet);
  /* Closed too, so that it's right as it opens. */
  th_popup_update (applet->content, applet->model);
}

static void
read_now (Applet *applet)
{
  g_autoptr (GPtrArray) raws = th_source_read ();

  th_model_update (applet->model, raws);
  refresh (applet);
}

/* Every second; a read every other one, or every one while the popup is open. */
static gboolean
on_tick (gpointer data)
{
  Applet *applet = data;

  applet->ticks++;
  if (applet->ticks % 2 == 0 || caminito_applet_popup_is_shown (applet->popup))
    read_now (applet);
  return G_SOURCE_CONTINUE;
}

static void
on_change_orient (MatePanelApplet *panel_applet G_GNUC_UNUSED,
                  guint            orient G_GNUC_UNUSED,
                  gpointer         data)
{
  show (data);
}

static void
on_change_size (MatePanelApplet *panel_applet G_GNUC_UNUSED,
                guint            size G_GNUC_UNUSED,
                gpointer         data)
{
  show (data);
}

/* A temperature in °C in the dialog's units, and back. */
static double
to_units (Applet *applet, double celsius)
{
  return applet->fahrenheit ? celsius * 9 / 5 + 32 : celsius;
}

static double
from_units (Applet *applet, double value)
{
  return applet->fahrenheit ? (value - 32) * 5 / 9 : value;
}

static void on_units_changed (GtkComboBox *combo, gpointer data);
static void on_threshold_changed (GtkSpinButton *spin, gpointer data);

static void
sync_spin (Applet *applet, GtkWidget *spin, GtkWidget *unit, double celsius)
{
  g_signal_handlers_block_by_func (spin, on_threshold_changed, applet);
  gtk_spin_button_set_range (GTK_SPIN_BUTTON (spin), round (to_units (applet, THRESHOLD_MIN)),
                             round (to_units (applet, THRESHOLD_MAX)));
  gtk_spin_button_set_value (GTK_SPIN_BUTTON (spin), round (to_units (applet, celsius)));
  g_signal_handlers_unblock_by_func (spin, on_threshold_changed, applet);
  gtk_label_set_text (GTK_LABEL (unit), applet->fahrenheit ? "°F" : "°C");
}

/* The dialog follows a change from elsewhere too, dconf or another instance of the applet. */
static void
sync_dialog (Applet *applet)
{
  if (applet->preferences == NULL)
    return;
  g_signal_handlers_block_by_func (applet->units, on_units_changed, applet);
  gtk_combo_box_set_active (GTK_COMBO_BOX (applet->units),
                            applet->fahrenheit ? UNITS_FAHRENHEIT : UNITS_CELSIUS);
  g_signal_handlers_unblock_by_func (applet->units, on_units_changed, applet);
  sync_spin (applet, applet->warm_spin, applet->warm_unit, applet->warm);
  sync_spin (applet, applet->hot_spin, applet->hot_unit, applet->hot);
}

/* The units and the thresholds, which the model has no getters for, into it. */
static void
apply_preferences (Applet *applet)
{
  th_model_set_fahrenheit (applet->model, applet->fahrenheit);
  th_model_set_thresholds (applet->model, applet->warm, applet->hot);
}

/* Everything the preferences keep, into the model and the popup. */
static void
load_settings (Applet *applet)
{
  g_auto (GStrv) pins = g_settings_get_strv (applet->settings, KEY_PANEL_SENSORS);

  th_model_set_pinned (applet->model, (const char *const *) pins);
  applet->fahrenheit = g_settings_get_enum (applet->settings, KEY_UNITS) == UNITS_FAHRENHEIT;
  applet->warm = g_settings_get_double (applet->settings, KEY_WARM);
  applet->hot = g_settings_get_double (applet->settings, KEY_HOT);
  apply_preferences (applet);
  th_popup_set_show_all (applet->content, g_settings_get_boolean (applet->settings,
                                                                  KEY_SHOW_ALL));
}

static void
on_setting_changed (GSettings *settings G_GNUC_UNUSED, const char *key G_GNUC_UNUSED,
                    gpointer data)
{
  Applet *applet = data;

  load_settings (applet);
  refresh (applet);
  sync_dialog (applet);
}

/* What the dialog set goes into the preferences one key at a time, as each write's "changed"
   comes back to on_setting_changed at once and reads them all; without them, straight into
   the model. */
static void
on_units_changed (GtkComboBox *combo, gpointer data)
{
  Applet *applet = data;
  gboolean fahrenheit = gtk_combo_box_get_active (combo) == UNITS_FAHRENHEIT;

  if (applet->settings != NULL)
    {
      g_settings_set_enum (applet->settings, KEY_UNITS,
                           fahrenheit ? UNITS_FAHRENHEIT : UNITS_CELSIUS);
      return;
    }
  applet->fahrenheit = fahrenheit;
  apply_preferences (applet);
  refresh (applet);
  /* The thresholds stay as they are in °C, and are shown in the new units. */
  sync_dialog (applet);
}

static void
on_threshold_changed (GtkSpinButton *spin, gpointer data)
{
  Applet *applet = data;
  gboolean warm = GTK_WIDGET (spin) == applet->warm_spin;
  double celsius = CLAMP (from_units (applet, gtk_spin_button_get_value (spin)), THRESHOLD_MIN,
                          THRESHOLD_MAX);

  if (applet->settings != NULL)
    {
      g_settings_set_double (applet->settings, warm ? KEY_WARM : KEY_HOT, celsius);
      return;
    }
  if (warm)
    applet->warm = celsius;
  else
    applet->hot = celsius;
  apply_preferences (applet);
  refresh (applet);
}

/* The popup's pin: the panel and the tooltip change at once, and the preferences keep it. */
static void
on_popup_pin (const char *id, gpointer data)
{
  Applet *applet = data;

  if (!th_model_toggle_pin (applet->model, id))
    return;
  refresh (applet);
  if (applet->settings != NULL)
    g_settings_set_strv (applet->settings, KEY_PANEL_SENSORS, th_model_pinned (applet->model));
}

static void
on_popup_show_all (gboolean open, gpointer data)
{
  Applet *applet = data;

  if (applet->settings != NULL)
    g_settings_set_boolean (applet->settings, KEY_SHOW_ALL, open);
}

static void
on_preferences_response (GtkDialog *dialog, int response G_GNUC_UNUSED, gpointer data)
{
  Applet *applet = data;

  gtk_widget_destroy (GTK_WIDGET (dialog));
  applet->preferences = NULL;
}

/* A threshold's row: its label, a spin button and the unit. */
static GtkWidget *
threshold_add (Applet *applet, GtkWidget *grid, int row, const char *mnemonic, GtkWidget **unit)
{
  GtkWidget *label = gtk_label_new_with_mnemonic (mnemonic);
  GtkWidget *spin = gtk_spin_button_new_with_range (0, 1, 1);

  gtk_label_set_xalign (GTK_LABEL (label), 0);
  gtk_label_set_mnemonic_widget (GTK_LABEL (label), spin);
  gtk_spin_button_set_digits (GTK_SPIN_BUTTON (spin), 0);
  *unit = gtk_label_new (NULL);
  gtk_label_set_xalign (GTK_LABEL (*unit), 0);
  gtk_grid_attach (GTK_GRID (grid), label, 0, row, 1, 1);
  gtk_grid_attach (GTK_GRID (grid), spin, 1, row, 1, 1);
  gtk_grid_attach (GTK_GRID (grid), *unit, 2, row, 1, 1);
  g_signal_connect (spin, "value-changed", G_CALLBACK (on_threshold_changed), applet);
  return spin;
}

/* The units and the two thresholds. */
static void
on_preferences (GtkAction *action G_GNUC_UNUSED, gpointer data)
{
  Applet *applet = data;
  GtkWidget *dialog;
  GtkWidget *grid;
  GtkWidget *label;

  if (applet->preferences != NULL)
    {
      gtk_window_present (GTK_WINDOW (applet->preferences));
      return;
    }
  dialog = gtk_dialog_new_with_buttons (NAME " Preferences", NULL, 0, "_Close",
                                        GTK_RESPONSE_CLOSE, NULL);
  gtk_window_set_resizable (GTK_WINDOW (dialog), FALSE);
  gtk_window_set_icon_name (GTK_WINDOW (dialog), "preferences-system");
  grid = gtk_grid_new ();
  gtk_grid_set_row_spacing (GTK_GRID (grid), 6);
  gtk_grid_set_column_spacing (GTK_GRID (grid), 12);
  g_object_set (grid, "margin", 12, NULL);

  label = gtk_label_new_with_mnemonic ("_Units:");
  gtk_label_set_xalign (GTK_LABEL (label), 0);
  applet->units = gtk_combo_box_text_new ();
  gtk_combo_box_text_append_text (GTK_COMBO_BOX_TEXT (applet->units), "Celsius (°C)");
  gtk_combo_box_text_append_text (GTK_COMBO_BOX_TEXT (applet->units), "Fahrenheit (°F)");
  gtk_label_set_mnemonic_widget (GTK_LABEL (label), applet->units);
  gtk_grid_attach (GTK_GRID (grid), label, 0, 0, 1, 1);
  gtk_grid_attach (GTK_GRID (grid), applet->units, 1, 0, 2, 1);
  g_signal_connect (applet->units, "changed", G_CALLBACK (on_units_changed), applet);
  applet->warm_spin = threshold_add (applet, grid, 1, "_Warm from:", &applet->warm_unit);
  applet->hot_spin = threshold_add (applet, grid, 2, "_Hot from:", &applet->hot_unit);
  gtk_container_add (GTK_CONTAINER (gtk_dialog_get_content_area (GTK_DIALOG (dialog))), grid);
  g_signal_connect (dialog, "response", G_CALLBACK (on_preferences_response), applet);
  applet->preferences = dialog;
  sync_dialog (applet);
  gtk_widget_show_all (dialog);
}

static void
on_about (GtkAction *action G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
  GtkIconTheme *theme = gtk_icon_theme_get_default ();
  const char *logo = NULL;

  for (guint i = 0; thermometer[i] != NULL && logo == NULL; i++)
    if (gtk_icon_theme_has_icon (theme, thermometer[i]))
      logo = thermometer[i];
  gtk_show_about_dialog (NULL,
                         "program-name", NAME,
                         "version", VERSION,
                         "comments", "The computer's temperatures and fan speeds. It only "
                                     "watches: it never sets a fan's speed.",
                         "logo-icon-name", logo,
                         "license-type", GTK_LICENSE_GPL_3_0,
                         NULL);
}

static void
applet_free (gpointer data)
{
  Applet *applet = data;

  g_source_remove (applet->timeout);
  if (applet->preferences != NULL)
    gtk_widget_destroy (applet->preferences);
  if (applet->settings != NULL)
    {
      g_signal_handlers_disconnect_by_data (applet->settings, applet);
      g_object_unref (applet->settings);
    }
  th_popup_free (applet->content);
  th_model_free (applet->model);
  th_source_release ();
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
      g_warning ("the schema %s isn't installed: the preferences won't be kept", SCHEMA_ID);
      return NULL;
    }
  return mate_panel_applet_settings_new (panel_applet, (char *) SCHEMA_ID);
}

static gboolean
fill (MatePanelApplet *panel_applet)
{
  static const ThPopupActions popup_actions = { on_popup_pin, on_popup_show_all };
  GtkActionGroup *actions;
  GtkAction *action;
  GtkWidget *lines;
  Applet *applet = g_new0 (Applet, 1);

  applet->applet = panel_applet;
  mate_panel_applet_set_flags (panel_applet, MATE_PANEL_APPLET_EXPAND_MINOR);
  mate_panel_applet_set_background_widget (panel_applet, GTK_WIDGET (panel_applet));
  applet->box = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 4);
  applet->icon = caminito_applet_icon_new (panel_applet);
  caminito_applet_icon_set (applet->icon, thermometer);
  lines = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  gtk_widget_set_valign (lines, GTK_ALIGN_CENTER);
  applet->top = gtk_label_new (NULL);
  applet->bottom = gtk_label_new (NULL);
  gtk_box_pack_start (GTK_BOX (lines), applet->top, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (lines), applet->bottom, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (applet->box), applet->icon, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (applet->box), lines, FALSE, FALSE, 0);
  gtk_container_add (GTK_CONTAINER (panel_applet), applet->box);

  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  /* mate-panel's applet menu still takes GtkActions. */
  actions = gtk_action_group_new ("Thermal Applet Actions");
  action = gtk_action_new ("Preferences", "_Preferences", NULL, "preferences-system");
  g_signal_connect (action, "activate", G_CALLBACK (on_preferences), applet);
  gtk_action_group_add_action (actions, action);
  g_object_unref (action);
  action = gtk_action_new ("About", "_About", NULL, "help-about");
  g_signal_connect (action, "activate", G_CALLBACK (on_about), applet);
  gtk_action_group_add_action (actions, action);
  g_object_unref (action);
  G_GNUC_END_IGNORE_DEPRECATIONS
  mate_panel_applet_setup_menu (panel_applet, menu_xml, actions);
  g_object_unref (actions);

  th_source_acquire ();
  applet->model = th_model_new ();
  g_object_set_data_full (G_OBJECT (panel_applet), "caminito-thermal-applet", applet,
                          applet_free);
  applet->popup = caminito_applet_popup_new (GTK_WIDGET (panel_applet));
  applet->content = th_popup_new (applet->popup, &popup_actions, applet);
  applet->warm = TH_WARM_DEFAULT;
  applet->hot = TH_HOT_DEFAULT;
  applet->settings = settings_new (panel_applet);
  if (applet->settings != NULL)
    {
      load_settings (applet);
      g_signal_connect (applet->settings, "changed", G_CALLBACK (on_setting_changed), applet);
    }
  g_signal_connect (panel_applet, "change-orient", G_CALLBACK (on_change_orient), applet);
  g_signal_connect (panel_applet, "change-size", G_CALLBACK (on_change_size), applet);
  gtk_widget_show_all (GTK_WIDGET (panel_applet));
  read_now (applet);
  applet->timeout = g_timeout_add_seconds (1, on_tick, applet);
  return TRUE;
}

static gboolean
factory (MatePanelApplet *applet, const char *iid, gpointer data G_GNUC_UNUSED)
{
  if (g_strcmp0 (iid, APPLET_ID) != 0)
    return FALSE;
  return fill (applet);
}

MATE_PANEL_APPLET_IN_PROCESS_FACTORY (FACTORY_ID, PANEL_TYPE_APPLET, NAME, factory, NULL)
