/*
 * The Sound applet (CMN-213, ADR 0035): an in-process mate-panel applet whose icon shows the
 * Default output, the speaker by its level or headphones, crossed out when muted and struck
 * through with no sound server (the strike is the applet's own where themes have none), with
 * a red dot while an application records. It reads the sound server through libpulse
 * (sd-pulse.c). The wheel on it sets the Default output's volume and middle-click mutes it;
 * left-click toggles the popup (sd-popup.c), with the output and input rows (CMN-215) and the
 * Applications (CMN-216). Its
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
  /* The volume the last step asked for, and when: the next step goes on from it while
     the server hasn't answered, rather than from the volume it last reported. */
  pa_volume_t          asked;
  gint64               asked_at;
  GtkToggleAction     *mute_output;
  GtkToggleAction     *mute_input;
  GtkAction           *settings;
  /* The profiles' items, in the panel's menu before "Sound settings…", which the applet
     holds a reference to, and the cards they were made from. */
  GPtrArray           *profile_items;
  SdCard              *cards;
  guint                n_cards;
  /* The menu is being set to the status, not changed by the user. */
  gboolean             updating;
} Applet;

static const char menu_xml[] =
  "<menuitem name='MuteOutput' action='MuteOutput'/>"
  "<menuitem name='MuteInput' action='MuteInput'/>"
  "<separator/>"
  "<menuitem name='Settings' action='Settings'/>";

/* The item stays as the server has it: an update checks the profile once the server has
   switched to it. */
static void
on_profile_toggled (GtkCheckMenuItem *item, gpointer data)
{
  Applet *applet = data;
  gboolean active = gtk_check_menu_item_get_active (item);

  if (applet->updating)
    return;
  applet->updating = TRUE;
  gtk_check_menu_item_set_active (item, !active);
  applet->updating = FALSE;
  if (active)
    sd_pulse_set_profile (applet->pulse,
                          GPOINTER_TO_UINT (g_object_get_data (G_OBJECT (item), "card")),
                          g_object_get_data (G_OBJECT (item), "profile"));
}

static void
add_profile_item (Applet *applet, GtkMenuShell *menu, int *position, GtkWidget *item)
{
  gtk_menu_shell_insert (menu, item, (*position)++);
  gtk_widget_show (item);
  g_ptr_array_add (applet->profile_items, g_object_ref (item));
}

/* The items may be gone with the menu already, when the applet goes: the reference keeps
   them until then. */
static void
profile_item_free (gpointer item)
{
  gtk_widget_destroy (item);
  g_object_unref (item);
}

