/*
 * The Sound applet's popup (CMN-213, CMN-215, CMN-216, CMN-217). See sd-popup.h.
 *
 * The rows are built once and updated in place, and a device list is built again only when
 * its devices change, in the same box: a new viewport would lose the scroll position, and a
 * widget built again loses the focus (CMN-210). An application's row lives as long as the
 * application plays or records, or is pinned, and is moved into its place rather than built
 * again, so that an open row stays open: into Pinned applications when it's pinned, and back.
 *
 * The Undo strip that an unpin leaves is the popup's own: it shows at once, in the place of
 * the rows it stands for, which are hidden until the status comes without their pins, and
 * goes after UNDO_MS.
 *
 * The wheel over a slider scrolls the popup; only Shift+wheel changes the slider. GtkRange's
 * class handler of scroll-event takes every wheel event, so the scrolled window would never
 * see one that starts over a slider: a handler of ours, which runs before it, stops the
 * emission and returns FALSE for a plain wheel, and GTK propagates the event to the
 * scrolled window (gtkwidget.c:2270-2277, gtkmain.c:2567-2606). With Shift, it moves the
 * slider by volume-step a wheel click (CMN-210):
 *
 * - Only smooth events reach the slider, since GDK drops an emulated discrete one for a
 *   window that selects smooth scrolling (gdkwindow.c:9626-9631). A discrete event that does
 *   arrive comes from a device with no smooth stream, and is a whole click.
 * - A click's smooth delta is 1.5 on GDK's Wayland backend under Caminito (libinput's 15 a
 *   click over GDK's 10, gdkdevice-wayland.c:1924) and 1.0 on X11.
 * - A Wheel glide (ADR 0039) or a hi-res wheel sends a click in shares, and the slider moves
 *   by each, unrounded: the volume is rounded where it's sent.
 * - A touchpad's stream has no clicks to count: Shift over a slider leaves the volume alone.
 */

#include "sd-popup.h"

#ifdef GDK_WINDOWING_WAYLAND
#include <gdk/gdkwayland.h>
#endif

/* The prototype's popup's width. */
#define WIDTH     372
#define REVEAL_MS 140
/* The popup window's 1px border, top and bottom (caminito-applet-popup.c). */
#define BORDER    2
/* How long the server's volume waits after the user moves a slider: long enough for the
   server's answers to the moves, short enough that a change made elsewhere since shows
   soon, as the panel icon's wheel waits (CMN-213). */
#define HOLD_MS   1000
/* An application's name, as wide in every row up to a limit, so that the sliders line up
   (CMN-210). */
#define NAME_WIDTH 92
#define NAME_CHARS 14
#define ICON_SIZE  22
/* How long the Undo strip stays after an unpin. */
#define UNDO_MS    5000

static const char *const side_names[] = { "output", "input" };

typedef struct _Slider Slider;

/* A volume slider: a Default device's, an application's, one of its streams' or System
   sounds'. */
struct _Slider
{
  SdPopup   *popup;
  GtkWidget *scale;
  /* "45%", NULL where the row has none. */
  GtkWidget *percent;
  gulong     value_changed;
  /* A button is held on the slider, or the user moved it a moment ago: the server's volume
     waits, since its answers to the earlier moves may come after the later ones. */
  gboolean   dragging;
  guint      hold_id;
  /* The volume as the server last gave it, in percent. */
  guint      server_percent;
  /* Asks for the volume the user moved it to, for owner. */
  void     (*set) (Slider *slider, pa_volume_t volume);
  gpointer   owner;
  /* An application's stream's, by its SdStreamVolume's index. */
  guint32    stream;
};

typedef struct
{
  SdPopup   *popup;
  SdSide     side;
  GtkWidget *row;
  GtkWidget *mute;
  GtkWidget *mute_image;
  /* What the mute button shows. */
  gboolean   muted;
  gboolean   struck;
  Slider    *slider;
  GtkWidget *pill;
  GtkWidget *pill_label;
  GtkWidget *revealer;
  GtkWidget *list;
  /* What the list was built from, to build it again only when that changes. */
  char      *list_key;
  GtkWidget *ports;
  GtkWidget *ports_button;
  GtkWidget *ports_label;
  GtkWidget *ports_revealer;
  GtkWidget *ports_list;
  char      *ports_key;
  /* The Default device's name and active port, NULL with none. */
  char      *device;
  char      *active_port;
  /* The input's meter, and the level it shows from its next frame. */
  GtkWidget *meter;
  double     level;
} Side;

/* A Sound application's row, which lives as long as its box. */
typedef struct
{
  SdPopup   *popup;
  SdSide     side;
  char      *id;
  /* Its key in the popup's rows. */
  char      *key;
  GtkWidget *box;
  GtkWidget *icon;
  GtkWidget *name;
  GtkWidget *name_label;
  Slider    *slider;
  GtkWidget *bar;
  /* The level the bar shows from its next frame. */
  double     level;
  GtkWidget *badge;
  /* The badge and what a pinned application is doing, under the bar. */
  GtkWidget *line;
  GtkWidget *state;
  GtkWidget *revealer;
  GtkWidget *devices;
  char      *devices_key;
  /* "Follow the default", which a pinned application's list hasn't. */
  GtkWidget *follow;
  GtkWidget *streams;
  char      *streams_key;
  /* The device it's moved or pinned to, NULL while it follows the default. */
  char      *device;
  /* Pinned (CMN-217): the × and the strip it opens, rather than the pin button. */
  gboolean   pinned;
  GtkWidget *right;
  GtkWidget *pin;
  GtkWidget *unpin;
  GtkWidget *confirm;
  GtkWidget *confirm_label;
} AppRow;

struct _SdPopup
{
  CaminitoAppletPopup  *applet_popup;
  const SdPopupActions *actions;
  gpointer              data;
  GtkWidget            *content;
  GtkWidget            *scrolled;
  GtkWidget            *rows;
  GtkWidget            *no_server;
  GtkWidget            *footer;
  Side                  sides[2];
  /* Applications: System sounds, then the rows of those playing, the Recording heading and
     the rows of those recording. */
  GtkWidget            *apps;
  Slider               *system;
  GtkWidget            *app_boxes[2];
  GtkWidget            *recording_heading;
  /* AppRow, by "<side>\x1f<id>", pinned or not. */
  GHashTable           *app_rows;
  /* Pinned applications (CMN-217): the section, its heading's × with "Unpin all" beside it,
     the strip the × opens, and the box of the rows with the Undo strip among them. */
  GtkWidget            *pinned;
  GtkWidget            *unpin_all;
  GtkWidget            *unpin_all_label;
  GtkWidget            *unpin_all_confirm;
  GtkWidget            *unpin_all_text;
  GtkWidget            *pinned_box;
  GtkWidget            *undo;
  GtkWidget            *undo_label;
  /* While the Undo strip shows: its timeout, its place among the rows, and the keys of the
     rows it stands for, which stay hidden while they're there. */
  guint                 undo_id;
  guint                 undo_position;
  GHashTable           *undo_hidden;
  gboolean              has_server;
  /* The rows' names, as wide as the widest, so that their sliders line up. */
  GtkSizeGroup         *names;
  /* Every Slider, for the step and the cap. */
  GPtrArray            *sliders;
  guint                 step;
  gboolean              above_100;
  /* The most it may take, or 0 while it isn't known. */
  int                   room;
  guint                 fit_idle;
};

static const char css[] =
  /* Menta's family draws buttons with a border-image, which takes no radius (CMN-210). */
  ".sd-round, .sd-pill { border-image: none; background-image: none; box-shadow: none;"
  "  -gtk-icon-style: symbolic; text-shadow: none;"
  "  background-color: alpha(currentColor, 0.07); border: 1px solid alpha(currentColor, 0.15); }"
  ".sd-round:hover, .sd-pill:hover { background-color: alpha(currentColor, 0.14); }"
  ".sd-round { border-radius: 999px; min-width: 30px; min-height: 30px; padding: 0;"
  "  border-color: transparent; }"
  ".sd-pill { border-radius: 999px; padding: 2px 8px 2px 10px; font-size: 12px; }"
  ".sd-drop { background-color: alpha(currentColor, 0.05); border-radius: 8px; }"
  ".sd-drop button { -gtk-icon-style: symbolic; }"
  ".sd-detail { font-size: 11px; color: alpha(currentColor, 0.6); }"
  ".sd-link { color: @theme_selected_bg_color; font-size: 12px; }"
  ".sd-footer { border-top: 1px solid alpha(currentColor, 0.15); }"
  ".sd-heading { font-size: 11px; font-weight: bold; color: alpha(currentColor, 0.6); }"
  ".sd-section { border-top: 1px solid alpha(currentColor, 0.15); }"
  ".sd-name { font-weight: 500; padding: 0 2px; min-height: 0; }"
  ".sd-pct { font-size: 12px; color: alpha(currentColor, 0.6); }"
  ".sd-x { min-width: 24px; min-height: 24px; padding: 0; -gtk-icon-style: symbolic; }"
  /* Unpin all's ×, and the word beside it (CMN-217). */
  ".sd-danger, .sd-danger label { color: #e01b24; }"
  /* A pinned application's device while it's gone. */
  ".sd-gone { opacity: 0.55; }"
  ".sd-strip { background-color: alpha(currentColor, 0.06); border-radius: 6px; padding: 6px 8px; }"
  ".sd-strip label { font-size: 12px; }"
  /* The selection's colour mixed toward the text's, so that it reads on a dark theme: blue
     in Adwaita and BlueMenta, green in Menta (CMN-210). */
  ".sd-badge { color: mix(@theme_selected_bg_color, @theme_fg_color, 0.3);"
  "  background-color: alpha(@theme_selected_bg_color, 0.18);"
  "  border-radius: 3px; padding: 0 5px; font-size: 11px; }"
  "levelbar.sd-meter trough { min-height: 3px; padding: 0; border: none;"
  "  border-radius: 2px; background-image: none;"
  "  background-color: alpha(currentColor, 0.12); }"
  "levelbar.sd-meter block { min-height: 3px; border: none; border-radius: 2px;"
  "  background-image: none; box-shadow: none; }"
  "levelbar.sd-meter block.filled { background-color: #3aa86b; }"
  "levelbar.sd-meter block.empty { background-color: transparent; }";

