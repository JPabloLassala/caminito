/*
 * One Dock item's widget (CMN-219).
 */

#include <math.h>

#include <libxfce4windowing/libxfce4windowing.h>

#include "dk-item.h"

/* The pulse's period, and the bounce's count and height (CMN-221). */
#define PULSE_PERIOD (1 * G_USEC_PER_SEC)
#define BOUNCES 3
/* The part of the item's thickness the icon gives up to bounce in, as the panel has no room
   beside the item: the icon is smaller while it bounces. */
#define BOUNCE_ROOM 0.2
/* How long the icon takes to shrink into the bounce and to come back. */
#define BOUNCE_RAMP (100 * (G_USEC_PER_SEC / 1000))

struct _DkItemView
{
  GtkWidget       *widget;
  char            *key;
  GIcon           *icon;
  GPtrArray       *windows;
  gboolean         active;
  gboolean         hover;
  /* The target it is dragged by, or NULL when it can't be dragged; and whether it is shown
     faint, as a Kept folder that is gone is (CMN-239). */
  char            *drag_target;
  gboolean         dimmed;
  GtkPositionType  edge;
  DkFeedbackState  feedback;
  DkLauncherEntry  launcher;
  guint            tick_id;
  /* The icon at the size and scale it was last drawn at, until one of them or the theme
     changes. */
  cairo_surface_t *surface;
  int              surface_size;
  int              surface_scale;
};

static void
view_free (gpointer data)
{
  DkItemView *view = data;

  g_free (view->key);
  g_free (view->drag_target);
  g_clear_object (&view->icon);
  g_clear_pointer (&view->windows, g_ptr_array_unref);
  g_clear_pointer (&view->surface, cairo_surface_destroy);
  g_free (view);
}

DkItemView *
dk_item_view_from_widget (GtkWidget *widget)
{
  return g_object_get_data (G_OBJECT (widget), "dk-item-view");
}

GtkWidget *
dk_item_view_get_widget (DkItemView *view)
{
  return view->widget;
}

const char *
dk_item_view_get_key (DkItemView *view)
{
  return view->key;
}

GPtrArray *
dk_item_view_get_windows (DkItemView *view)
{
  return view->windows;
}

gboolean
dk_item_view_is_active (DkItemView *view)
{
  return view->active;
}

static cairo_surface_t *
icon_surface (DkItemView *view, int size, int scale)
{
  GtkIconTheme *theme = gtk_icon_theme_get_for_screen (gtk_widget_get_screen (view->widget));
  GtkIconInfo *info = NULL;
  cairo_surface_t *surface = NULL;

  if (view->icon != NULL)
    info = gtk_icon_theme_lookup_by_gicon_for_scale (theme, view->icon, size, scale,
                                                     GTK_ICON_LOOKUP_FORCE_SIZE);
  if (info == NULL)
    info = gtk_icon_theme_lookup_icon_for_scale (theme, "application-x-executable", size,
                                                 scale, GTK_ICON_LOOKUP_FORCE_SIZE);
  if (info != NULL)
    {
      g_autoptr (GError) error = NULL;

      surface = gtk_icon_info_load_surface (info, gtk_widget_get_window (view->widget), &error);
      if (surface == NULL)
        g_warning ("can't load the icon of %s: %s", view->key, error->message);
      g_object_unref (info);
    }
  return surface;
}

static void
invalidate_icon (DkItemView *view)
{
  g_clear_pointer (&view->surface, cairo_surface_destroy);
  gtk_widget_queue_draw (view->widget);
}

/* The panel's own colour for a highlight where the theme defines one. */
static void
highlight_color (GtkStyleContext *context, GdkRGBA *color)
{
  if (!gtk_style_context_lookup_color (context, "theme_selected_bg_color", color))
    gdk_rgba_parse (color, "#3daee9");
}

/* Plasma's indicator, on the panel's edge: a line for one window, a few short ones for
   several, and in the highlight colour, and longer, for the item that has the focus. */
