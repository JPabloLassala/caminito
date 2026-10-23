/*
 * The Sound applet (CMN-213, ADR 0035): an in-process mate-panel applet whose icon shows the
 * Default output, the speaker by its level or headphones, crossed out when muted and struck
 * through with no sound server, with a red dot while an application records. It reads the
 * sound server through libpulse (sd-pulse.c). The wheel on it sets the Default output's
 * volume and middle-click mutes it; left-click toggles the popup (sd-popup.c). Its
 * right-click menu mutes the output and the microphone, has a section of profiles for each
 * card, and "Sound settings…", which runs mate-volume-control.
 */

#include <mate-panel-applet.h>

#include "caminito-applet-icon.h"
#include "caminito-applet-popup.h"
#include "sd-popup.h"
#include "sd-pulse.h"
#include "sd-state.h"

#define FACTORY_ID "CaminitoSoundAppletFactory"
#define APPLET_ID  "CaminitoSoundApplet"

#define MEDIA_KEYS_SCHEMA "org.mate.SettingsDaemon.plugins.media-keys"
#define SOUND_SCHEMA      "org.gnome.desktop.sound"
/* mate-settings-daemon's own default, where its schema isn't installed. */
#define DEFAULT_STEP 5

typedef struct
{
  MatePanelApplet     *applet;
  GtkWidget           *icon;
  /* The strike and the red dot, over the icon. */
  GtkWidget           *marks;
  SdPulse             *pulse;
  CaminitoAppletPopup *popup;
  SdPopup             *content;
  /* NULL where their schemas aren't installed. */
  GSettings           *media_keys;
  GSettings           *sound;
  /* A touchpad's scroll, until it makes up a step. */
  double               scrolled;
  GtkToggleAction     *mute_output;
  GtkToggleAction     *mute_input;
  GtkAction           *settings;
  /* The profiles' items, in the panel's menu before "Sound settings…", and the cards they
     were made from. */
  GPtrArray           *profile_items;
  SdStatus             cards;
  /* The menu is being set to the status, not changed by the user. */
  gboolean             updating;
} Applet;

static const char menu_xml[] =
  "<menuitem name='MuteOutput' action='MuteOutput'/>"
  "<menuitem name='MuteInput' action='MuteInput'/>"
  "<separator/>"
  "<menuitem name='Settings' action='Settings'/>";

static void
on_profile_toggled (GtkCheckMenuItem *item, gpointer data)
{
  Applet *applet = data;

  if (applet->updating || !gtk_check_menu_item_get_active (item))
    return;
  sd_pulse_set_profile (applet->pulse,
                        GPOINTER_TO_UINT (g_object_get_data (G_OBJECT (item), "card")),
                        g_object_get_data (G_OBJECT (item), "profile"));
}

static void
add_profile_item (Applet *applet, GtkMenuShell *menu, int *position, GtkWidget *item)
{
  gtk_menu_shell_insert (menu, item, (*position)++);
  gtk_widget_show (item);
  g_ptr_array_add (applet->profile_items, item);
}

/* mate-panel builds the menu from the XML once, so the cards' sections go into its GtkMenu
   by hand: the one "Sound settings…" is in. */