static void
add_css (void)
{
  static gboolean added;
  g_autoptr (GtkCssProvider) provider = NULL;

  if (added)
    return;
  added = TRUE;
  provider = gtk_css_provider_new ();
  gtk_css_provider_load_from_data (provider, css, -1, NULL);
  gtk_style_context_add_provider_for_screen (gdk_screen_get_default (),
                                             GTK_STYLE_PROVIDER (provider),
                                             GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
}

static void
add_class (GtkWidget *widget, const char *name)
{
  gtk_style_context_add_class (gtk_widget_get_style_context (widget), name);
}

/* "output-device-headset", from the side and a suffix. */
static void
set_name (GtkWidget *widget, const Side *side, const char *what, const char *id)
{
  g_autofree char *name = id != NULL ? g_strdup_printf ("%s-%s-%s", side_names[side->side],
                                                        what, id)
                                     : g_strdup_printf ("%s-%s", side_names[side->side], what);

  gtk_widget_set_name (widget, name);
}

/* "app-output-mpv", "app-output-mpv-scale"… */
static void
set_app_name (GtkWidget *widget, const AppRow *row, const char *what)
{
  g_autofree char *name = what != NULL ? g_strdup_printf ("app-%s-%s-%s", side_names[row->side],
                                                          row->id, what)
                                       : g_strdup_printf ("app-%s-%s", side_names[row->side],
                                                          row->id);

  gtk_widget_set_name (widget, name);
}

static void
set_icon (GtkWidget *image, const char *const *names)
{
  g_autoptr (GIcon) icon = g_themed_icon_new_from_names ((char **) names, -1);

  gtk_image_set_from_gicon (GTK_IMAGE (image), icon, GTK_ICON_SIZE_MENU);
}

static double
cap (SdPopup *popup)
{
  return popup->above_100 ? 150 : 100;
}

static void
show_percent (Slider *slider)
{
  g_autofree char *text = NULL;

  if (slider->percent == NULL)
    return;
  text = g_strdup_printf ("%.0f%%", gtk_range_get_value (GTK_RANGE (slider->scale)));
  gtk_label_set_text (GTK_LABEL (slider->percent), text);
}

/* The volume a slider shows: the server's, unless the slider is already on it, so that a
   glide's unrounded shares aren't rounded away by the server's answers. */
static void
show_volume (Slider *slider)
{
  GtkRange *range = GTK_RANGE (slider->scale);
  double shown = MIN (slider->server_percent, cap (slider->popup));

  if (slider->dragging || slider->hold_id != 0
      || (guint) (gtk_range_get_value (range) + 0.5) == (guint) shown)
    return;
  g_signal_handler_block (slider->scale, slider->value_changed);
  gtk_range_set_value (range, shown);
  g_signal_handler_unblock (slider->scale, slider->value_changed);
  show_percent (slider);
}

/* The server's volume, which the slider shows unless the user is moving it. */
static void
slider_update (Slider *slider, pa_volume_t volume)
{
  slider->server_percent = sd_state_percent (volume);
  show_volume (slider);
}

static gboolean
on_hold_end (gpointer data)
{
  Slider *slider = data;

  slider->hold_id = 0;
  show_volume (slider);
  return G_SOURCE_REMOVE;
}

static void
on_value_changed (GtkRange *range, gpointer data)
{
  Slider *slider = data;

  g_clear_handle_id (&slider->hold_id, g_source_remove);
  slider->hold_id = g_timeout_add (HOLD_MS, on_hold_end, slider);
  show_percent (slider);
  slider->set (slider, sd_state_volume_of_percent (gtk_range_get_value (range)));
}

/* A wheel click's smooth delta on the event's backend. */
static double
click_unit (GdkEvent *event)
{
#ifdef GDK_WINDOWING_WAYLAND
  if (GDK_IS_WAYLAND_DISPLAY (gdk_window_get_display (event->any.window)))
    return 1.5;
#endif
  return 1.0;
}

static gboolean
on_scale_scroll (GtkWidget *scale, GdkEventScroll *event, gpointer data)
{
  Slider *slider = data;
  GtkRange *range = GTK_RANGE (scale);
  GdkDevice *source = gdk_event_get_source_device ((GdkEvent *) event);
  double clicks = 0;

  if (!(event->state & GDK_SHIFT_MASK))
    {
      /* Not handled, and GtkRange's handler skipped: the scrolled window has it. */
      g_signal_stop_emission_by_name (scale, "scroll-event");
      return FALSE;
    }
  if (event->direction == GDK_SCROLL_SMOOTH)
    {
      double dx, dy;

      if (source != NULL && gdk_device_get_source (source) != GDK_SOURCE_MOUSE)
        return TRUE;
      gdk_event_get_scroll_deltas ((GdkEvent *) event, &dx, &dy);
      /* Shift doesn't turn the wheel horizontal in GTK3, but a mouse's tilt is. A stop, and
         XI2's first event after it enters, carry no delta. */
      clicks = (dy != 0 ? -dy : dx) / click_unit ((GdkEvent *) event);
    }
  else if (!gdk_event_get_pointer_emulated ((GdkEvent *) event))
    switch (event->direction)
      {
      case GDK_SCROLL_UP:
      case GDK_SCROLL_RIGHT:
        clicks = 1;
        break;
      case GDK_SCROLL_DOWN:
      case GDK_SCROLL_LEFT:
        clicks = -1;
        break;
      case GDK_SCROLL_SMOOTH:
      default:
        break;
      }
  if (clicks != 0)
    gtk_range_set_value (range, gtk_range_get_value (range) + clicks * slider->popup->step);
  return TRUE;
}

static gboolean
on_scale_press (GtkWidget *scale G_GNUC_UNUSED, GdkEventButton *event G_GNUC_UNUSED,
                gpointer data)
{
  Slider *slider = data;

  slider->dragging = TRUE;
  return FALSE;
}

static gboolean
on_scale_release (GtkWidget *scale G_GNUC_UNUSED, GdkEventButton *event G_GNUC_UNUSED,
                  gpointer data)
{
  Slider *slider = data;

  slider->dragging = FALSE;
  show_volume (slider);
  return FALSE;
}

static void
on_scale_destroy (GtkWidget *scale G_GNUC_UNUSED, gpointer data)
{
  Slider *slider = data;

  g_clear_handle_id (&slider->hold_id, g_source_remove);
  g_ptr_array_remove_fast (slider->popup->sliders, slider);
  g_free (slider);
}

/* The step and the cap a slider has. */
static void
slider_set_range (Slider *slider)
{
  GtkRange *range = GTK_RANGE (slider->scale);

  /* A narrower range clamps the value, which isn't the user asking for it. */
  g_signal_handler_block (slider->scale, slider->value_changed);
  gtk_range_set_range (range, 0, cap (slider->popup));
  gtk_range_set_increments (range, slider->popup->step, 2.0 * slider->popup->step);
  g_signal_handler_unblock (slider->scale, slider->value_changed);
  show_volume (slider);
  show_percent (slider);
}

/* A slider that asks set for the volume the user moves it to, with its percent beside it
   when with_percent. It's freed with its scale. */
static Slider *
slider_new (SdPopup *popup, void (*set) (Slider *slider, pa_volume_t volume), gpointer owner,
            gboolean with_percent)
{
  Slider *slider = g_new0 (Slider, 1);

  slider->popup = popup;
  slider->set = set;
  slider->owner = owner;
  slider->scale = gtk_scale_new_with_range (GTK_ORIENTATION_HORIZONTAL, 0, 100, 1);
  gtk_scale_set_draw_value (GTK_SCALE (slider->scale), FALSE);
  gtk_widget_set_hexpand (slider->scale, TRUE);
  slider->value_changed = g_signal_connect (slider->scale, "value-changed",
                                            G_CALLBACK (on_value_changed), slider);
  /* Before GtkRange's class handlers, which they run ahead of. */
  g_signal_connect (slider->scale, "scroll-event", G_CALLBACK (on_scale_scroll), slider);
  g_signal_connect (slider->scale, "button-press-event", G_CALLBACK (on_scale_press), slider);
  g_signal_connect (slider->scale, "button-release-event", G_CALLBACK (on_scale_release), slider);
  g_signal_connect (slider->scale, "destroy", G_CALLBACK (on_scale_destroy), slider);
  if (with_percent)
    {
      slider->percent = gtk_label_new (NULL);
      gtk_label_set_xalign (GTK_LABEL (slider->percent), 1);
      gtk_widget_set_size_request (slider->percent, 34, -1);
      add_class (slider->percent, "sd-pct");
    }
  g_ptr_array_add (popup->sliders, slider);
  slider_set_range (slider);
  return slider;
}

static void
side_set_volume (Slider *slider, pa_volume_t volume)
{
  Side *side = slider->owner;

  side->popup->actions->set_volume (side->side, volume, side->popup->data);
}

static void
on_mute_clicked (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  Side *side = data;

  side->popup->actions->set_muted (side->side, !side->muted, side->popup->data);
}

/* Crosses out the mute button's headphones or monitor, as themes have no such icons. */
static gboolean
on_draw_strike (GtkWidget *image, cairo_t *cr, gpointer data)
{
  Side *side = data;
  GtkStyleContext *style = gtk_widget_get_style_context (image);
  double width = gtk_widget_get_allocated_width (image);
  double height = gtk_widget_get_allocated_height (image);
  double size = MIN (width, height);
  double x = (width - size) / 2, y = (height - size) / 2;
  GdkRGBA ink;

  if (!side->struck)
    return FALSE;
  gtk_style_context_get_color (style, gtk_style_context_get_state (style), &ink);
  cairo_set_line_cap (cr, CAIRO_LINE_CAP_ROUND);
  cairo_move_to (cr, x + size * 0.1, y + size * 0.1);
  cairo_line_to (cr, x + size * 0.9, y + size * 0.9);
  gdk_cairo_set_source_rgba (cr, &ink);
  cairo_set_line_width (cr, MAX (1.5, size / 10));
  cairo_stroke (cr);
  return FALSE;
}

/* The pill's list, the port dropdown's ports, or an application's row, open or shut. */
static void
on_toggle_clicked (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  GtkRevealer *revealer = data;

  gtk_revealer_set_reveal_child (revealer, !gtk_revealer_get_reveal_child (revealer));
}

static void
on_device_clicked (GtkButton *button, gpointer data)
{
  Side *side = data;

  side->popup->actions->set_default (side->side, g_object_get_data (G_OBJECT (button), "device"),
                                     side->popup->data);
}

static void
on_port_clicked (GtkButton *button, gpointer data)
{
  Side *side = data;

  if (side->device == NULL)
    return;
  side->popup->actions->set_port (side->side, side->device,
                                  g_object_get_data (G_OBJECT (button), "port"),
                                  side->popup->data);
}

/* A level bar, the input's meter or an application's activity bar, takes the latest level
   once a frame, rather than at every peak (CMN-210). */
static gboolean
on_level_tick (GtkWidget *bar, GdkFrameClock *clock G_GNUC_UNUSED, gpointer data)
{
  double level = *(double *) data;

  if (gtk_level_bar_get_value (GTK_LEVEL_BAR (bar)) != level)
    gtk_level_bar_set_value (GTK_LEVEL_BAR (bar), level);
  return G_SOURCE_CONTINUE;
}

/* A level bar in one green, not the theme's low, high and full colours, showing *level. */
static GtkWidget *
level_bar_new (double *level)
{
  GtkWidget *bar = gtk_level_bar_new_for_interval (0, 1);

  gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (bar), GTK_LEVEL_BAR_OFFSET_LOW);
  gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (bar), GTK_LEVEL_BAR_OFFSET_HIGH);
  gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (bar), GTK_LEVEL_BAR_OFFSET_FULL);
  add_class (bar, "sd-meter");
  gtk_widget_add_tick_callback (bar, on_level_tick, level, NULL);
  return bar;
}