static void
draw_indicator (DkItemView *view, cairo_t *cr, GtkStyleContext *context, int width, int height)
{
  guint n = view->windows->len;
  gboolean horizontal = view->edge == GTK_POS_BOTTOM || view->edge == GTK_POS_TOP;
  double along = horizontal ? width : height;
  double across = horizontal ? height : width;
  double thickness = MAX (2.0, along / 16.0);
  double length, gap, segment, start, edge_offset;
  guint segments = MIN (n, 3);
  GdkRGBA color;

  if (n == 0)
    return;
  if (view->active)
    highlight_color (context, &color);
  else
    {
      gtk_style_context_get_color (context, GTK_STATE_FLAG_NORMAL, &color);
      color.alpha *= 0.65;
    }
  length = along * (view->active ? 0.8 : 0.5);
  gap = thickness * 1.5;
  segment = segments > 1 ? (length - gap * (segments - 1)) / segments : length;
  start = (along - length) / 2;
  edge_offset = (view->edge == GTK_POS_BOTTOM || view->edge == GTK_POS_RIGHT)
                  ? across - thickness : 0;
  gdk_cairo_set_source_rgba (cr, &color);
  for (guint i = 0; i < segments; i++)
    {
      double at = start + i * (segment + gap);
      double x = horizontal ? at : edge_offset;
      double y = horizontal ? edge_offset : at;
      double w = horizontal ? segment : thickness;
      double h = horizontal ? thickness : segment;
      double r = thickness / 2;

      cairo_new_sub_path (cr);
      cairo_arc (cr, x + r, y + r, r, G_PI, 1.5 * G_PI);
      cairo_arc (cr, x + w - r, y + r, r, 1.5 * G_PI, 2 * G_PI);
      cairo_arc (cr, x + w - r, y + h - r, r, 0, 0.5 * G_PI);
      cairo_arc (cr, x + r, y + h - r, r, 0.5 * G_PI, G_PI);
      cairo_close_path (cr);
      cairo_fill (cr);
    }
}

/* What stands for an icon the theme can't give, as it can't where the image loaders are
   missing: a rounded square with the program's first letter. */
static void
draw_stand_in (DkItemView *view, cairo_t *cr, GtkStyleContext *context, int width, int height,
               int size)
{
  g_autofree char *name = gtk_widget_get_tooltip_text (view->widget);
  g_autofree char *letter = NULL;
  g_autoptr (PangoLayout) layout = NULL;
  PangoFontDescription *font;
  GdkRGBA color;
  double side = size * 0.72, radius = side / 4;
  double x = (width - side) / 2, y = (height - side) / 2;
  int text_width, text_height;

  gtk_style_context_get_color (context, GTK_STATE_FLAG_NORMAL, &color);
  color.alpha = 0.25;
  gdk_cairo_set_source_rgba (cr, &color);
  cairo_new_sub_path (cr);
  cairo_arc (cr, x + side - radius, y + radius, radius, -G_PI / 2, 0);
  cairo_arc (cr, x + side - radius, y + side - radius, radius, 0, G_PI / 2);
  cairo_arc (cr, x + radius, y + side - radius, radius, G_PI / 2, G_PI);
  cairo_arc (cr, x + radius, y + radius, radius, G_PI, 3 * G_PI / 2);
  cairo_close_path (cr);
  cairo_fill (cr);
  if (name == NULL || name[0] == '\0')
    return;
  letter = g_utf8_strup (name, g_utf8_next_char (name) - name);
  layout = gtk_widget_create_pango_layout (view->widget, letter);
  font = pango_font_description_new ();
  pango_font_description_set_absolute_size (font, side * 0.55 * PANGO_SCALE);
  pango_font_description_set_weight (font, PANGO_WEIGHT_BOLD);
  pango_layout_set_font_description (layout, font);
  pango_font_description_free (font);
  pango_layout_get_pixel_size (layout, &text_width, &text_height);
  gtk_style_context_get_color (context, GTK_STATE_FLAG_NORMAL, &color);
  gdk_cairo_set_source_rgba (cr, &color);
  cairo_move_to (cr, (width - text_width) / 2.0, (height - text_height) / 2.0);
  pango_cairo_show_layout (cr, layout);
}

/* The soft box behind the icon of an item with a window that demands attention: it stays
   until the window is focused. */