static void
cards_clear (Applet *applet)
{
  for (guint i = 0; i < applet->n_cards; i++)
    sd_card_clear (&applet->cards[i]);
  g_clear_pointer (&applet->cards, g_free);
  applet->n_cards = 0;
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

  if (sd_cards_equal (applet->cards, applet->n_cards, status->cards, status->n_cards))
    return;
  G_GNUC_BEGIN_IGNORE_DEPRECATIONS
  proxies = gtk_action_get_proxies (applet->settings);
  G_GNUC_END_IGNORE_DEPRECATIONS
  /* Not built yet: the next update tries again. */
  if (proxies == NULL)
    return;
  cards_clear (applet);
  applet->n_cards = status->n_cards;
  applet->cards = g_new0 (SdCard, status->n_cards);
  for (guint i = 0; i < status->n_cards; i++)
    sd_card_copy (&status->cards[i], &applet->cards[i]);
  g_ptr_array_set_size (applet->profile_items, 0);
  settings_item = proxies->data;
  menu = GTK_MENU_SHELL (gtk_widget_get_parent (settings_item));
  children = gtk_container_get_children (GTK_CONTAINER (menu));
  position = g_list_index (children, settings_item);
  g_list_free (children);
  for (guint i = 0; i < status->n_cards; i++)
    {
      const SdCard *card = &status->cards[i];
      GtkWidget *heading = gtk_menu_item_new_with_label (card->description);

      gtk_widget_set_sensitive (heading, FALSE);
      add_profile_item (applet, menu, &position, heading);
      for (guint j = 0; j < card->n_profiles; j++)
        {
          /* Drawn as radio items, but not a GtkRadioMenuItem's group, which would always
             check one: a card may have no active profile. */
          GtkWidget *item = gtk_check_menu_item_new_with_label (card->profiles[j].description);

          gtk_check_menu_item_set_draw_as_radio (GTK_CHECK_MENU_ITEM (item), TRUE);
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

static gboolean has_marks (const SdStatus *status);

static void
update (Applet *applet)
{
  const SdStatus *status = sd_pulse_status (applet->pulse);
  g_autofree char *tooltip = sd_state_tooltip (status);

  caminito_applet_icon_set (applet->icon, sd_state_icon_names (status));
  /* The overlay draws it with its marks. */
  gtk_widget_set_opacity (applet->icon, has_marks (status) ? 0 : 1);
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

/* The icon as the GtkImage shows it, for the marks to cut into. NULL where the theme has
   none of its names. */
static cairo_surface_t *
icon_surface (Applet *applet, int size)
{
  GtkIconTheme *theme = gtk_icon_theme_get_for_screen (gtk_widget_get_screen (applet->icon));
  int scale = gtk_widget_get_scale_factor (applet->icon);
  GIcon *gicon = NULL;
  g_autoptr (GtkIconInfo) info = NULL;
  g_autoptr (GdkPixbuf) pixbuf = NULL;

  gtk_image_get_gicon (GTK_IMAGE (applet->icon), &gicon, NULL);
  if (gicon == NULL)
    return NULL;
  info = gtk_icon_theme_lookup_by_gicon_for_scale (theme, gicon, size, scale,
                                                   GTK_ICON_LOOKUP_FORCE_SIZE);
  if (info == NULL)
    return NULL;
  /* A symbolic icon in the panel's foreground colour; any other as it is. */
  pixbuf = gtk_icon_info_load_symbolic_for_context (info,
                                                    gtk_widget_get_style_context (applet->icon),
                                                    NULL, NULL);
  if (pixbuf == NULL)
    return NULL;
  return gdk_cairo_surface_create_from_pixbuf (pixbuf, scale,
                                               gtk_widget_get_window (applet->icon));
}

static gboolean
has_marks (const SdStatus *status)
{
  return sd_state_icon_struck (status) || (status->running && status->recording != NULL);
}

/* With a mark, the overlay draws the icon itself, and the GtkImage under it is hidden: a gap
   cut out of the icon around each mark keeps it apart from the icon's lines, over whatever
   the panel's background is. */
static gboolean
on_draw_marks (GtkWidget *marks, cairo_t *cr, gpointer data)
{
  Applet *applet = data;
  const SdStatus *status = sd_pulse_status (applet->pulse);
  GtkStyleContext *style = gtk_widget_get_style_context (marks);
  int size = gtk_image_get_pixel_size (GTK_IMAGE (applet->icon));
  double x = (gtk_widget_get_allocated_width (marks) - size) / 2.0;
  double y = (gtk_widget_get_allocated_height (marks) - size) / 2.0;
  double radius = size / 7.0;
  gboolean struck = sd_state_icon_struck (status);
  gboolean recording = status->running && status->recording != NULL;
  cairo_surface_t *icon;
  GdkRGBA ink;

  if (!has_marks (status))
    return FALSE;
  gtk_style_context_get_color (style, gtk_style_context_get_state (style), &ink);
  cairo_push_group (cr);
  icon = icon_surface (applet, size);
  if (icon != NULL)
    {
      cairo_set_source_surface (cr, icon, x, y);
      cairo_paint (cr);
      cairo_surface_destroy (icon);
    }
  /* The gaps. */
  cairo_set_operator (cr, CAIRO_OPERATOR_CLEAR);
  cairo_set_line_cap (cr, CAIRO_LINE_CAP_ROUND);
  /* Corner to corner, as audio-volume-muted's own cross runs. */
  cairo_move_to (cr, x + size * 0.15, y + size * 0.15);
  cairo_line_to (cr, x + size * 0.85, y + size * 0.85);
  if (struck)
    {
      cairo_set_line_width (cr, size / 6.0);
      cairo_stroke_preserve (cr);
    }
  cairo_new_sub_path (cr);
  /* In the top right-hand corner. */
  cairo_arc (cr, x + size - radius, y + radius, radius + 1, 0, 2 * G_PI);
  if (recording)
    cairo_fill (cr);
  cairo_new_path (cr);
  /* The marks. */
  cairo_set_operator (cr, CAIRO_OPERATOR_OVER);
  if (struck)
    {
      cairo_move_to (cr, x + size * 0.15, y + size * 0.15);
      cairo_line_to (cr, x + size * 0.85, y + size * 0.85);
      gdk_cairo_set_source_rgba (cr, &ink);
      cairo_set_line_width (cr, size / 12.0);
      cairo_stroke (cr);
    }
  if (recording)
    {
      cairo_arc (cr, x + size - radius, y + radius, radius, 0, 2 * G_PI);
      cairo_set_source_rgb (cr, 0.88, 0.11, 0.14);
      cairo_fill (cr);
    }
  cairo_pop_group_to_source (cr);
  cairo_paint (cr);
  return FALSE;
}

static guint volume_step (Applet *applet);
static gboolean above_100 (Applet *applet);
static void on_settings (GtkAction *action, gpointer data);

static void
on_popup_set_volume (SdSide side, pa_volume_t volume, gpointer data)
{
  Applet *applet = data;

  if (side == SD_OUTPUT)
    sd_pulse_set_volume (applet->pulse, volume);
  else
    sd_pulse_set_input_volume (applet->pulse, volume);
}

static void
on_popup_set_muted (SdSide side, gboolean muted, gpointer data)
{
  Applet *applet = data;

  if (side == SD_OUTPUT)
    sd_pulse_set_muted (applet->pulse, muted);
  else
    sd_pulse_set_input_muted (applet->pulse, muted);
}

static void
on_popup_set_default (SdSide side, const char *device, gpointer data)
{
  Applet *applet = data;

  sd_pulse_set_default (applet->pulse, side, device);
}

static void
on_popup_set_port (SdSide side, const char *device, const char *port, gpointer data)
{
  Applet *applet = data;

  sd_pulse_set_port (applet->pulse, side, device, port);
}

static void
on_popup_open_settings (gpointer data G_GNUC_UNUSED)
{
  on_settings (NULL, NULL);
}

static void
on_popup_set_app_volume (SdSide side, const char *app, pa_volume_t volume, gpointer data)
{
  Applet *applet = data;

  sd_pulse_set_app_volume (applet->pulse, side, app, volume);
}

static void
on_popup_set_stream_volume (SdSide side, guint32 stream, pa_volume_t volume, gpointer data)
{
  Applet *applet = data;

  sd_pulse_set_stream_volume (applet->pulse, side, stream, volume);
}

static void
on_popup_move_app (SdSide side, const char *app, const char *device, gpointer data)
{
  Applet *applet = data;

  sd_pulse_move_app (applet->pulse, side, app, device);
}

static void
on_popup_set_system_volume (pa_volume_t volume, gpointer data)
{
  Applet *applet = data;

  sd_pulse_set_system_volume (applet->pulse, volume);
}

static const SdPopupActions popup_actions = {
  .set_volume = on_popup_set_volume,
  .set_muted = on_popup_set_muted,
  .set_default = on_popup_set_default,
  .set_port = on_popup_set_port,
  .open_settings = on_popup_open_settings,
  .set_app_volume = on_popup_set_app_volume,
  .set_stream_volume = on_popup_set_stream_volume,
  .move_app = on_popup_move_app,
  .set_system_volume = on_popup_set_system_volume,
};

static void
on_volume_settings_changed (GSettings *settings G_GNUC_UNUSED, const char *key G_GNUC_UNUSED,
                            gpointer data)
{
  Applet *applet = data;

  sd_popup_set_volume_settings (applet->content, volume_step (applet), above_100 (applet));
}

static void
on_input_level (double level, gpointer data)
{
  Applet *applet = data;

  sd_popup_set_input_level (applet->content, level);
}

static void
on_app_level (SdSide side, const char *app, double level, gpointer data)
{
  Applet *applet = data;

  sd_popup_set_app_level (applet->content, side, app, level);
}

/* The Default input and the applications are heard only while the popup shows their bars
   (CMN-210). */
static void
on_popup_shown (GtkWidget *window G_GNUC_UNUSED, gpointer data)
{
  Applet *applet = data;

  sd_pulse_watch_input_level (applet->pulse, on_input_level, applet);
  sd_pulse_watch_app_levels (applet->pulse, on_app_level, applet);
}

static void
on_popup_hidden (GtkWidget *window G_GNUC_UNUSED, gpointer data)
{
  Applet *applet = data;

  sd_pulse_watch_input_level (applet->pulse, NULL, NULL);
  sd_pulse_watch_app_levels (applet->pulse, NULL, NULL);
}

static SdRect
rect_of (const GdkRectangle *rectangle)
{
  return (SdRect) { rectangle->x, rectangle->y, rectangle->width, rectangle->height };
}

/* The room the popup has where it opens against the applet, before each opening: the
   panel may have moved, or the work area changed. */
static void
update_room (Applet *applet)
{
  GtkWidget *widget = GTK_WIDGET (applet->applet);
  GtkWidget *toplevel = gtk_widget_get_toplevel (widget);
  GdkWindow *window = gtk_widget_get_window (toplevel);
  GdkDisplay *display;
  GdkMonitor *monitor;
  GdkRectangle geometry, workarea;
  MatePanelAppletOrient orient = mate_panel_applet_get_orient (applet->applet);
  SdPlace place = { 0 };
  int width, height;

  if (window == NULL)
    return;
  display = gdk_window_get_display (window);
  monitor = gdk_display_get_monitor_at_window (display, window);
  if (monitor == NULL)
    return;
  gdk_monitor_get_geometry (monitor, &geometry);
  gdk_monitor_get_workarea (monitor, &workarea);
  place.geometry = rect_of (&geometry);
  place.workarea = rect_of (&workarea);
  width = gdk_window_get_width (window);
  height = gdk_window_get_height (window);
  /* X11 places the panel's window in root coordinates; a Wayland client can't learn where
     its layer surface is. */
  place.panel_known = g_str_equal (G_OBJECT_TYPE_NAME (display), "GdkX11Display");
  if (place.panel_known)
    {
      gdk_window_get_origin (window, &place.panel.x, &place.panel.y);
      place.panel.width = width;
      place.panel.height = height;
    }
  place.opens_up = orient == MATE_PANEL_APPLET_ORIENT_UP;
  place.sideways = orient == MATE_PANEL_APPLET_ORIENT_LEFT
                   || orient == MATE_PANEL_APPLET_ORIENT_RIGHT;
  place.panel_thickness = place.sideways ? width : height;
  sd_popup_set_room (applet->content, sd_state_popup_room (&place));
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

static gboolean
above_100 (Applet *applet)
{
  return applet->sound != NULL
         && g_settings_get_boolean (applet->sound, "allow-volume-above-100-percent");
}

static void
scroll_step (Applet *applet, gboolean up)
{
  const SdStatus *status = sd_pulse_status (applet->pulse);

  gint64 now = g_get_monotonic_time ();
  /* Long enough for the server's answer to a step, short enough that a change made
     elsewhere since is the next step's start. */
  gboolean answered = now - applet->asked_at > G_USEC_PER_SEC;

  if (!status->has_output)
    return;
  applet->asked = sd_state_scroll_volume (answered ? status->volume : applet->asked,
                                          volume_step (applet), up, above_100 (applet));
  applet->asked_at = now;
  sd_pulse_set_volume (applet->pulse, applet->asked);
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

  /* Ahead of the popup's own handler, which opens it. */
  if (event->button == GDK_BUTTON_PRIMARY && event->type == GDK_BUTTON_PRESS)
    update_room (applet);
  if (event->button != GDK_BUTTON_MIDDLE)
    return FALSE;
  /* A double click's second press, too, which would start a move. */
  if (event->type == GDK_BUTTON_PRESS && status->has_output)
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

  /* The popup's window goes with the applet, after this: the applet's data is set first,
     and GLib frees an object's data in order. */
  g_signal_handlers_disconnect_by_data (caminito_applet_popup_get_window (applet->popup), applet);
  sd_popup_free (applet->content);
  sd_pulse_free (applet->pulse);
  g_clear_object (&applet->media_keys);
  g_clear_object (&applet->sound);
  g_ptr_array_unref (applet->profile_items);
  cards_clear (applet);
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
  applet->profile_items = g_ptr_array_new_with_free_func (profile_item_free);
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

  /* Smooth too: a touchpad on Wayland gives nothing else. With it, a wheel's clicks come as
     smooth steps of 1, which on_scroll adds up the same way. */
  gtk_widget_add_events (GTK_WIDGET (panel_applet), GDK_SCROLL_MASK | GDK_SMOOTH_SCROLL_MASK);
  g_signal_connect (panel_applet, "scroll-event", G_CALLBACK (on_scroll), applet);
  g_signal_connect (panel_applet, "button-press-event", G_CALLBACK (on_button_press), applet);
  setup_menu (applet);

  g_object_set_data_full (G_OBJECT (panel_applet), "caminito-sound-applet", applet,
                          applet_free);
  applet->media_keys = settings_new (MEDIA_KEYS_SCHEMA);
  applet->sound = settings_new (SOUND_SCHEMA);
  applet->popup = caminito_applet_popup_new (GTK_WIDGET (panel_applet));
  applet->content = sd_popup_new (applet->popup, &popup_actions, applet);
  sd_popup_set_volume_settings (applet->content, volume_step (applet), above_100 (applet));
  if (applet->media_keys != NULL)
    g_signal_connect (applet->media_keys, "changed::volume-step",
                      G_CALLBACK (on_volume_settings_changed), applet);
  if (applet->sound != NULL)
    g_signal_connect (applet->sound, "changed::allow-volume-above-100-percent",
                      G_CALLBACK (on_volume_settings_changed), applet);
  g_signal_connect (caminito_applet_popup_get_window (applet->popup), "show",
                    G_CALLBACK (on_popup_shown), applet);
  g_signal_connect (caminito_applet_popup_get_window (applet->popup), "hide",
                    G_CALLBACK (on_popup_hidden), applet);
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