/* A row of a list: the icon, the name with a line under it, and the check, in a flat
   button. */
static GtkWidget *
list_item (const char *const *icon_names, const char *title, const char *detail,
           GtkWidget **check)
{
  GtkWidget *button = gtk_button_new ();
  GtkWidget *grid = gtk_grid_new ();
  GtkWidget *label = gtk_label_new (title);

  gtk_button_set_relief (GTK_BUTTON (button), GTK_RELIEF_NONE);
  gtk_grid_set_column_spacing (GTK_GRID (grid), 8);
  if (icon_names != NULL)
    {
      GtkWidget *icon = gtk_image_new ();

      set_icon (icon, icon_names);
      gtk_grid_attach (GTK_GRID (grid), icon, 0, 0, 1, 2);
    }
  gtk_label_set_xalign (GTK_LABEL (label), 0);
  gtk_label_set_ellipsize (GTK_LABEL (label), PANGO_ELLIPSIZE_END);
  gtk_widget_set_hexpand (label, TRUE);
  gtk_grid_attach (GTK_GRID (grid), label, 1, 0, 1, 1);
  if (detail != NULL)
    {
      GtkWidget *line = gtk_label_new (detail);

      gtk_label_set_xalign (GTK_LABEL (line), 0);
      gtk_label_set_ellipsize (GTK_LABEL (line), PANGO_ELLIPSIZE_END);
      add_class (line, "sd-detail");
      gtk_grid_attach (GTK_GRID (grid), line, 1, 1, 1, 1);
    }
  *check = gtk_image_new_from_icon_name ("object-select-symbolic", GTK_ICON_SIZE_MENU);
  gtk_widget_set_no_show_all (*check, TRUE);
  gtk_widget_set_size_request (*check, 18, -1);
  gtk_grid_attach (GTK_GRID (grid), *check, 2, 0, 1, 2);
  gtk_container_add (GTK_CONTAINER (button), grid);
  gtk_widget_show_all (button);
  return button;
}

static void
clear_box (GtkWidget *box)
{
  g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (box));

  for (GList *l = children; l != NULL; l = l->next)
    gtk_widget_destroy (l->data);
}

/* The list's devices, built again only when one is added, gone or changed; the checks
   follow the default in place, so that the focus stays where it is. */
static void
update_list (Side *side, const SdDevice *devices, guint n_devices)
{
  GString *key = g_string_new (NULL);
  g_autoptr (GList) children = NULL;

  for (guint i = 0; i < n_devices; i++)
    g_string_append_printf (key, "%s\x1f%s\x1f%s\x1f%d\x1e", devices[i].name,
                            devices[i].description, sd_state_device_detail (&devices[i]) ?: "",
                            devices[i].form);
  if (g_strcmp0 (key->str, side->list_key) != 0)
    {
      clear_box (side->list);
      for (guint i = 0; i < n_devices; i++)
        {
          GtkWidget *check;
          GtkWidget *item = list_item (sd_state_device_icon_names (&devices[i]),
                                       devices[i].description,
                                       sd_state_device_detail (&devices[i]), &check);

          set_name (item, side, "device", devices[i].name);
          set_name (check, side, "check", devices[i].name);
          g_object_set_data_full (G_OBJECT (item), "device", g_strdup (devices[i].name), g_free);
          g_object_set_data (G_OBJECT (item), "check", check);
          g_signal_connect (item, "clicked", G_CALLBACK (on_device_clicked), side);
          gtk_box_pack_start (GTK_BOX (side->list), item, FALSE, FALSE, 0);
        }
    }
  g_free (side->list_key);
  side->list_key = g_string_free (key, FALSE);
  children = gtk_container_get_children (GTK_CONTAINER (side->list));
  for (GList *l = children; l != NULL; l = l->next)
    gtk_widget_set_visible (g_object_get_data (l->data, "check"),
                            g_strcmp0 (g_object_get_data (l->data, "device"), side->device) == 0);
}

/* The dropdown of the default's plugged ports, when it has more than one. */
static void
update_ports (Side *side, const SdDevice *device)
{
  GString *key = g_string_new (NULL);
  g_autoptr (GList) children = NULL;
  gboolean shown = device != NULL && device->n_ports > 1;

  gtk_widget_set_visible (side->ports, shown);
  if (shown)
    {
      g_string_append_printf (key, "%s\x1e", device->name);
      for (guint i = 0; i < device->n_ports; i++)
        g_string_append_printf (key, "%s\x1f%s\x1e", device->ports[i].name,
                                device->ports[i].description);
    }
  if (g_strcmp0 (key->str, side->ports_key) != 0)
    {
      clear_box (side->ports_list);
      for (guint i = 0; shown && i < device->n_ports; i++)
        {
          GtkWidget *check;
          GtkWidget *item = list_item (NULL, device->ports[i].description, NULL, &check);

          set_name (item, side, "port", device->ports[i].name);
          set_name (check, side, "port-check", device->ports[i].name);
          g_object_set_data_full (G_OBJECT (item), "port", g_strdup (device->ports[i].name),
                                  g_free);
          g_object_set_data (G_OBJECT (item), "check", check);
          g_signal_connect (item, "clicked", G_CALLBACK (on_port_clicked), side);
          gtk_box_pack_start (GTK_BOX (side->ports_list), item, FALSE, FALSE, 0);
        }
    }
  g_free (side->ports_key);
  side->ports_key = g_string_free (key, FALSE);
  if (!shown)
    return;
  gtk_label_set_text (GTK_LABEL (side->ports_label),
                      device->active_port_description ?: device->active_port ?: "");
  children = gtk_container_get_children (GTK_CONTAINER (side->ports_list));
  for (GList *l = children; l != NULL; l = l->next)
    gtk_widget_set_visible (g_object_get_data (l->data, "check"),
                            g_strcmp0 (g_object_get_data (l->data, "port"), side->active_port)
                              == 0);
}

static void
update_side (Side *side, const SdStatus *status)
{
  const SdDevice *device = sd_status_default_device (status, side->side);
  guint n_devices;
  const SdDevice *devices = sd_status_devices (status, side->side, &n_devices);

  g_free (side->device);
  g_free (side->active_port);
  side->device = device != NULL ? g_strdup (device->name) : NULL;
  side->active_port = device != NULL ? g_strdup (device->active_port) : NULL;
  side->muted = device != NULL && device->muted;
  side->struck = sd_state_mute_struck (side->side, device);
  set_icon (side->mute_image, sd_state_mute_icon_names (side->side, device));
  gtk_widget_queue_draw (side->mute_image);
  gtk_widget_set_sensitive (side->mute, device != NULL);
  gtk_widget_set_sensitive (side->slider->scale, device != NULL);
  slider_update (side->slider, device != NULL ? device->volume : 0);
  if (device != NULL)
    gtk_label_set_text (GTK_LABEL (side->pill_label), device->description);
  else
    gtk_label_set_text (GTK_LABEL (side->pill_label),
                        side->side == SD_OUTPUT ? "No output" : "No input");
  update_list (side, devices, n_devices);
  update_ports (side, device);
  if (side->meter != NULL)
    gtk_widget_set_sensitive (side->meter, device != NULL && !device->muted);
}

/* The output or input row: the round mute button, the slider, and the pill that opens the
   list beneath, with the port dropdown under the list. */