static void
draw_highlight (cairo_t *cr, GtkStyleContext *context, int width, int height)
{
  GdkRGBA color;
  double side = MIN (width, height) - 4.0, radius = side / 5;
  double x = (width - side) / 2, y = (height - side) / 2;

  highlight_color (context, &color);
  cairo_new_sub_path (cr);
  cairo_arc (cr, x + side - radius, y + radius, radius, -G_PI / 2, 0);
  cairo_arc (cr, x + side - radius, y + side - radius, radius, 0, G_PI / 2);
  cairo_arc (cr, x + radius, y + side - radius, radius, G_PI / 2, G_PI);
  cairo_arc (cr, x + radius, y + radius, radius, G_PI, 3 * G_PI / 2);
  cairo_close_path (cr);
  color.alpha = 0.4;
  gdk_cairo_set_source_rgba (cr, &color);
  cairo_fill_preserve (cr);
  color.alpha = 0.9;
  gdk_cairo_set_source_rgba (cr, &color);
  cairo_set_line_width (cr, 1.5);
  cairo_stroke (cr);
}

static void
rounded_rect (cairo_t *cr, double x, double y, double w, double h)
{
  double r = MIN (w, h) / 2;

  cairo_new_sub_path (cr);
  cairo_arc (cr, x + r, y + r, r, G_PI, 1.5 * G_PI);
  cairo_arc (cr, x + w - r, y + r, r, 1.5 * G_PI, 2 * G_PI);
  cairo_arc (cr, x + w - r, y + h - r, r, 0, 0.5 * G_PI);
  cairo_arc (cr, x + r, y + h - r, r, 0.5 * G_PI, G_PI);
  cairo_close_path (cr);
}

/* The program's count (CMN-227): a pill on the icon's corner away from the panel's edge, at
   the end of the row or the top of the column, where neither the indicator nor the progress
   bar is, in the highlight colour with white text, and "99+" above 99. */
static void
draw_badge (DkItemView *view, cairo_t *cr, GtkStyleContext *context, int width, int height)
{
  g_autofree char *text = view->launcher.count_visible
                            ? dk_launcher_badge_text (view->launcher.count) : NULL;
  g_autoptr (PangoLayout) layout = NULL;
  PangoFontDescription *font;
  GdkRGBA color;
  double size = MIN (width, height);
  double pill_height, pill_width, pad, x, y;
  int text_width, text_height;

  if (text == NULL)
    return;
  layout = gtk_widget_create_pango_layout (view->widget, text);
  font = pango_font_description_new ();
  pango_font_description_set_absolute_size (font, MAX (8.0, size * 0.27) * PANGO_SCALE);
  pango_font_description_set_weight (font, PANGO_WEIGHT_BOLD);
  pango_layout_set_font_description (layout, font);
  pango_font_description_free (font);
  pango_layout_get_pixel_size (layout, &text_width, &text_height);
  pad = text_height * 0.3;
  pill_height = text_height + pad;
  /* A circle for one digit, a pill for more. */
  pill_width = MAX (pill_height, text_width + 2 * pad);
  x = view->edge == GTK_POS_RIGHT ? 0 : width - pill_width;
  y = view->edge == GTK_POS_TOP ? height - pill_height : 0;
  highlight_color (context, &color);
  rounded_rect (cr, x, y, pill_width, pill_height);
  gdk_cairo_set_source_rgba (cr, &color);
  cairo_fill (cr);
  cairo_set_source_rgb (cr, 1, 1, 1);
  cairo_move_to (cr, x + (pill_width - text_width) / 2.0, y + (pill_height - text_height) / 2.0);
  pango_cairo_show_layout (cr, layout);
}

/* The program's progress (CMN-227): a thin bar across the icon on the panel's side, just
   inside the indicator and away from the badge, filling from the start of the row or
   column. */
static void
draw_progress (DkItemView *view, cairo_t *cr, GtkStyleContext *context, int width, int height)
{
  gboolean horizontal = view->edge == GTK_POS_BOTTOM || view->edge == GTK_POS_TOP;
  double along = horizontal ? width : height;
  double across = horizontal ? height : width;
  double thickness = MAX (3.0, MIN (width, height) / 12.0);
  double margin = along / 8.0;
  double length = along - 2 * margin;
  /* Past the indicator, which is MAX (2.0, along / 16.0) thick, and as much again. */
  double inset = 2 * MAX (2.0, along / 16.0);
  double offset = (view->edge == GTK_POS_BOTTOM || view->edge == GTK_POS_RIGHT)
                    ? across - thickness - inset : inset;
  double x = horizontal ? margin : offset;
  double y = horizontal ? offset : margin;
  double fill = length * view->launcher.progress;
  GdkRGBA color;

  if (!view->launcher.progress_visible)
    return;
  cairo_set_source_rgba (cr, 0, 0, 0, 0.55);
  rounded_rect (cr, x, y, horizontal ? length : thickness, horizontal ? thickness : length);
  cairo_fill (cr);
  if (fill < thickness / 2)
    return;
  highlight_color (context, &color);
  gdk_cairo_set_source_rgba (cr, &color);
  /* The fill is at least as long as it is thick, so that its ends stay round. */
  fill = MAX (fill, thickness);
  rounded_rect (cr, x, y, horizontal ? fill : thickness, horizontal ? thickness : fill);
  cairo_fill (cr);
}