static void
update_profiles (Applet *applet, const SdStatus *status)
{
  GSList *proxies;
  GtkWidget *settings_item;
  GtkMenuShell *menu;
  GList *children;
  int position;

  if (sd_cards_equal (&applet->cards, status))
    return;
  sd_status_clear (&applet->cards);
  sd_cards_copy (status, &applet->cards);
  g_ptr_array_set_size (applet->profile_items, 0);
  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  proxies = gtk_action_get_proxies (applet->settings);
  G_GNUC_END_IGNORE_DEPRECATIONS
  if (proxies == NULL)
    return;
  settings_item = proxies->data;
  menu = GTK_MENU_SHELL (gtk_widget_get_parent (settings_item));
  children = gtk_container_get_children (GTK_CONTAINER (menu));
  position = g_list_index (children, settings_item);
  g_list_free (children);
  for (guint i = 0; i < status->n_cards; i++)
    {
      const SdCard *card = &status->cards[i];
      GSList *group = NULL;
      GtkWidget *heading = gtk_menu_item_new_with_label (card->description);

      gtk_widget_set_sensitive (heading, FALSE);
      add_profile_item (applet, menu, &position, heading);
      for (guint j = 0; j < card->n_profiles; j++)
        {
          GtkWidget *item = gtk_radio_menu_item_new_with_label (group,
                                                                card->profiles[j].description);

          group = gtk_radio_menu_item_get_group (GTK_RADIO_MENU_ITEM (item));
          gtk_check_menu_item_set_active (GTK_CHECK_MENU_ITEM (item),
                                          g_strcmp0 (card->profiles[j].name, card->active) == 0);
          g_object_set_data (G_OBJECT (item), "card", GUINT_TO_POINTER (card->index));
          g_object_set_data_full (G_OBJECT (item), "profile",
                                  g_strdup (card->profiles[j].name), g_free);
          g_signal_connect (item, "toggled", G_CALLBACK (on_profile_toggled), applet);
          add_profile_item (applet, menu, &position, item);
        }
      add_profile_item (applet, menu, &position, gtk_separator_menu_item_new ());
    }
}

static void
update (Applet *applet)
{
  const SdStatus *status = sd_pulse_status (applet->pulse);
  g_autofree char *tooltip = sd_state_tooltip (status);

  caminito_applet_icon_set (applet->icon, sd_state_icon_names (status));
  gtk_widget_queue_draw (applet->marks);
  gtk_widget_set_tooltip_text (GTK_WIDGET (applet->applet), tooltip);
  sd_popup_update (applet->content, status);

  applet->updating = TRUE;
  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  gtk_toggle_action_set_active (applet->mute_output, status->has_output && status->muted);
  gtk_action_set_sensitive (GTK_ACTION (applet->mute_output), status->has_output);
  gtk_toggle_action_set_active (applet->mute_input, status->has_input && status->input_muted);
  gtk_action_set_sensitive (GTK_ACTION (applet->mute_input), status->has_input);
  G_GNUC_END_IGNORE_DEPRECATIONS
  update_profiles (applet, status);
  applet->updating = FALSE;
}

static void
on_changed (SdPulse *pulse G_GNUC_UNUSED, gpointer data)
{
  update (data);
}

/* A symbolic icon is drawn in the foreground's colour: a halo of the background's around a
   mark keeps it apart from the icon under it. */
static gboolean
on_draw_marks (GtkWidget *marks, cairo_t *cr, gpointer data)
{
  Applet *applet = data;
  const SdStatus *status = sd_pulse_status (applet->pulse);
  GtkStyleContext *style = gtk_widget_get_style_context (marks);
  double size = gtk_image_get_pixel_size (GTK_IMAGE (applet->icon));
  double x = (gtk_widget_get_allocated_width (marks) - size) / 2;
  double y = (gtk_widget_get_allocated_height (marks) - size) / 2;
  GdkRGBA ink, background = { 0, 0, 0, 0 };

  gtk_style_context_get_color (style, gtk_style_context_get_state (style), &ink);
  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  gtk_style_context_lookup_color (style, "theme_bg_color", &background);
  G_GNUC_END_IGNORE_DEPRECATIONS
  if (sd_state_icon_struck (status))
    {
      /* Corner to corner, as audio-volume-muted's own cross runs. */
      cairo_set_line_cap (cr, CAIRO_LINE_CAP_ROUND);
      cairo_move_to (cr, x + size * 0.15, y + size * 0.15);
      cairo_line_to (cr, x + size * 0.85, y + size * 0.85);
      gdk_cairo_set_source_rgba (cr, &background);
      cairo_set_line_width (cr, size / 6);
      cairo_stroke_preserve (cr);
      gdk_cairo_set_source_rgba (cr, &ink);
      cairo_set_line_width (cr, size / 12);
      cairo_stroke (cr);
    }
  if (status->running && status->recording != NULL)
    {
      double radius = size / 7;

      /* In the top right-hand corner. */
      cairo_arc (cr, x + size - radius, y + radius, radius + 1, 0, 2 * G_PI);
      gdk_cairo_set_source_rgba (cr, &background);
      cairo_fill (cr);
      cairo_arc (cr, x + size - radius, y + radius, radius, 0, 2 * G_PI);
      cairo_set_source_rgb (cr, 0.88, 0.11, 0.14);
      cairo_fill (cr);
    }
  return FALSE;
}