static GtkWidget *
side_new (SdPopup *popup, SdSide which)
{
  Side *side = &popup->sides[which];
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *grid = gtk_grid_new ();
  GtkWidget *middle = gtk_box_new (GTK_ORIENTATION_VERTICAL, 2);
  GtkWidget *pill_box = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 4);
  GtkWidget *drop = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *ports_box = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 6);
  GtkWidget *port_heading = gtk_label_new ("Port");

  side->popup = popup;
  side->side = which;
  side->row = box;
  set_name (box, side, "row", NULL);

  side->mute = gtk_button_new ();
  side->mute_image = gtk_image_new ();
  gtk_container_add (GTK_CONTAINER (side->mute), side->mute_image);
  g_signal_connect_after (side->mute_image, "draw", G_CALLBACK (on_draw_strike), side);
  gtk_widget_set_valign (side->mute, GTK_ALIGN_CENTER);
  gtk_widget_set_tooltip_text (side->mute, which == SD_OUTPUT ? "Mute output" : "Mute microphone");
  add_class (side->mute, "sd-round");
  set_name (side->mute, side, "mute", NULL);
  set_name (side->mute_image, side, "mute-image", NULL);
  g_signal_connect (side->mute, "clicked", G_CALLBACK (on_mute_clicked), side);

  side->slider = slider_new (popup, side_set_volume, side, FALSE);
  set_name (side->slider->scale, side, "scale", NULL);
  gtk_box_pack_start (GTK_BOX (middle), side->slider->scale, FALSE, FALSE, 0);
  if (which == SD_INPUT)
    {
      side->meter = level_bar_new (&side->level);
      g_object_set (side->meter, "margin-start", 8, "margin-end", 8, NULL);
      set_name (side->meter, side, "meter", NULL);
      gtk_box_pack_start (GTK_BOX (middle), side->meter, FALSE, FALSE, 0);
    }
  gtk_widget_set_valign (middle, GTK_ALIGN_CENTER);

  side->pill = gtk_button_new ();
  side->pill_label = gtk_label_new (NULL);
  gtk_label_set_ellipsize (GTK_LABEL (side->pill_label), PANGO_ELLIPSIZE_END);
  gtk_label_set_max_width_chars (GTK_LABEL (side->pill_label), 14);
  gtk_box_pack_start (GTK_BOX (pill_box), side->pill_label, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (pill_box),
                      gtk_image_new_from_icon_name ("pan-down-symbolic", GTK_ICON_SIZE_MENU),
                      FALSE, FALSE, 0);
  gtk_container_add (GTK_CONTAINER (side->pill), pill_box);
  add_class (side->pill, "sd-pill");
  gtk_widget_set_valign (side->pill, GTK_ALIGN_CENTER);
  gtk_widget_set_tooltip_text (side->pill, which == SD_OUTPUT ? "Outputs" : "Inputs");
  set_name (side->pill, side, "pill", NULL);


  gtk_grid_set_column_spacing (GTK_GRID (grid), 8);
  g_object_set (grid, "margin-start", 12, "margin-end", 12, "margin-top", 8, NULL);
  gtk_grid_attach (GTK_GRID (grid), side->mute, 0, 0, 1, 1);
  gtk_grid_attach (GTK_GRID (grid), middle, 1, 0, 1, 1);
  gtk_grid_attach (GTK_GRID (grid), side->pill, 2, 0, 1, 1);
  {
    GList *chain = g_list_append (g_list_append (g_list_append (NULL, side->mute), middle),
                                  side->pill);

    /* Left to right. GTK's own order is by the controls' centres, and the slider's is a
       pixel above the round button's. */
    G_GNUC_BEGIN_IGNORE_DEPRECATIONS
    gtk_container_set_focus_chain (GTK_CONTAINER (grid), chain);
    G_GNUC_END_IGNORE_DEPRECATIONS
    g_list_free (chain);
  }
  gtk_box_pack_start (GTK_BOX (box), grid, FALSE, FALSE, 0);

  side->list = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  set_name (side->list, side, "list", NULL);
  gtk_box_pack_start (GTK_BOX (drop), side->list, FALSE, FALSE, 0);

  side->ports = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  gtk_widget_set_no_show_all (side->ports, TRUE);
  set_name (side->ports, side, "ports", NULL);
  side->ports_button = gtk_button_new ();
  gtk_button_set_relief (GTK_BUTTON (side->ports_button), GTK_RELIEF_NONE);
  set_name (side->ports_button, side, "ports-button", NULL);
  gtk_label_set_xalign (GTK_LABEL (port_heading), 0);
  add_class (port_heading, "sd-detail");
  gtk_box_pack_start (GTK_BOX (ports_box), port_heading, FALSE, FALSE, 0);
  side->ports_label = gtk_label_new (NULL);
  gtk_label_set_xalign (GTK_LABEL (side->ports_label), 0);
  gtk_label_set_ellipsize (GTK_LABEL (side->ports_label), PANGO_ELLIPSIZE_END);
  gtk_widget_set_hexpand (side->ports_label, TRUE);
  gtk_box_pack_start (GTK_BOX (ports_box), side->ports_label, TRUE, TRUE, 0);
  gtk_box_pack_start (GTK_BOX (ports_box),
                      gtk_image_new_from_icon_name ("pan-down-symbolic", GTK_ICON_SIZE_MENU),
                      FALSE, FALSE, 0);
  gtk_container_add (GTK_CONTAINER (side->ports_button), ports_box);

  gtk_box_pack_start (GTK_BOX (side->ports), side->ports_button, FALSE, FALSE, 0);
  side->ports_list = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  g_object_set (side->ports_list, "margin-start", 12, NULL);
  side->ports_revealer = gtk_revealer_new ();
  gtk_revealer_set_transition_duration (GTK_REVEALER (side->ports_revealer), REVEAL_MS);
  g_signal_connect (side->ports_button, "clicked", G_CALLBACK (on_toggle_clicked),
                    side->ports_revealer);
  gtk_container_add (GTK_CONTAINER (side->ports_revealer), side->ports_list);
  gtk_box_pack_start (GTK_BOX (side->ports), side->ports_revealer, FALSE, FALSE, 0);
  gtk_widget_show_all (side->ports_button);
  gtk_widget_show_all (side->ports_revealer);
  gtk_box_pack_start (GTK_BOX (drop), side->ports, FALSE, FALSE, 0);

  add_class (drop, "sd-drop");
  g_object_set (drop, "margin-start", 12, "margin-end", 12, "margin-top", 4, NULL);
  side->revealer = gtk_revealer_new ();
  gtk_revealer_set_transition_duration (GTK_REVEALER (side->revealer), REVEAL_MS);
  g_signal_connect (side->pill, "clicked", G_CALLBACK (on_toggle_clicked), side->revealer);
  gtk_container_add (GTK_CONTAINER (side->revealer), drop);
  gtk_box_pack_start (GTK_BOX (box), side->revealer, FALSE, FALSE, 0);
  return box;
}

static GtkWidget *
heading_new (const char *text)
{
  g_autofree char *upper = g_utf8_strup (text, -1);
  GtkWidget *label = gtk_label_new (upper);

  gtk_label_set_xalign (GTK_LABEL (label), 0);
  add_class (label, "sd-heading");
  g_object_set (label, "margin-start", 2, "margin-top", 6, "margin-bottom", 2, NULL);
  return label;
}

/* A row's grid: the icon, the name, the slider and its percent, and the button at the
   right; under the name, what sub holds. The focus goes from the name to the slider. */
static GtkWidget *
app_grid (SdPopup *popup, GtkWidget *icon, GtkWidget *name, Slider *slider, GtkWidget *right,
          GtkWidget *sub)
{
  GtkWidget *grid = gtk_grid_new ();

  gtk_grid_set_column_spacing (GTK_GRID (grid), 8);
  gtk_grid_set_row_spacing (GTK_GRID (grid), 3);
  g_object_set (grid, "margin-start", 4, "margin-end", 4, "margin-top", 3, "margin-bottom", 3,
                NULL);
  gtk_image_set_pixel_size (GTK_IMAGE (icon), ICON_SIZE);
  gtk_widget_set_valign (icon, GTK_ALIGN_START);
  gtk_grid_attach (GTK_GRID (grid), icon, 0, 0, 1, 1);
  gtk_widget_set_size_request (name, NAME_WIDTH, -1);
  gtk_size_group_add_widget (popup->names, name);
  gtk_grid_attach (GTK_GRID (grid), name, 1, 0, 1, 1);
  gtk_widget_set_valign (slider->scale, GTK_ALIGN_CENTER);
  gtk_grid_attach (GTK_GRID (grid), slider->scale, 2, 0, 1, 1);
  gtk_grid_attach (GTK_GRID (grid), slider->percent, 3, 0, 1, 1);
  gtk_grid_attach (GTK_GRID (grid), right, 4, 0, 1, 1);
  if (sub != NULL)
    gtk_grid_attach (GTK_GRID (grid), sub, 1, 1, 3, 1);
  {
    GList *chain = g_list_append (g_list_append (g_list_append (NULL, name), slider->scale),
                                  right);

    G_GNUC_BEGIN_IGNORE_DEPRECATIONS
    gtk_container_set_focus_chain (GTK_CONTAINER (grid), chain);
    G_GNUC_END_IGNORE_DEPRECATIONS
    g_list_free (chain);
  }
  return grid;
}

/* A row's button at the right: the pin, or a pinned application's ×. */
static GtkWidget *
row_button_new (const char *icon_name)
{
  GtkWidget *button = gtk_button_new_from_icon_name (icon_name, GTK_ICON_SIZE_MENU);

  gtk_button_set_relief (GTK_BUTTON (button), GTK_RELIEF_NONE);
  add_class (button, "sd-x");
  gtk_widget_set_valign (button, GTK_ALIGN_CENTER);
  gtk_widget_set_no_show_all (button, TRUE);
  return button;
}

/* The pin button; in System sounds' row, never shown, it keeps the slider as wide as the
   applications'. */
static GtkWidget *
pin_new (void)
{
  return row_button_new ("view-pin-symbolic");
}

static void
on_close_clicked (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  gtk_revealer_set_reveal_child (GTK_REVEALER (data), FALSE);
}

/* A strip that asks before unpinning, under what it unpins: the question, then Cancel, which
   closes it, and the destructive button, at the end. Shut. */
static GtkWidget *
confirm_new (const char *confirm_text, GtkWidget **label, GtkWidget **cancel, GtkWidget **confirm)
{
  GtkWidget *revealer = gtk_revealer_new ();
  GtkWidget *strip = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  GtkWidget *buttons = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 6);

  *label = gtk_label_new (NULL);
  gtk_label_set_xalign (GTK_LABEL (*label), 0);
  gtk_label_set_line_wrap (GTK_LABEL (*label), TRUE);
  gtk_box_pack_start (GTK_BOX (strip), *label, FALSE, FALSE, 0);
  *cancel = gtk_button_new_with_label ("Cancel");
  *confirm = gtk_button_new_with_label (confirm_text);
  add_class (*confirm, "destructive-action");
  gtk_widget_set_halign (buttons, GTK_ALIGN_END);
  gtk_box_pack_start (GTK_BOX (buttons), *cancel, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (buttons), *confirm, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (strip), buttons, FALSE, FALSE, 0);
  add_class (strip, "sd-strip");
  g_object_set (strip, "margin-start", 4, "margin-end", 4, "margin-bottom", 4, NULL);
  gtk_revealer_set_transition_duration (GTK_REVEALER (revealer), REVEAL_MS);
  gtk_container_add (GTK_CONTAINER (revealer), strip);
  g_signal_connect (*cancel, "clicked", G_CALLBACK (on_close_clicked), revealer);
  gtk_widget_show_all (revealer);
  return revealer;
}