/* Where the icon is and how it looks at now (CMN-221): a pulse breathes its opacity and size
   about the item's middle; a bounce is smaller, and jumps from the panel's edge and back
   BOUNCES times. */
typedef struct
{
  double x, y, scale, alpha;
} IconPose;

static IconPose
icon_pose (const DkItemView *view, int width, int height, int size, gint64 now)
{
  IconPose pose = { .scale = 1.0, .alpha = 1.0 };
  gint64 since = now - view->feedback.bounce_start;
  gboolean bouncing = view->feedback.bouncing && since >= 0 && since < DK_BOUNCE_LENGTH;
  double side;

  if (view->feedback.pulsing)
    {
      double breath = (1 + cos (2 * G_PI * (now % PULSE_PERIOD) / (double) PULSE_PERIOD)) / 2;

      pose.alpha = 0.4 + 0.6 * breath;
      pose.scale = 0.9 + 0.1 * breath;
    }
  if (bouncing)
    {
      double ramp = MIN (1.0, MIN (since, DK_BOUNCE_LENGTH - since) / (double) BOUNCE_RAMP);
      double progress = since / (double) DK_BOUNCE_LENGTH;

      pose.scale = 1.0 - BOUNCE_ROOM * ramp;
      side = size * pose.scale;
      pose.x = (width - side) / 2;
      pose.y = (height - side) / 2;
      /* Up from the edge to the room the shrinking left, and down again. */
      {
        double lift = fabs (sin (progress * BOUNCES * G_PI)) * size * (1.0 - pose.scale);

        switch (view->edge)
          {
          case GTK_POS_TOP:
            pose.y = lift;
            break;
          case GTK_POS_LEFT:
            pose.x = lift;
            break;
          case GTK_POS_RIGHT:
            pose.x = width - side - lift;
            break;
          default:
            pose.y = height - side - lift;
          }
      }
      return pose;
    }
  side = size * pose.scale;
  pose.x = (width - side) / 2;
  pose.y = (height - side) / 2;
  return pose;
}

static gboolean
on_draw (GtkWidget *widget, cairo_t *cr, gpointer data)
{
  DkItemView *view = data;
  GtkStyleContext *context = gtk_widget_get_style_context (widget);
  int width = gtk_widget_get_allocated_width (widget);
  int height = gtk_widget_get_allocated_height (widget);
  int size = MIN (width, height);
  int scale = gtk_widget_get_scale_factor (widget);
  IconPose pose = icon_pose (view, width, height, size, g_get_monotonic_time ());

  if (view->dimmed)
    pose.alpha *= 0.4;
  if (view->hover)
    {
      GdkRGBA color;

      gtk_style_context_get_color (context, GTK_STATE_FLAG_NORMAL, &color);
      color.alpha = 0.12;
      gdk_cairo_set_source_rgba (cr, &color);
      cairo_arc (cr, width / 2.0, height / 2.0, size / 2.0, 0, 2 * G_PI);
      cairo_fill (cr);
    }
  if (view->feedback.highlighted)
    draw_highlight (cr, context, width, height);
  if (view->surface == NULL || view->surface_size != size || view->surface_scale != scale)
    {
      g_clear_pointer (&view->surface, cairo_surface_destroy);
      view->surface = icon_surface (view, size, scale);
      view->surface_size = size;
      view->surface_scale = scale;
    }
  cairo_save (cr);
  cairo_translate (cr, pose.x, pose.y);
  cairo_scale (cr, pose.scale, pose.scale);
  if (pose.alpha < 1.0)
    cairo_push_group (cr);
  if (view->surface != NULL)
    {
      /* The icon fills the item's thickness. */
      cairo_set_source_surface (cr, view->surface, 0, 0);
      cairo_paint (cr);
    }
  else
    draw_stand_in (view, cr, context, size, size, size);
  if (pose.alpha < 1.0)
    {
      cairo_pop_group_to_source (cr);
      cairo_paint_with_alpha (cr, pose.alpha);
    }
  cairo_restore (cr);
  draw_indicator (view, cr, context, width, height);
  draw_progress (view, cr, context, width, height);
  draw_badge (view, cr, context, width, height);
  return FALSE;
}

