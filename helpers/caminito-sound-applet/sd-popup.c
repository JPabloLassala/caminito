/*
 * The Sound applet's popup (CMN-213, CMN-215). See sd-popup.h.
 *
 * The rows are built once and updated in place, and a device list is built again only when
 * its devices change, in the same box: a new viewport would lose the scroll position, and a
 * widget built again loses the focus (CMN-210).
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

static const char *const side_names[] = { "output", "input" };

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
  GtkWidget *scale;
  gulong     value_changed;
  /* A button is held on the slider, or the user moved it a moment ago: the server's volume
     waits, since its answers to the earlier moves may come after the later ones. */
  gboolean   dragging;
  guint      hold_id;
  /* The Default device's volume as the server last gave it, in percent. */
  guint      server_percent;
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

/* The volume a slider shows: the server's, unless the slider is already on it, so that a
   glide's unrounded shares aren't rounded away by the server's answers. */
static void
show_volume (Side *side)
{
  GtkRange *range = GTK_RANGE (side->scale);
  double shown = MIN (side->server_percent, cap (side->popup));

  if (side->dragging || side->hold_id != 0
      || (guint) (gtk_range_get_value (range) + 0.5) == (guint) shown)
    return;
  g_signal_handler_block (side->scale, side->value_changed);
  gtk_range_set_value (range, shown);
  g_signal_handler_unblock (side->scale, side->value_changed);
}

static gboolean
on_hold_end (gpointer data)
{
  Side *side = data;

  side->hold_id = 0;
  show_volume (side);
  return G_SOURCE_REMOVE;
}

static void
on_value_changed (GtkRange *range, gpointer data)
{
  Side *side = data;
  SdPopup *popup = side->popup;

  g_clear_handle_id (&side->hold_id, g_source_remove);
  side->hold_id = g_timeout_add (HOLD_MS, on_hold_end, side);
  popup->actions->set_volume (side->side, sd_state_volume_of_percent (gtk_range_get_value (range)),
                              popup->data);
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
  Side *side = data;
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
    gtk_range_set_value (range, gtk_range_get_value (range) + clicks * side->popup->step);
  return TRUE;
}

static gboolean
on_scale_press (GtkWidget *scale G_GNUC_UNUSED, GdkEventButton *event G_GNUC_UNUSED,
                gpointer data)
{
  Side *side = data;

  side->dragging = TRUE;
  return FALSE;
}