static void
system_set_volume (Slider *slider, pa_volume_t volume)
{
  SdPopup *popup = slider->owner;

  popup->actions->set_system_volume (volume, popup->data);
}

/* A name, left-aligned and cut short past NAME_CHARS. */
static GtkWidget *
name_label_new (const char *text)
{
  GtkWidget *label = gtk_label_new (text);

  gtk_label_set_xalign (GTK_LABEL (label), 0);
  gtk_label_set_ellipsize (GTK_LABEL (label), PANGO_ELLIPSIZE_END);
  gtk_label_set_max_width_chars (GTK_LABEL (label), NAME_CHARS);
  return label;
}

/* System sounds' row: the event role's volume, as pavucontrol's. */
static GtkWidget *
system_row_new (SdPopup *popup)
{
  const char *const icon_names[] = { "preferences-desktop-sound", "audio-x-generic",
                                     "audio-x-generic-symbolic", NULL };
  GtkWidget *icon = gtk_image_new ();
  GtkWidget *name = name_label_new ("System sounds");
  GtkWidget *grid;

  set_icon (icon, icon_names);
  add_class (name, "sd-name");
  popup->system = slider_new (popup, system_set_volume, popup, TRUE);
  gtk_widget_set_name (popup->system->scale, "system-scale");
  grid = app_grid (popup, icon, name, popup->system, pin_new (), NULL);
  gtk_widget_set_name (grid, "system-row");
  return grid;
}

static void
app_set_volume (Slider *slider, pa_volume_t volume)
{
  AppRow *row = slider->owner;

  row->popup->actions->set_app_volume (row->side, row->id, volume, row->popup->data);
}

static void
stream_set_volume (Slider *slider, pa_volume_t volume)
{
  AppRow *row = slider->owner;

  row->popup->actions->set_stream_volume (row->side, slider->stream, volume, row->popup->data);
}

static void
on_app_device_clicked (GtkButton *button, gpointer data)
{
  AppRow *row = data;

  /* NULL from "Follow the default". */
  row->popup->actions->move_app (row->side, row->id,
                                 g_object_get_data (G_OBJECT (button), "device"),
                                 row->popup->data);
}

static void
on_pin_clicked (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  AppRow *row = data;

  row->popup->actions->pin_app (row->side, row->id, row->popup->data);
}

static void arrange_pinned (SdPopup *popup);

/* The Undo strip gone, and the rows it stood for shown where they're still pinned. */
static void
undo_hide (SdPopup *popup)
{
  g_clear_handle_id (&popup->undo_id, g_source_remove);
  g_hash_table_remove_all (popup->undo_hidden);
  arrange_pinned (popup);
}

static gboolean
on_undo_timeout (gpointer data)
{
  SdPopup *popup = data;

  /* Gone with its return. */
  popup->undo_id = 0;
  undo_hide (popup);
  return G_SOURCE_REMOVE;
}

/* The Undo strip, with note, at position among the pinned rows, standing for the rows of
   keys. It replaces the last one: only the last unpin is undone. */
static void
undo_show (SdPopup *popup, const char *note, guint position, GPtrArray *keys)
{
  gtk_label_set_text (GTK_LABEL (popup->undo_label), note);
  g_hash_table_remove_all (popup->undo_hidden);
  for (guint i = 0; i < keys->len; i++)
    g_hash_table_add (popup->undo_hidden, g_strdup (g_ptr_array_index (keys, i)));
  popup->undo_position = position;
  g_clear_handle_id (&popup->undo_id, g_source_remove);
  popup->undo_id = g_timeout_add (UNDO_MS, on_undo_timeout, popup);
  arrange_pinned (popup);
}

/* The pinned rows that show, in order. */
static GPtrArray *
shown_pinned_rows (SdPopup *popup)
{
  g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (popup->pinned_box));
  GPtrArray *rows = g_ptr_array_new ();

  for (GList *l = children; l != NULL; l = l->next)
    {
      AppRow *row = g_object_get_data (l->data, "row");

      if (row != NULL && gtk_widget_get_visible (row->box))
        g_ptr_array_add (rows, row);
    }
  return rows;
}

static void
on_unpin_confirmed (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  AppRow *row = data;
  SdPopup *popup = row->popup;
  SdSide side = row->side;
  g_autofree char *id = g_strdup (row->id);
  g_autofree char *note = g_strdup_printf ("%s unpinned.",
                                           gtk_label_get_text (GTK_LABEL (row->name_label)));
  g_autoptr (GPtrArray) keys = g_ptr_array_new ();
  g_autoptr (GPtrArray) rows = shown_pinned_rows (popup);
  guint position = 0;

  g_ptr_array_find (rows, row, &position);
  gtk_revealer_set_reveal_child (GTK_REVEALER (row->confirm), FALSE);
  g_ptr_array_add (keys, row->key);
  undo_show (popup, note, position, keys);
  /* Last: the status it brings may take the row away. */
  popup->actions->unpin_app (side, id, popup->data);
}

static void
on_app_row_destroy (GtkWidget *box G_GNUC_UNUSED, gpointer data)
{
  AppRow *row = data;

  g_hash_table_remove (row->popup->app_rows, row->key);
  g_free (row->id);
  g_free (row->key);
  g_free (row->devices_key);
  g_free (row->streams_key);
  g_free (row->device);
  g_free (row);
}

static char *
app_key (SdSide side, const char *id)
{
  return g_strdup_printf ("%d\x1f%s", side, id);
}

/* A Sound application's row, empty until it's updated: its icon, its name, which opens what
   it plays to or records from beneath, its slider and percent, and the pin or, pinned, the ×
   with the strip it opens under the row; under them its activity bar, its badge and what a
   pinned application is doing. */
static AppRow *
app_row_new (SdPopup *popup, const SdApp *app)
{
  AppRow *row = g_new0 (AppRow, 1);
  GtkWidget *sub = gtk_box_new (GTK_ORIENTATION_VERTICAL, 3);
  GtkWidget *drop = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *heading = gtk_label_new (app->side == SD_OUTPUT ? "Plays to" : "Records from");
  GtkWidget *cancel, *unpin;

  row->popup = popup;
  row->side = app->side;
  row->id = g_strdup (app->id);
  row->key = app_key (app->side, app->id);
  row->box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  g_object_set_data (G_OBJECT (row->box), "row", row);
  set_app_name (row->box, row, NULL);

  row->pin = pin_new ();
  gtk_widget_set_tooltip_text (row->pin, "Pin to this device");
  set_app_name (row->pin, row, "pin");
  g_signal_connect (row->pin, "clicked", G_CALLBACK (on_pin_clicked), row);
  row->unpin = row_button_new ("window-close-symbolic");
  set_app_name (row->unpin, row, "unpin");
  row->right = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 0);
  gtk_widget_set_halign (row->right, GTK_ALIGN_END);
  gtk_box_pack_start (GTK_BOX (row->right), row->pin, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (row->right), row->unpin, FALSE, FALSE, 0);

  row->icon = gtk_image_new ();
  set_app_name (row->icon, row, "icon");
  row->name = gtk_button_new ();
  row->name_label = name_label_new (app->name);
  gtk_container_add (GTK_CONTAINER (row->name), row->name_label);
  gtk_button_set_relief (GTK_BUTTON (row->name), GTK_RELIEF_NONE);
  add_class (row->name, "sd-name");
  set_app_name (row->name, row, "name");
  row->slider = slider_new (popup, app_set_volume, row, TRUE);
  set_app_name (row->slider->scale, row, "scale");
  set_app_name (row->slider->percent, row, "percent");

  row->bar = level_bar_new (&row->level);
  set_app_name (row->bar, row, "bar");
  gtk_widget_set_no_show_all (row->bar, TRUE);
  gtk_box_pack_start (GTK_BOX (sub), row->bar, FALSE, FALSE, 0);
  row->line = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 6);
  gtk_widget_set_no_show_all (row->line, TRUE);
  row->badge = gtk_label_new (NULL);
  gtk_label_set_ellipsize (GTK_LABEL (row->badge), PANGO_ELLIPSIZE_END);
  gtk_widget_set_halign (row->badge, GTK_ALIGN_START);
  add_class (row->badge, "sd-badge");
  gtk_widget_set_no_show_all (row->badge, TRUE);
  set_app_name (row->badge, row, "badge");
  gtk_box_pack_start (GTK_BOX (row->line), row->badge, FALSE, FALSE, 0);
  row->state = gtk_label_new (NULL);
  add_class (row->state, "sd-detail");
  gtk_widget_set_no_show_all (row->state, TRUE);
  set_app_name (row->state, row, "state");
  gtk_box_pack_start (GTK_BOX (row->line), row->state, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (sub), row->line, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (row->box),
                      app_grid (popup, row->icon, row->name, row->slider, row->right, sub), FALSE,
                      FALSE, 0);

  row->confirm = confirm_new ("Unpin", &row->confirm_label, &cancel, &unpin);
  set_app_name (row->confirm, row, "confirm");
  set_app_name (cancel, row, "cancel");
  set_app_name (unpin, row, "confirm-unpin");
  g_signal_connect (unpin, "clicked", G_CALLBACK (on_unpin_confirmed), row);
  g_signal_connect (row->unpin, "clicked", G_CALLBACK (on_toggle_clicked), row->confirm);
  gtk_box_pack_start (GTK_BOX (row->box), row->confirm, FALSE, FALSE, 0);

  gtk_label_set_xalign (GTK_LABEL (heading), 0);
  add_class (heading, "sd-detail");
  g_object_set (heading, "margin-start", 8, "margin-top", 4, NULL);
  gtk_box_pack_start (GTK_BOX (drop), heading, FALSE, FALSE, 0);
  row->devices = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  gtk_box_pack_start (GTK_BOX (drop), row->devices, FALSE, FALSE, 0);
  row->streams = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  gtk_box_pack_start (GTK_BOX (drop), row->streams, FALSE, FALSE, 0);
  add_class (drop, "sd-drop");
  g_object_set (drop, "margin-start", 4, "margin-end", 4, "margin-bottom", 4, NULL);
  row->revealer = gtk_revealer_new ();
  gtk_revealer_set_transition_duration (GTK_REVEALER (row->revealer), REVEAL_MS);
  gtk_container_add (GTK_CONTAINER (row->revealer), drop);
  gtk_box_pack_start (GTK_BOX (row->box), row->revealer, FALSE, FALSE, 0);
  g_signal_connect (row->name, "clicked", G_CALLBACK (on_toggle_clicked), row->revealer);

  g_signal_connect (row->box, "destroy", G_CALLBACK (on_app_row_destroy), row);
  g_hash_table_insert (popup->app_rows, row->key, row);
  gtk_widget_show_all (row->box);
  return row;
}

