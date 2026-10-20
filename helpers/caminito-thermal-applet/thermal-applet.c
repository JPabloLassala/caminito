/*
 * The Thermal applet (267, ADR 0035): an in-process mate-panel applet, a thermometer with the
 * panel sensors' temperatures over the fastest fan's speed, in the manner of TG Pro's menu
 * bar item. It reads libsensors (th-source.c) every 2 s, or every 1 s while its popup is
 * open, and th-model.c makes of that what it shows. Below a 32 px panel it's one line, and
 * on a vertical panel the first panel sensor as a bare number. Left-click toggles the
 * popup, which 268 fills; right-click has Preferences, which 269 fills, and About.
 */

#include <mate-panel-applet.h>

#include "caminito-applet-icon.h"
#include "caminito-applet-popup.h"
#include "th-model.h"
#include "th-source.h"

#define FACTORY_ID "CaminitoThermalAppletFactory"
#define APPLET_ID  "CaminitoThermalApplet"
#define NAME       "Temperatures and Fans"

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
  ThModel             *model;
  guint                timeout;
  guint                ticks;
  GtkWidget           *preferences;
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

static void
read_now (Applet *applet)
{
  g_autoptr (GPtrArray) raws = th_source_read ();

  th_model_update (applet->model, raws);
  show (applet);
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

static void
on_preferences_response (GtkDialog *dialog, int response G_GNUC_UNUSED, gpointer data)
{
  Applet *applet = data;

  gtk_widget_destroy (GTK_WIDGET (dialog));
  applet->preferences = NULL;
}

/* Empty until 269 gives it °C or °F and the thresholds. */
static void
on_preferences (GtkAction *action G_GNUC_UNUSED, gpointer data)
{
  Applet *applet = data;
  GtkWidget *dialog;

  if (applet->preferences != NULL)
    {
      gtk_window_present (GTK_WINDOW (applet->preferences));
      return;
    }
  dialog = gtk_dialog_new_with_buttons (NAME " Preferences", NULL, 0, "_Close",
                                        GTK_RESPONSE_CLOSE, NULL);
  gtk_window_set_resizable (GTK_WINDOW (dialog), FALSE);
  gtk_window_set_icon_name (GTK_WINDOW (dialog), "preferences-system");
  g_signal_connect (dialog, "response", G_CALLBACK (on_preferences_response), applet);
  gtk_widget_show_all (dialog);
  applet->preferences = dialog;
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
                         "license-type", GTK_LICENSE_GPL_2_0,
                         NULL);
}

static void
applet_free (gpointer data)
{
  Applet *applet = data;

  g_source_remove (applet->timeout);
  if (applet->preferences != NULL)
    gtk_widget_destroy (applet->preferences);
  th_model_free (applet->model);
  th_source_release ();
  g_free (applet);
}

/* The popup's window, empty until 268 gives it its tiles and Show all sensors. */
static void
fill_popup (Applet *applet)
{
  GtkWidget *content = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);

  gtk_widget_set_size_request (content, 340, -1);
  g_object_set (content, "margin", 12, NULL);
  gtk_container_add (GTK_CONTAINER (caminito_applet_popup_get_window (applet->popup)), content);
  gtk_widget_show (content);
}

static gboolean
fill (MatePanelApplet *panel_applet)
{
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
  fill_popup (applet);
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