static gboolean
on_scale_release (GtkWidget *scale G_GNUC_UNUSED, GdkEventButton *event G_GNUC_UNUSED,
                  gpointer data)
{
  Side *side = data;

  side->dragging = FALSE;
  show_volume (side);
  return FALSE;
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

static void
on_pill_clicked (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  Side *side = data;
  GtkRevealer *revealer = GTK_REVEALER (side->revealer);

  gtk_revealer_set_reveal_child (revealer, !gtk_revealer_get_reveal_child (revealer));
}

static void
on_ports_clicked (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  Side *side = data;
  GtkRevealer *revealer = GTK_REVEALER (side->ports_revealer);

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

/* The meter takes the latest level once a frame, rather than at every peak (CMN-210). */
static gboolean
on_meter_tick (GtkWidget *meter, GdkFrameClock *clock G_GNUC_UNUSED, gpointer data)
{
  Side *side = data;

  if (gtk_level_bar_get_value (GTK_LEVEL_BAR (meter)) != side->level)
    gtk_level_bar_set_value (GTK_LEVEL_BAR (meter), side->level);
  return G_SOURCE_CONTINUE;
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
  gtk_widget_set_sensitive (side->scale, device != NULL);
  side->server_percent = device != NULL ? sd_state_percent (device->volume) : 0;
  show_volume (side);
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

  side->scale = gtk_scale_new_with_range (GTK_ORIENTATION_HORIZONTAL, 0, 100, 1);
  gtk_scale_set_draw_value (GTK_SCALE (side->scale), FALSE);
  gtk_widget_set_hexpand (side->scale, TRUE);
  set_name (side->scale, side, "scale", NULL);
  side->value_changed = g_signal_connect (side->scale, "value-changed",
                                          G_CALLBACK (on_value_changed), side);
  /* Before GtkRange's class handlers, which they run ahead of. */
  g_signal_connect (side->scale, "scroll-event", G_CALLBACK (on_scale_scroll), side);
  g_signal_connect (side->scale, "button-press-event", G_CALLBACK (on_scale_press), side);
  g_signal_connect (side->scale, "button-release-event", G_CALLBACK (on_scale_release), side);
  gtk_box_pack_start (GTK_BOX (middle), side->scale, FALSE, FALSE, 0);
  if (which == SD_INPUT)
    {
      side->meter = gtk_level_bar_new_for_interval (0, 1);
      /* One green, not the theme's low, high and full colours. */
      gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (side->meter), GTK_LEVEL_BAR_OFFSET_LOW);
      gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (side->meter), GTK_LEVEL_BAR_OFFSET_HIGH);
      gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (side->meter), GTK_LEVEL_BAR_OFFSET_FULL);
      add_class (side->meter, "sd-meter");
      g_object_set (side->meter, "margin-start", 8, "margin-end", 8, NULL);
      set_name (side->meter, side, "meter", NULL);
      gtk_widget_add_tick_callback (side->meter, on_meter_tick, side, NULL);
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
  g_signal_connect (side->pill, "clicked", G_CALLBACK (on_pill_clicked), side);

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
  g_signal_connect (side->ports_button, "clicked", G_CALLBACK (on_ports_clicked), side);
  gtk_box_pack_start (GTK_BOX (side->ports), side->ports_button, FALSE, FALSE, 0);
  side->ports_list = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  g_object_set (side->ports_list, "margin-start", 12, NULL);
  side->ports_revealer = gtk_revealer_new ();
  gtk_revealer_set_transition_duration (GTK_REVEALER (side->ports_revealer), REVEAL_MS);
  gtk_container_add (GTK_CONTAINER (side->ports_revealer), side->ports_list);
  gtk_box_pack_start (GTK_BOX (side->ports), side->ports_revealer, FALSE, FALSE, 0);
  gtk_widget_show_all (side->ports_button);
  gtk_widget_show_all (side->ports_revealer);
  gtk_box_pack_start (GTK_BOX (drop), side->ports, FALSE, FALSE, 0);

  add_class (drop, "sd-drop");
  g_object_set (drop, "margin-start", 12, "margin-end", 12, "margin-top", 4, NULL);
  side->revealer = gtk_revealer_new ();
  gtk_revealer_set_transition_duration (GTK_REVEALER (side->revealer), REVEAL_MS);
  gtk_container_add (GTK_CONTAINER (side->revealer), drop);
  gtk_box_pack_start (GTK_BOX (box), side->revealer, FALSE, FALSE, 0);
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

  add_css ();
  popup->applet_popup = applet_popup;
  popup->actions = actions;
  popup->data = data;
  popup->step = 5;

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
  gtk_container_add (GTK_CONTAINER (caminito_applet_popup_get_window (applet_popup)), content);
  /* After GtkWindow's own handler, which maps it. */
  g_signal_connect (caminito_applet_popup_get_window (applet_popup), "show",
                    G_CALLBACK (on_window_show), NULL);
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
  gtk_widget_set_visible (popup->no_server, !status->running);
  for (guint i = 0; i < G_N_ELEMENTS (popup->sides); i++)
    {
      gtk_widget_set_visible (popup->sides[i].row, status->running);
      update_side (&popup->sides[i], status);
    }
}

void
sd_popup_set_volume_settings (SdPopup *popup, guint step, gboolean above_100)
{
  popup->step = step;
  popup->above_100 = above_100;
  for (guint i = 0; i < G_N_ELEMENTS (popup->sides); i++)
    {
      Side *side = &popup->sides[i];
      GtkRange *range = GTK_RANGE (side->scale);

      /* A narrower range clamps the value, which isn't the user asking for it. */
      g_signal_handler_block (side->scale, side->value_changed);
      gtk_range_set_range (range, 0, cap (popup));
      gtk_range_set_increments (range, step, 2.0 * step);
      g_signal_handler_unblock (side->scale, side->value_changed);
      show_volume (side);
    }
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
sd_popup_free (SdPopup *popup)
{
  g_clear_handle_id (&popup->fit_idle, g_source_remove);
  /* Its widgets' handlers have the popup for their data. */
  gtk_widget_destroy (popup->content);
  for (guint i = 0; i < G_N_ELEMENTS (popup->sides); i++)
    {
      Side *side = &popup->sides[i];

      g_clear_handle_id (&side->hold_id, g_source_remove);
      g_free (side->list_key);
      g_free (side->ports_key);
      g_free (side->device);
      g_free (side->active_port);
    }
  g_free (popup);
}