/* The row in box at position, moved there from wherever it was, or put there new. */
static void
place_row (AppRow *row, GtkWidget *box, int position)
{
  GtkWidget *parent = gtk_widget_get_parent (row->box);

  if (parent != box)
    {
      g_object_ref (row->box);
      if (parent != NULL)
        gtk_container_remove (GTK_CONTAINER (parent), row->box);
      gtk_box_pack_start (GTK_BOX (box), row->box, FALSE, FALSE, 0);
      g_object_unref (row->box);
    }
  gtk_box_reorder_child (GTK_BOX (box), row->box, position);
}

/* "Follow the default (WH-1000XM4)" and the side's devices, built again only when they
   change; the checks follow the application's device in place. */
static void
update_app_devices (AppRow *row, const SdStatus *status)
{
  const SdDevice *fallback = sd_status_default_device (status, row->side);
  guint n_devices;
  const SdDevice *devices = sd_status_devices (status, row->side, &n_devices);
  g_autofree char *follow = fallback != NULL
                              ? g_strdup_printf ("Follow the default (%s)", fallback->description)
                              : g_strdup ("Follow the default");
  GString *key = g_string_new (follow);
  g_autoptr (GList) children = NULL;

  for (guint i = 0; i < n_devices; i++)
    g_string_append_printf (key, "\x1e%s\x1f%s\x1f%s\x1f%d", devices[i].name,
                            devices[i].description, sd_state_device_detail (&devices[i]) ?: "",
                            devices[i].form);
  if (g_strcmp0 (key->str, row->devices_key) != 0)
    {
      GtkWidget *check;
      GtkWidget *item = list_item (NULL, follow, NULL, &check);

      clear_box (row->devices);
      set_app_name (item, row, "follow");
      set_app_name (check, row, "check-follow");
      g_object_set_data (G_OBJECT (item), "check", check);
      g_signal_connect (item, "clicked", G_CALLBACK (on_app_device_clicked), row);
      gtk_box_pack_start (GTK_BOX (row->devices), item, FALSE, FALSE, 0);
      row->follow = item;
      for (guint i = 0; i < n_devices; i++)
        {
          g_autofree char *item_name = g_strdup_printf ("device-%s", devices[i].name);
          g_autofree char *check_name = g_strdup_printf ("check-%s", devices[i].name);

          item = list_item (sd_state_device_icon_names (&devices[i]), devices[i].description,
                            sd_state_device_detail (&devices[i]), &check);
          set_app_name (item, row, item_name);
          set_app_name (check, row, check_name);
          g_object_set_data_full (G_OBJECT (item), "device", g_strdup (devices[i].name), g_free);
          g_object_set_data (G_OBJECT (item), "check", check);
          g_signal_connect (item, "clicked", G_CALLBACK (on_app_device_clicked), row);
          gtk_box_pack_start (GTK_BOX (row->devices), item, FALSE, FALSE, 0);
        }
    }
  g_free (row->devices_key);
  row->devices_key = g_string_free (key, FALSE);
  /* A pinned application's device is chosen here, and it's unpinned with its ×. */
  gtk_widget_set_visible (row->follow, !row->pinned);
  children = gtk_container_get_children (GTK_CONTAINER (row->devices));
  for (GList *l = children; l != NULL; l = l->next)
    gtk_widget_set_visible (g_object_get_data (l->data, "check"),
                            g_strcmp0 (g_object_get_data (l->data, "device"), row->device) == 0);
}

/* A slider for each stream, when there's more than one, built again only when they change. */
static void
update_app_streams (AppRow *row, const SdApp *app)
{
  GString *key = g_string_new (NULL);
  g_autoptr (GList) children = NULL;

  for (guint i = 0; app->n_streams > 1 && i < app->n_streams; i++)
    g_string_append_printf (key, "%u\x1f%s\x1e", app->streams[i].index,
                            app->streams[i].title ?: "");
  if (g_strcmp0 (key->str, row->streams_key) != 0)
    {
      clear_box (row->streams);
      for (guint i = 0; app->n_streams > 1 && i < app->n_streams; i++)
        {
          GtkWidget *line = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 8);
          GtkWidget *title = name_label_new (app->streams[i].title ?: app->name);
          Slider *slider = slider_new (row->popup, stream_set_volume, row, TRUE);
          g_autofree char *name = g_strdup_printf ("stream-%u", app->streams[i].index);

          slider->stream = app->streams[i].index;
          gtk_widget_set_size_request (title, NAME_WIDTH, -1);
          add_class (title, "sd-detail");
          set_app_name (slider->scale, row, name);
          g_object_set_data (G_OBJECT (line), "slider", slider);
          g_object_set (line, "margin-start", 8, "margin-end", 8, NULL);
          gtk_box_pack_start (GTK_BOX (line), title, FALSE, FALSE, 0);
          gtk_box_pack_start (GTK_BOX (line), slider->scale, TRUE, TRUE, 0);
          gtk_box_pack_start (GTK_BOX (line), slider->percent, FALSE, FALSE, 0);
          gtk_widget_show_all (line);
          gtk_box_pack_start (GTK_BOX (row->streams), line, FALSE, FALSE, 0);
        }
    }
  g_free (row->streams_key);
  row->streams_key = g_string_free (key, FALSE);
  children = gtk_container_get_children (GTK_CONTAINER (row->streams));
  for (GList *l = children; l != NULL; l = l->next)
    {
      Slider *slider = g_object_get_data (l->data, "slider");

      for (guint i = 0; i < app->n_streams; i++)
        if (app->streams[i].index == slider->stream)
          slider_update (slider, app->streams[i].volume);
    }
}


/* What a pinned application is doing, under its name: nothing while it plays. */
static const char *
activity_text (SdPinActivity activity)
{
  switch (activity)
    {
    case SD_PIN_PAUSED:
      return "‖ Paused";
    case SD_PIN_NOT_RUNNING:
      return "■ Not running";
    case SD_PIN_PLAYING:
    default:
      return NULL;
    }
}

/* The row of app, which pinned says is pinned, NULL where it isn't. */
static void
update_app_row (AppRow *row, const SdApp *app, const SdPinnedApp *pinned, const SdStatus *status)
{
  const char *const icon_names[] = { app->icon ?: "application-x-executable",
                                     "application-x-executable", NULL };
  /* With no stream there's nothing to read or set. */
  gboolean running = pinned == NULL || pinned->activity != SD_PIN_NOT_RUNNING;
  const char *activity = pinned != NULL ? activity_text (pinned->activity) : NULL;
  g_autofree char *question = g_strdup_printf ("Unpin %s? It goes back to following the default "
                                               "%s.",
                                               app->name,
                                               app->side == SD_OUTPUT ? "output" : "input");
  gboolean gone = pinned != NULL && pinned->device_gone;
  GtkStyleContext *badge_style = gtk_widget_get_style_context (row->badge);

  row->pinned = pinned != NULL;
  set_icon (row->icon, icon_names);
  gtk_label_set_text (GTK_LABEL (row->name_label), app->name);
  gtk_widget_set_tooltip_text (row->name, app->name);
  slider_update (row->slider, app->volume);
  gtk_widget_set_visible (row->slider->scale, running);
  gtk_widget_set_visible (row->slider->percent, running);
  /* Without the slider, the × still at the right edge. */
  gtk_widget_set_hexpand (row->right, !running);
  gtk_widget_set_visible (row->pin, pinned == NULL);
  gtk_widget_set_visible (row->unpin, pinned != NULL);
  if (pinned == NULL)
    gtk_revealer_set_reveal_child (GTK_REVEALER (row->confirm), FALSE);
  gtk_label_set_text (GTK_LABEL (row->confirm_label), question);
  gtk_widget_set_visible (row->bar, pinned == NULL || pinned->activity == SD_PIN_PLAYING);
  g_free (row->device);
  row->device = g_strdup (app->device);
  /* The server's description, an output's monitor's too, which no list has; while a pinned
     device is gone, the one it had. */
  gtk_widget_set_visible (row->badge, app->device != NULL);
  if (gone)
    {
      g_autofree char *text = g_strdup_printf ("%s · not connected", app->device_description);

      gtk_label_set_text (GTK_LABEL (row->badge), text);
      gtk_style_context_add_class (badge_style, "sd-gone");
    }
  else
    {
      gtk_label_set_text (GTK_LABEL (row->badge), app->device_description ?: "");
      gtk_style_context_remove_class (badge_style, "sd-gone");
    }
  gtk_widget_set_visible (row->state, activity != NULL);
  gtk_label_set_text (GTK_LABEL (row->state), activity ?: "");
  gtk_widget_set_visible (row->line, app->device != NULL || activity != NULL);
  update_app_devices (row, status);
  update_app_streams (row, app);
}

/* The row of app, built where it's new, and its key kept. */
static AppRow *
kept_row (SdPopup *popup, const SdApp *app, GHashTable *kept)
{
  char *key = app_key (app->side, app->id);
  AppRow *row = g_hash_table_lookup (popup->app_rows, key);

  g_hash_table_add (kept, key);
  return row != NULL ? row : app_row_new (popup, app);
}

/* The pinned rows that the Undo strip doesn't stand for, and the strip in its place among
   them; the section while either shows. */