static gboolean
on_crossing (GtkWidget *widget G_GNUC_UNUSED, GdkEventCrossing *event, gpointer data)
{
  DkItemView *view = data;

  view->hover = event->type == GDK_ENTER_NOTIFY && event->mode != GDK_CROSSING_UNGRAB;
  gtk_widget_queue_draw (view->widget);
  return FALSE;
}

/* The rectangle is the item's in its toplevel: on Wayland libxfce4windowing hands it to
   set_rectangle as it is, against the toplevel's surface, so one relative to the item's own
   GdkWindow would put every item at the dock's origin. */
void
dk_item_view_update_rectangles (DkItemView *view)
{
  GtkWidget *toplevel = gtk_widget_get_toplevel (view->widget);
  GdkWindow *window = gtk_widget_get_window (toplevel);
  GdkRectangle rect = { 0, 0, gtk_widget_get_allocated_width (view->widget),
                        gtk_widget_get_allocated_height (view->widget) };

  if (window == NULL || !gtk_widget_get_mapped (view->widget)
      || !gtk_widget_translate_coordinates (view->widget, toplevel, 0, 0, &rect.x, &rect.y))
    return;
  for (guint i = 0; i < view->windows->len; i++)
    xfw_window_set_button_geometry (view->windows->pdata[i], window, &rect, NULL);
}

static void
on_size_allocate (GtkWidget *widget G_GNUC_UNUSED, GdkRectangle *allocation G_GNUC_UNUSED,
                  gpointer data)
{
  dk_item_view_update_rectangles (data);
}

static void
on_map (GtkWidget *widget G_GNUC_UNUSED, gpointer data)
{
  dk_item_view_update_rectangles (data);
}

static void
on_style_updated (GtkWidget *widget G_GNUC_UNUSED, gpointer data)
{
  invalidate_icon (data);
}

static void
on_screen_changed (GtkWidget *widget G_GNUC_UNUSED, GdkScreen *previous G_GNUC_UNUSED,
                   gpointer data)
{
  invalidate_icon (data);
}

static void
on_drag_begin (GtkWidget *widget, GdkDragContext *context, gpointer data)
{
  DkItemView *view = data;
  int size = MIN (gtk_widget_get_allocated_width (widget),
                  gtk_widget_get_allocated_height (widget));
  cairo_surface_t *surface = icon_surface (view, size, gtk_widget_get_scale_factor (widget));

  if (surface != NULL)
    {
      gtk_drag_set_icon_surface (context, surface);
      cairo_surface_destroy (surface);
    }
}

static void
on_drag_data_get (GtkWidget *widget G_GNUC_UNUSED, GdkDragContext *context G_GNUC_UNUSED,
                  GtkSelectionData *selection, guint info G_GNUC_UNUSED,
                  guint time G_GNUC_UNUSED, gpointer data)
{
  DkItemView *view = data;

  if (view->drag_target == NULL)
    return;
  gtk_selection_data_set (selection, gdk_atom_intern (view->drag_target, FALSE), 8,
                          (const guchar *) view->key, strlen (view->key));
}