static guint
volume_step (Applet *applet)
{
  int step;

  if (applet->media_keys == NULL)
    return DEFAULT_STEP;
  step = g_settings_get_int (applet->media_keys, "volume-step");
  return step > 0 ? (guint) step : DEFAULT_STEP;
}

static void
scroll_step (Applet *applet, gboolean up)
{
  const SdStatus *status = sd_pulse_status (applet->pulse);
  gboolean above_100 = applet->sound != NULL
                       && g_settings_get_boolean (applet->sound,
                                                  "allow-volume-above-100-percent");

  if (!status->has_output)
    return;
  sd_pulse_set_volume (applet->pulse, sd_state_scroll_volume (status->volume,
                                                              volume_step (applet), up,
                                                              above_100));
  /* As the volume keys do. */
  if (up && status->muted)
    sd_pulse_set_muted (applet->pulse, FALSE);
}

static gboolean
on_scroll (GtkWidget *widget G_GNUC_UNUSED, GdkEventScroll *event, gpointer data)
{
  Applet *applet = data;
  double dx, dy;

  switch (event->direction)
    {
    case GDK_SCROLL_UP:
      scroll_step (applet, TRUE);
      return TRUE;
    case GDK_SCROLL_DOWN:
      scroll_step (applet, FALSE);
      return TRUE;
    case GDK_SCROLL_SMOOTH:
      if (!gdk_event_get_scroll_deltas ((GdkEvent *) event, &dx, &dy))
        return FALSE;
      applet->scrolled += dy;
      while (applet->scrolled <= -1)
        {
          applet->scrolled += 1;
          scroll_step (applet, TRUE);
        }
      while (applet->scrolled >= 1)
        {
          applet->scrolled -= 1;
          scroll_step (applet, FALSE);
        }
      return TRUE;
    default:
      return FALSE;
    }
}

/* Before mate-panel's own handler, which would take a middle-click to move the applet. */
static gboolean
on_button_press (GtkWidget *widget G_GNUC_UNUSED, GdkEventButton *event, gpointer data)
{
  Applet *applet = data;
  const SdStatus *status = sd_pulse_status (applet->pulse);

  if (event->button != GDK_BUTTON_MIDDLE || event->type != GDK_BUTTON_PRESS)
    return FALSE;
  if (status->has_output)
    sd_pulse_set_muted (applet->pulse, !status->muted);
  return TRUE;
}

static void
on_mute_output (GtkToggleAction *action, gpointer data)
{
  Applet *applet = data;

  if (applet->updating)
    return;
  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  sd_pulse_set_muted (applet->pulse, gtk_toggle_action_get_active (action));
  G_GNUC_END_IGNORE_DEPRECATIONS
}

static void
on_mute_input (GtkToggleAction *action, gpointer data)
{
  Applet *applet = data;

  if (applet->updating)
    return;
  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  sd_pulse_set_input_muted (applet->pulse, gtk_toggle_action_get_active (action));
  G_GNUC_END_IGNORE_DEPRECATIONS
}

static void
on_settings (GtkAction *action G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
  const char *const argv[] = { "mate-volume-control", NULL };
  g_autoptr (GError) error = NULL;

  if (!g_spawn_async (NULL, (char **) argv, NULL,
                      G_SPAWN_SEARCH_PATH | G_SPAWN_STDOUT_TO_DEV_NULL, NULL, NULL, NULL,
                      &error))
    g_warning ("can't open mate-volume-control: %s", error->message);
}

/* id's settings, or NULL where its schema isn't installed, which would abort. */
static GSettings *
settings_new (const char *id)
{
  GSettingsSchemaSource *source = g_settings_schema_source_get_default ();
  g_autoptr (GSettingsSchema) schema = NULL;

  if (source != NULL)
    schema = g_settings_schema_source_lookup (source, id, TRUE);
  if (schema == NULL)
    return NULL;
  return g_settings_new (id);
}