static void
arrange_pinned (SdPopup *popup)
{
  g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (popup->pinned_box));
  gboolean undoing = popup->undo_id != 0;
  guint shown = 0;

  for (GList *l = children; l != NULL; l = l->next)
    {
      AppRow *row = g_object_get_data (l->data, "row");
      gboolean hidden;

      if (row == NULL)
        continue;
      hidden = undoing && g_hash_table_contains (popup->undo_hidden, row->key);
      gtk_widget_set_visible (row->box, !hidden);
      shown += !hidden;
    }
  gtk_widget_set_visible (popup->undo, undoing);
  /* The rows are in order, the hidden ones in their places, so this is the place it stood
     for. */
  if (undoing)
    gtk_box_reorder_child (GTK_BOX (popup->pinned_box), popup->undo, popup->undo_position);
  gtk_widget_set_visible (popup->unpin_all, shown > 0);
  if (shown == 0)
    gtk_revealer_set_reveal_child (GTK_REVEALER (popup->unpin_all_confirm), FALSE);
  gtk_widget_set_visible (popup->pinned, popup->has_server && (shown > 0 || undoing));
}

/* The applications' rows in the status's order, the pinned ones under Pinned applications:
   those gone destroyed, the new ones built, the others updated and moved into their place. */
static void
update_apps (SdPopup *popup, const SdStatus *status)
{
  g_autoptr (GHashTable) kept = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  g_autoptr (GPtrArray) gone = g_ptr_array_new ();
  int positions[2] = { 0, 0 };
  GHashTableIter iter;
  gpointer value;

  gtk_widget_set_sensitive (popup->system->scale, status->has_system_sounds);
  slider_update (popup->system, status->has_system_sounds ? status->system_volume : 0);
  for (guint i = 0; i < status->n_pinned; i++)
    {
      const SdPinnedApp *pinned = &status->pinned[i];
      AppRow *row = kept_row (popup, &pinned->app, kept);

      place_row (row, popup->pinned_box, i);
      update_app_row (row, &pinned->app, pinned, status);
    }
  for (guint i = 0; i < status->n_apps; i++)
    {
      const SdApp *app = &status->apps[i];
      AppRow *row = kept_row (popup, app, kept);

      place_row (row, popup->app_boxes[app->side], positions[app->side]++);
      /* Hidden by an Undo while it was pinned: unpinned, it shows here. */
      gtk_widget_show (row->box);
      update_app_row (row, app, NULL, status);
    }
  g_hash_table_iter_init (&iter, popup->app_rows);
  while (g_hash_table_iter_next (&iter, NULL, &value))
    if (!g_hash_table_contains (kept, ((AppRow *) value)->key))
      g_ptr_array_add (gone, ((AppRow *) value)->box);
  /* Not while iterating: a row leaves the table as it's destroyed. */
  for (guint i = 0; i < gone->len; i++)
    gtk_widget_destroy (g_ptr_array_index (gone, i));
  gtk_widget_set_visible (popup->recording_heading, positions[SD_INPUT] > 0);
  arrange_pinned (popup);
}

/* "Unpin all" beside the heading's ×, while it's hovered or focused. */
static void
on_unpin_all_state (GtkWidget *button, GtkStateFlags previous G_GNUC_UNUSED, gpointer data)
{
  SdPopup *popup = data;
  GtkStateFlags flags = gtk_widget_get_state_flags (button);

  gtk_widget_set_visible (popup->unpin_all_label,
                          (flags & (GTK_STATE_FLAG_PRELIGHT | GTK_STATE_FLAG_FOCUSED)) != 0);
}

/* The heading's × opens its strip, counting the rows as it opens, and again closes it. */
static void
on_unpin_all_clicked (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  SdPopup *popup = data;
  GtkRevealer *revealer = GTK_REVEALER (popup->unpin_all_confirm);
  g_autoptr (GPtrArray) rows = shown_pinned_rows (popup);
  g_autofree char *question = rows->len == 1
    ? g_strdup ("Unpin all 1 application? It goes back to following the default device.")
    : g_strdup_printf ("Unpin all %u applications? They go back to following the default "
                       "devices.",
                       rows->len);

  gtk_label_set_text (GTK_LABEL (popup->unpin_all_text), question);
  gtk_revealer_set_reveal_child (revealer, !gtk_revealer_get_reveal_child (revealer));
}

static void
on_unpin_all_confirmed (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  SdPopup *popup = data;
  g_autoptr (GPtrArray) rows = shown_pinned_rows (popup);
  g_autoptr (GPtrArray) keys = g_ptr_array_new ();
  g_autofree char *note = rows->len == 1 ? g_strdup ("1 application unpinned.")
                                         : g_strdup_printf ("%u applications unpinned.", rows->len);

  for (guint i = 0; i < rows->len; i++)
    g_ptr_array_add (keys, ((AppRow *) g_ptr_array_index (rows, i))->key);
  gtk_revealer_set_reveal_child (GTK_REVEALER (popup->unpin_all_confirm), FALSE);
  undo_show (popup, note, 0, keys);
  popup->actions->unpin_all (popup->data);
}

static void
on_undo_clicked (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  SdPopup *popup = data;

  undo_hide (popup);
  popup->actions->undo_unpin (popup->data);
}

/* Pinned applications: the heading with its red × at the right, lined up with the rows'
   buttons, and "Unpin all" beside it on hover or focus; the strip the × opens; the rows, and
   the Undo strip among them. No tooltip: the word is the × 's. */
static GtkWidget *
pinned_new (SdPopup *popup)
{
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *heading_row = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 0);
  GtkWidget *inside = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 2);
  GtkWidget *icon = gtk_image_new_from_icon_name ("window-close-symbolic", GTK_ICON_SIZE_MENU);
  GtkWidget *cancel, *confirm, *undo_button;

  popup->pinned = box;
  add_class (box, "sd-section");
  g_object_set (box, "margin-start", 8, "margin-end", 8, "margin-top", 10, NULL);
  gtk_widget_set_name (box, "pinned");
  gtk_widget_set_no_show_all (box, TRUE);

  gtk_box_pack_start (GTK_BOX (heading_row), heading_new ("Pinned applications"), TRUE, TRUE, 0);
  popup->unpin_all_label = gtk_label_new ("Unpin all");
  gtk_widget_set_name (popup->unpin_all_label, "unpin-all-label");
  gtk_widget_set_no_show_all (popup->unpin_all_label, TRUE);
  gtk_box_pack_start (GTK_BOX (inside), popup->unpin_all_label, FALSE, FALSE, 0);
  /* As wide as a row's ×, whose icon is centred in its 24 pixels. */
  g_object_set (icon, "margin-start", 4, "margin-end", 4, NULL);
  gtk_widget_set_name (icon, "unpin-all-icon");
  gtk_box_pack_start (GTK_BOX (inside), icon, FALSE, FALSE, 0);
  popup->unpin_all = gtk_button_new ();
  gtk_container_add (GTK_CONTAINER (popup->unpin_all), inside);
  gtk_button_set_relief (GTK_BUTTON (popup->unpin_all), GTK_RELIEF_NONE);
  add_class (popup->unpin_all, "sd-x");
  add_class (popup->unpin_all, "sd-danger");
  gtk_widget_set_valign (popup->unpin_all, GTK_ALIGN_CENTER);
  gtk_widget_set_name (popup->unpin_all, "unpin-all");
  gtk_widget_set_no_show_all (popup->unpin_all, TRUE);
  gtk_widget_show_all (inside);
  gtk_widget_hide (popup->unpin_all_label);
  g_signal_connect (popup->unpin_all, "state-flags-changed", G_CALLBACK (on_unpin_all_state),
                    popup);
  g_signal_connect (popup->unpin_all, "clicked", G_CALLBACK (on_unpin_all_clicked), popup);
  gtk_box_pack_start (GTK_BOX (heading_row), popup->unpin_all, FALSE, FALSE, 0);
  /* The rows' grids' margin. */
  g_object_set (heading_row, "margin-end", 4, NULL);
  gtk_box_pack_start (GTK_BOX (box), heading_row, FALSE, FALSE, 0);

  popup->unpin_all_confirm = confirm_new ("Unpin all", &popup->unpin_all_text, &cancel, &confirm);
  gtk_widget_set_name (popup->unpin_all_confirm, "unpin-all-confirm");
  gtk_widget_set_name (cancel, "unpin-all-cancel");
  gtk_widget_set_name (confirm, "unpin-all-confirm-unpin");
  g_signal_connect (confirm, "clicked", G_CALLBACK (on_unpin_all_confirmed), popup);
  gtk_box_pack_start (GTK_BOX (box), popup->unpin_all_confirm, FALSE, FALSE, 0);

  popup->pinned_box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  gtk_widget_set_name (popup->pinned_box, "pinned-apps");
  popup->undo = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 6);
  gtk_widget_set_name (popup->undo, "undo");
  add_class (popup->undo, "sd-strip");
  g_object_set (popup->undo, "margin-start", 4, "margin-end", 4, "margin-top", 3,
                "margin-bottom", 3, NULL);
  popup->undo_label = gtk_label_new (NULL);
  gtk_label_set_xalign (GTK_LABEL (popup->undo_label), 0);
  gtk_label_set_ellipsize (GTK_LABEL (popup->undo_label), PANGO_ELLIPSIZE_END);
  gtk_box_pack_start (GTK_BOX (popup->undo), popup->undo_label, TRUE, TRUE, 0);
  undo_button = gtk_button_new_with_label ("Undo");
  gtk_widget_set_name (undo_button, "undo-button");
  g_signal_connect (undo_button, "clicked", G_CALLBACK (on_undo_clicked), popup);
  gtk_box_pack_start (GTK_BOX (popup->undo), undo_button, FALSE, FALSE, 0);
  gtk_widget_show_all (popup->undo);
  gtk_widget_set_no_show_all (popup->undo, TRUE);
  gtk_widget_hide (popup->undo);
  gtk_box_pack_start (GTK_BOX (popup->pinned_box), popup->undo, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), popup->pinned_box, FALSE, FALSE, 0);
  gtk_widget_show_all (heading_row);
  gtk_widget_show (popup->unpin_all_confirm);
  gtk_widget_show (popup->pinned_box);
  return box;
}

/* Applications: the heading, System sounds, the boxes of the rows playing and recording,
   and the Recording heading between them. */