DkItemView *
dk_item_view_new (void)
{
  DkItemView *view = g_new0 (DkItemView, 1);

  view->widget = gtk_drawing_area_new ();
  view->windows = g_ptr_array_new_with_free_func (g_object_unref);
  view->edge = GTK_POS_BOTTOM;
  g_object_set_data_full (G_OBJECT (view->widget), "dk-item-view", view, view_free);
  gtk_widget_add_events (view->widget, GDK_BUTTON_PRESS_MASK | GDK_BUTTON_RELEASE_MASK
                                         | GDK_ENTER_NOTIFY_MASK | GDK_LEAVE_NOTIFY_MASK);
  g_signal_connect (view->widget, "draw", G_CALLBACK (on_draw), view);
  g_signal_connect (view->widget, "enter-notify-event", G_CALLBACK (on_crossing), view);
  g_signal_connect (view->widget, "leave-notify-event", G_CALLBACK (on_crossing), view);
  g_signal_connect (view->widget, "size-allocate", G_CALLBACK (on_size_allocate), view);
  g_signal_connect (view->widget, "map", G_CALLBACK (on_map), view);
  g_signal_connect (view->widget, "style-updated", G_CALLBACK (on_style_updated), view);
  g_signal_connect (view->widget, "screen-changed", G_CALLBACK (on_screen_changed), view);
  g_signal_connect (view->widget, "drag-begin", G_CALLBACK (on_drag_begin), view);
  g_signal_connect (view->widget, "drag-data-get", G_CALLBACK (on_drag_data_get), view);
  return view;
}

static gboolean
on_tick (GtkWidget *widget, GdkFrameClock *clock G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
  gtk_widget_queue_draw (widget);
  return G_SOURCE_CONTINUE;
}

void
dk_item_view_set_feedback (DkItemView *view, const DkFeedbackState *state)
{
  gboolean animated = state->pulsing || state->bouncing;

  view->feedback = *state;
  /* A frame for every frame the compositor takes while it moves, and none once it's still. */
  if (animated && view->tick_id == 0)
    view->tick_id = gtk_widget_add_tick_callback (view->widget, on_tick, NULL, NULL);
  else if (!animated && view->tick_id != 0)
    {
      gtk_widget_remove_tick_callback (view->widget, view->tick_id);
      view->tick_id = 0;
    }
  gtk_widget_queue_draw (view->widget);
}

DkFeedbackState
dk_item_view_get_feedback (DkItemView *view)
{
  return view->feedback;
}

void
dk_item_view_set_launcher (DkItemView *view, const DkLauncherEntry *entry)
{
  view->launcher = *entry;
  gtk_widget_queue_draw (view->widget);
}

void
dk_item_view_set_geometry (DkItemView *view, guint thickness, GtkPositionType edge)
{
  view->edge = edge;
  gtk_widget_set_size_request (view->widget, thickness, thickness);
  gtk_widget_queue_draw (view->widget);
}

void
dk_item_view_set (DkItemView *view, const char *key, GIcon *icon, const char *name,
                  GPtrArray *windows, gboolean active, gboolean draggable)
{
  if (g_strcmp0 (view->key, key) != 0)
    {
      g_free (view->key);
      view->key = g_strdup (key);
    }
  if (!g_icon_equal (view->icon, icon))
    {
      g_clear_object (&view->icon);
      view->icon = icon != NULL ? g_object_ref (icon) : NULL;
      invalidate_icon (view);
    }
  g_ptr_array_set_size (view->windows, 0);
  for (guint i = 0; i < windows->len; i++)
    g_ptr_array_add (view->windows, g_object_ref (windows->pdata[i]));
  view->active = active;
  gtk_widget_set_tooltip_text (view->widget, name);
  /* A Kept folder's target is its own to set, and not undone here. */
  if (draggable)
    dk_item_view_set_drag (view, DK_DRAG_TARGET);
  else if (g_strcmp0 (view->drag_target, DK_DRAG_TARGET) == 0)
    dk_item_view_set_drag (view, NULL);
  gtk_widget_queue_draw (view->widget);
  dk_item_view_update_rectangles (view);
}

void
dk_item_view_set_drag (DkItemView *view, const char *target)
{
  GtkTargetEntry entry = { (char *) target, GTK_TARGET_SAME_APP, 0 };

  if (g_strcmp0 (view->drag_target, target) == 0)
    return;
  g_free (view->drag_target);
  view->drag_target = g_strdup (target);
  if (target != NULL)
    gtk_drag_source_set (view->widget, GDK_BUTTON1_MASK, &entry, 1, GDK_ACTION_MOVE);
  else
    gtk_drag_source_unset (view->widget);
}

void
dk_item_view_set_dimmed (DkItemView *view, gboolean dimmed)
{
  if (view->dimmed == dimmed)
    return;
  view->dimmed = dimmed;
  gtk_widget_queue_draw (view->widget);
}