static void
applet_free (gpointer data)
{
  Applet *applet = data;

  /* The popup's window goes with the applet. */
  sd_popup_free (applet->content);
  sd_pulse_free (applet->pulse);
  g_clear_object (&applet->media_keys);
  g_clear_object (&applet->sound);
  g_ptr_array_unref (applet->profile_items);
  sd_status_clear (&applet->cards);
  g_free (applet);
}

static void
setup_menu (Applet *applet)
{
  GtkActionGroup *actions;

  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  /* mate-panel's applet menu still takes GtkActions. */
  actions = gtk_action_group_new ("Sound Applet Actions");
  applet->mute_output = gtk_toggle_action_new ("MuteOutput", "Mute _output", NULL, NULL);
  g_signal_connect (applet->mute_output, "toggled", G_CALLBACK (on_mute_output), applet);
  gtk_action_group_add_action (actions, GTK_ACTION (applet->mute_output));
  g_object_unref (applet->mute_output);
  applet->mute_input = gtk_toggle_action_new ("MuteInput", "Mute _microphone", NULL, NULL);
  g_signal_connect (applet->mute_input, "toggled", G_CALLBACK (on_mute_input), applet);
  gtk_action_group_add_action (actions, GTK_ACTION (applet->mute_input));
  g_object_unref (applet->mute_input);
  applet->settings = gtk_action_new ("Settings", "_Sound settings…", NULL, NULL);
  g_signal_connect (applet->settings, "activate", G_CALLBACK (on_settings), NULL);
  gtk_action_group_add_action (actions, applet->settings);
  g_object_unref (applet->settings);
  G_GNUC_END_IGNORE_DEPRECATIONS
  /* The group keeps the actions, and the applet keeps the group. */
  mate_panel_applet_setup_menu (applet->applet, menu_xml, actions);
  g_object_unref (actions);
}

static gboolean
fill (MatePanelApplet *panel_applet)
{
  GtkWidget *overlay;
  Applet *applet = g_new0 (Applet, 1);

  applet->applet = panel_applet;
  applet->profile_items = g_ptr_array_new_with_free_func ((GDestroyNotify) gtk_widget_destroy);
  mate_panel_applet_set_flags (panel_applet, MATE_PANEL_APPLET_EXPAND_MINOR);
  mate_panel_applet_set_background_widget (panel_applet, GTK_WIDGET (panel_applet));
  overlay = gtk_overlay_new ();
  applet->icon = caminito_applet_icon_new (panel_applet);
  gtk_container_add (GTK_CONTAINER (overlay), applet->icon);
  applet->marks = gtk_drawing_area_new ();
  g_signal_connect (applet->marks, "draw", G_CALLBACK (on_draw_marks), applet);
  gtk_overlay_add_overlay (GTK_OVERLAY (overlay), applet->marks);
  gtk_overlay_set_overlay_pass_through (GTK_OVERLAY (overlay), applet->marks, TRUE);
  gtk_container_add (GTK_CONTAINER (panel_applet), overlay);

  gtk_widget_add_events (GTK_WIDGET (panel_applet), GDK_SCROLL_MASK);
  g_signal_connect (panel_applet, "scroll-event", G_CALLBACK (on_scroll), applet);
  g_signal_connect (panel_applet, "button-press-event", G_CALLBACK (on_button_press), applet);
  setup_menu (applet);

  g_object_set_data_full (G_OBJECT (panel_applet), "caminito-sound-applet", applet,
                          applet_free);
  applet->media_keys = settings_new (MEDIA_KEYS_SCHEMA);
  applet->sound = settings_new (SOUND_SCHEMA);
  applet->popup = caminito_applet_popup_new (GTK_WIDGET (panel_applet));
  applet->content = sd_popup_new (applet->popup);
  applet->pulse = sd_pulse_new (on_changed, applet);
  gtk_widget_show_all (GTK_WIDGET (panel_applet));
  /* No sound server until libpulse answers. */
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

MATE_PANEL_APPLET_IN_PROCESS_FACTORY (FACTORY_ID, PANEL_TYPE_APPLET, "Sound", factory, NULL)