static GtkWidget *
apps_new (SdPopup *popup)
{
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);

  add_class (box, "sd-section");
  g_object_set (box, "margin-start", 8, "margin-end", 8, "margin-top", 10, NULL);
  gtk_widget_set_name (box, "applications");
  gtk_box_pack_start (GTK_BOX (box), heading_new ("Applications"), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), system_row_new (popup), FALSE, FALSE, 0);
  for (SdSide side = SD_OUTPUT; side <= SD_INPUT; side++)
    {
      popup->app_boxes[side] = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
      gtk_widget_set_name (popup->app_boxes[side],
                           side == SD_OUTPUT ? "apps-playing" : "apps-recording");
      if (side == SD_INPUT)
        {
          popup->recording_heading = heading_new ("Recording");
          gtk_widget_set_name (popup->recording_heading, "recording-heading");
          gtk_widget_set_no_show_all (popup->recording_heading, TRUE);
          gtk_box_pack_start (GTK_BOX (box), popup->recording_heading, FALSE, FALSE, 0);
        }
      gtk_box_pack_start (GTK_BOX (box), popup->app_boxes[side], FALSE, FALSE, 0);
    }
  return box;
}

/* A shown GtkWindow follows its content's minimum height and never its natural one, and a
   scrolled window's minimum is its scroll bar's: the rows' natural height for their width,
   up to the room, is made the scrolled window's minimum, so that the popup grows with its
   rows, and it's resized to its minimum when that shrinks. This runs after the rows are
   allocated, from an idle, since a resize queued while GTK allocates is lost: the popup
   follows its rows a frame behind (CMN-210). */
static void
fit (SdPopup *popup)
{
  GtkScrolledWindow *scrolled = GTK_SCROLLED_WINDOW (popup->scrolled);
  GtkWidget *toplevel = gtk_widget_get_toplevel (popup->scrolled);
  int minimum = gtk_scrolled_window_get_min_content_height (scrolled);
  int natural, footer, most = G_MAXINT;

  if (popup->room > 0)
    {
      gtk_widget_get_preferred_height (popup->footer, NULL, &footer);
      most = MAX (0, popup->room - footer - BORDER);
    }
  /* GTK wants the minimum under the maximum at every step. */
  if (minimum > most)
    gtk_scrolled_window_set_min_content_height (scrolled, most);
  gtk_scrolled_window_set_max_content_height (scrolled, most == G_MAXINT ? -1 : most);
  gtk_widget_get_preferred_height_for_width (gtk_bin_get_child (GTK_BIN (scrolled)),
                                             gtk_widget_get_allocated_width (popup->scrolled),
                                             NULL, &natural);
  natural = MIN (natural, most);
  if (natural == minimum)
    return;
  gtk_scrolled_window_set_min_content_height (scrolled, natural);
  if (natural < minimum && GTK_IS_WINDOW (toplevel))
    gtk_window_resize (GTK_WINDOW (toplevel), 1, 1);
}

static gboolean
on_fit_idle (gpointer data)
{
  SdPopup *popup = data;

  popup->fit_idle = 0;
  fit (popup);
  return G_SOURCE_REMOVE;
}

static void
on_rows_allocate (GtkWidget *widget G_GNUC_UNUSED, GdkRectangle *allocation G_GNUC_UNUSED,
                  gpointer data)
{
  SdPopup *popup = data;

  if (popup->fit_idle == 0)
    popup->fit_idle = g_idle_add (on_fit_idle, popup);
}

/* Each opening starts the keyboard at the top: GtkWindow focuses the first control as it
   maps, with no ring, so that Tab would go past it, or keeps the last opening's focus. */
static void
on_window_show (GtkWidget *window, gpointer data G_GNUC_UNUSED)
{
  gtk_window_set_focus (GTK_WINDOW (window), NULL);
}

/* Closed, the meter and the activity bars go quiet: the reader stops hearing the Default
   input and the applications. */
static void
on_window_hide (GtkWidget *window G_GNUC_UNUSED, gpointer data)
{
  SdPopup *popup = data;
  GHashTableIter iter;
  gpointer value;

  popup->sides[SD_INPUT].level = 0;
  g_hash_table_iter_init (&iter, popup->app_rows);
  while (g_hash_table_iter_next (&iter, NULL, &value))
    ((AppRow *) value)->level = 0;
}

static void
on_settings_clicked (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  SdPopup *popup = data;

  caminito_applet_popup_hide (popup->applet_popup);
  popup->actions->open_settings (popup->data);
}

SdPopup *
sd_popup_new (CaminitoAppletPopup *applet_popup, const SdPopupActions *actions, gpointer data)
{
  SdPopup *popup = g_new0 (SdPopup, 1);
  GtkWidget *content = popup->content = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *settings = gtk_button_new_with_label ("Sound settings…");
  GtkWidget *window = caminito_applet_popup_get_window (applet_popup);

  add_css ();
  popup->applet_popup = applet_popup;
  popup->actions = actions;
  popup->data = data;
  popup->step = 5;
  popup->sliders = g_ptr_array_new ();
  popup->app_rows = g_hash_table_new (g_str_hash, g_str_equal);
  popup->undo_hidden = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  popup->names = gtk_size_group_new (GTK_SIZE_GROUP_HORIZONTAL);

  gtk_widget_set_size_request (content, WIDTH, -1);
  popup->scrolled = gtk_scrolled_window_new (NULL, NULL);
  gtk_widget_set_name (popup->scrolled, "scrolled");
  gtk_scrolled_window_set_policy (GTK_SCROLLED_WINDOW (popup->scrolled), GTK_POLICY_NEVER,
                                  GTK_POLICY_AUTOMATIC);
  gtk_scrolled_window_set_propagate_natural_height (GTK_SCROLLED_WINDOW (popup->scrolled), TRUE);
  popup->rows = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  g_object_set (popup->rows, "margin-bottom", 8, NULL);
  /* Tab scrolls the focused control into view. */
  gtk_container_set_focus_vadjustment (
    GTK_CONTAINER (popup->rows),
    gtk_scrolled_window_get_vadjustment (GTK_SCROLLED_WINDOW (popup->scrolled)));
  popup->no_server = gtk_label_new ("No sound server");
  g_object_set (popup->no_server, "margin", 12, NULL);
  gtk_box_pack_start (GTK_BOX (popup->rows), popup->no_server, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (popup->rows), side_new (popup, SD_OUTPUT), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (popup->rows), side_new (popup, SD_INPUT), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (popup->rows), pinned_new (popup), FALSE, FALSE, 0);
  popup->apps = apps_new (popup);
  gtk_box_pack_start (GTK_BOX (popup->rows), popup->apps, FALSE, FALSE, 0);
  gtk_container_add (GTK_CONTAINER (popup->scrolled), popup->rows);
  g_signal_connect_after (popup->rows, "size-allocate", G_CALLBACK (on_rows_allocate), popup);
  gtk_box_pack_start (GTK_BOX (content), popup->scrolled, TRUE, TRUE, 0);

  popup->footer = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 0);
  gtk_widget_set_name (popup->footer, "footer");
  add_class (popup->footer, "sd-footer");
  gtk_button_set_relief (GTK_BUTTON (settings), GTK_RELIEF_NONE);
  add_class (settings, "sd-link");
  gtk_widget_set_name (settings, "settings");
  g_object_set (settings, "margin", 4, NULL);
  g_signal_connect (settings, "clicked", G_CALLBACK (on_settings_clicked), popup);
  gtk_box_pack_end (GTK_BOX (popup->footer), settings, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (content), popup->footer, FALSE, FALSE, 0);

  gtk_widget_show_all (content);
  gtk_container_add (GTK_CONTAINER (window), content);
  /* After GtkWindow's own handler, which maps it. */
  g_signal_connect (window, "show", G_CALLBACK (on_window_show), NULL);
  g_signal_connect (window, "hide", G_CALLBACK (on_window_hide), popup);
  sd_popup_set_volume_settings (popup, popup->step, FALSE);
  {
    SdStatus none = { .running = FALSE };

    sd_popup_update (popup, &none);
  }
  return popup;
}

void
sd_popup_update (SdPopup *popup, const SdStatus *status)
{
  popup->has_server = status->running;
  gtk_widget_set_visible (popup->no_server, !status->running);
  for (guint i = 0; i < G_N_ELEMENTS (popup->sides); i++)
    {
      gtk_widget_set_visible (popup->sides[i].row, status->running);
      update_side (&popup->sides[i], status);
    }
  gtk_widget_set_visible (popup->apps, status->running);
  update_apps (popup, status);
}

void
sd_popup_set_volume_settings (SdPopup *popup, guint step, gboolean above_100)
{
  popup->step = step;
  popup->above_100 = above_100;
  for (guint i = 0; i < popup->sliders->len; i++)
    slider_set_range (g_ptr_array_index (popup->sliders, i));
}

void
sd_popup_set_room (SdPopup *popup, int room)
{
  popup->room = room;
  fit (popup);
}

void
sd_popup_set_input_level (SdPopup *popup, double level)
{
  popup->sides[SD_INPUT].level = CLAMP (level, 0, 1);
}

void
sd_popup_set_app_level (SdPopup *popup, SdSide side, const char *app, double level)
{
  g_autofree char *key = app_key (side, app);
  AppRow *row = g_hash_table_lookup (popup->app_rows, key);

  if (row != NULL)
    row->level = CLAMP (level, 0, 1);
}

void
sd_popup_free (SdPopup *popup)
{
  g_clear_handle_id (&popup->fit_idle, g_source_remove);
  g_clear_handle_id (&popup->undo_id, g_source_remove);
  g_signal_handlers_disconnect_by_data (caminito_applet_popup_get_window (popup->applet_popup),
                                        popup);
  /* Its widgets' handlers have the popup for their data: the sliders and the rows go with
     them. */
  gtk_widget_destroy (popup->content);
  for (guint i = 0; i < G_N_ELEMENTS (popup->sides); i++)
    {
      Side *side = &popup->sides[i];

      g_free (side->list_key);
      g_free (side->ports_key);
      g_free (side->device);
      g_free (side->active_port);
    }
  g_ptr_array_unref (popup->sliders);
  g_hash_table_unref (popup->app_rows);
  g_hash_table_unref (popup->undo_hidden);
  g_object_unref (popup->names);
  g_free (popup);
}
