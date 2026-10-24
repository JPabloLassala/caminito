/*
 * The applets' popup (ADR 0035). See caminito-applet-popup.h.
 *
 * It's a GTK_WINDOW_POPUP transient for the panel and placed with
 * gdk_window_move_to_rect(), which is an xdg_popup of the panel's layer surface on Wayland
 * and an override-redirect window on X11, so the same code places it under Caminito and
 * under X11 MATE. A seat grab, as GtkMenu takes, sends every click to it while it's shown.
 */

#include "caminito-applet-popup.h"

/* A click on the applet that closes the popup, when the compositor ends the grab before
   the panel sees the click, mustn't open it again. */
#define REOPEN_DELAY_USEC (250 * 1000)

struct _CaminitoAppletPopup
{
  GtkWidget       *applet;
  GtkWidget       *window;
  gint64           hidden_at;
  /* Set by caminito_applet_popup_set_orient(), else the applet's own is asked. */
  gboolean         orient_fixed;
  MatePanelAppletOrient orient;
  /* The size it was last placed at, so that placing it again follows a change of it. */
  int              placed_width;
  int              placed_height;
};

void
caminito_applet_popup_anchors (MatePanelAppletOrient orient,
                               GdkGravity           *rect_anchor,
                               GdkGravity           *window_anchor,
                               GdkAnchorHints       *hints)
{
  switch (orient)
    {
    case MATE_PANEL_APPLET_ORIENT_UP:
      *rect_anchor = GDK_GRAVITY_NORTH_WEST;
      *window_anchor = GDK_GRAVITY_SOUTH_WEST;
      *hints = GDK_ANCHOR_FLIP_Y | GDK_ANCHOR_SLIDE_X;
      break;
    case MATE_PANEL_APPLET_ORIENT_LEFT:
      *rect_anchor = GDK_GRAVITY_NORTH_WEST;
      *window_anchor = GDK_GRAVITY_NORTH_EAST;
      *hints = GDK_ANCHOR_FLIP_X | GDK_ANCHOR_SLIDE_Y;
      break;
    case MATE_PANEL_APPLET_ORIENT_RIGHT:
      *rect_anchor = GDK_GRAVITY_NORTH_EAST;
      *window_anchor = GDK_GRAVITY_NORTH_WEST;
      *hints = GDK_ANCHOR_FLIP_X | GDK_ANCHOR_SLIDE_Y;
      break;
    case MATE_PANEL_APPLET_ORIENT_DOWN:
    default:
      *rect_anchor = GDK_GRAVITY_SOUTH_WEST;
      *window_anchor = GDK_GRAVITY_NORTH_WEST;
      *hints = GDK_ANCHOR_FLIP_Y | GDK_ANCHOR_SLIDE_X;
      break;
    }
}

gboolean
caminito_applet_popup_is_shown (CaminitoAppletPopup *popup)
{
  return gtk_widget_get_visible (popup->window);
}

void
caminito_applet_popup_hide (CaminitoAppletPopup *popup)
{
  if (!caminito_applet_popup_is_shown (popup))
    return;
  gtk_widget_hide (popup->window);
}

static void
place (CaminitoAppletPopup *popup)
{
  GtkWidget *applet = popup->applet;
  GtkWidget *toplevel = gtk_widget_get_toplevel (applet);
  MatePanelAppletOrient orient = popup->orient;
  GtkAllocation allocation;
  GdkRectangle rect;
  GdkGravity rect_anchor, window_anchor;
  GdkAnchorHints hints;

  gtk_widget_get_allocation (applet, &allocation);
  gtk_widget_translate_coordinates (applet, toplevel, 0, 0, &rect.x, &rect.y);
  rect.width = allocation.width;
  rect.height = allocation.height;
  if (!popup->orient_fixed && MATE_PANEL_IS_APPLET (applet))
    orient = mate_panel_applet_get_orient (MATE_PANEL_APPLET (applet));
  caminito_applet_popup_anchors (orient, &rect_anchor, &window_anchor, &hints);
  popup->placed_width = gtk_widget_get_allocated_width (popup->window);
  popup->placed_height = gtk_widget_get_allocated_height (popup->window);
  gdk_window_move_to_rect (gtk_widget_get_window (popup->window), &rect, rect_anchor,
                           window_anchor, hints, 0, 0);
}

void
caminito_applet_popup_set_orient (CaminitoAppletPopup *popup, MatePanelAppletOrient orient)
{
  popup->orient = orient;
  popup->orient_fixed = TRUE;
}

/* GDK places the window by the size it had then: a popup that opens upwards would grow
   over the panel when a row opens in it. */
static void
on_size_allocate (GtkWidget *window, GdkRectangle *allocation, gpointer data)
{
  CaminitoAppletPopup *popup = data;

  if (!gtk_widget_get_mapped (window)
      || (allocation->width == popup->placed_width
          && allocation->height == popup->placed_height))
    return;
  place (popup);
}

static void
prepare_grab (GdkSeat *seat G_GNUC_UNUSED, GdkWindow *window G_GNUC_UNUSED, gpointer data)
{
  CaminitoAppletPopup *popup = data;

  gtk_widget_show (popup->window);
}

void
caminito_applet_popup_show (CaminitoAppletPopup *popup)
{
  GtkWidget *toplevel = gtk_widget_get_toplevel (popup->applet);
  GdkWindow *window;
  GdkSeat *seat;

  if (caminito_applet_popup_is_shown (popup) || !GTK_IS_WINDOW (toplevel))
    return;

  gtk_window_set_transient_for (GTK_WINDOW (popup->window), GTK_WINDOW (toplevel));
  gtk_window_set_screen (GTK_WINDOW (popup->window), gtk_widget_get_screen (toplevel));
  gtk_widget_realize (popup->window);
  place (popup);

  /* Shown from inside the grab, so that on Wayland the popup is mapped as a grabbing
     xdg_popup, which the compositor closes on a click elsewhere. */
  window = gtk_widget_get_window (popup->window);
  seat = gdk_display_get_default_seat (gdk_window_get_display (window));
  if (gdk_seat_grab (seat, window, GDK_SEAT_CAPABILITY_ALL, TRUE, NULL, NULL, prepare_grab,
                     popup)
      != GDK_GRAB_SUCCESS)
    {
      /* Without the grab a click elsewhere couldn't close it. */
      gtk_widget_hide (popup->window);
      return;
    }
  gtk_grab_add (popup->window);
}

void
caminito_applet_popup_toggle (CaminitoAppletPopup *popup)
{
  if (caminito_applet_popup_is_shown (popup))
    caminito_applet_popup_hide (popup);
  else if (g_get_monotonic_time () - popup->hidden_at > REOPEN_DELAY_USEC)
    caminito_applet_popup_show (popup);
}

GtkWidget *
caminito_applet_popup_get_window (CaminitoAppletPopup *popup)
{
  return popup->window;
}

static void
on_hide (GtkWidget *window, gpointer data)
{
  CaminitoAppletPopup *popup = data;
  GdkDisplay *display = gtk_widget_get_display (window);

  gtk_grab_remove (window);
  gdk_seat_ungrab (gdk_display_get_default_seat (display));
  popup->hidden_at = g_get_monotonic_time ();
}

/* With the grab, every click comes here: one outside the popup, the applet's own among
   them, closes it. */
static gboolean
on_window_button_press (GtkWidget *window, GdkEventButton *event, gpointer data)
{
  CaminitoAppletPopup *popup = data;
  GdkWindow *toplevel = gtk_widget_get_window (window);
  GdkWindow *event_window = event->window;
  GtkAllocation allocation;
  double x = event->x, y = event->y;

  /* On X11 the grab reports a click anywhere in the client to the popup's window, with the
     coordinates of the window it was in: one on the applet reads as a point in the popup.
     The root coordinates are the popup's own to compare. Wayland needs no such thing,
     since its compositor ends the popup itself when the click is elsewhere. */
  if (g_str_equal (G_OBJECT_TYPE_NAME (gtk_widget_get_display (window)), "GdkX11Display"))
    {
      int origin_x, origin_y;

      gdk_window_get_origin (toplevel, &origin_x, &origin_y);
      if (event->x_root < origin_x || event->y_root < origin_y
          || event->x_root >= origin_x + gdk_window_get_width (toplevel)
          || event->y_root >= origin_y + gdk_window_get_height (toplevel))
        {
          caminito_applet_popup_hide (popup);
          return TRUE;
        }
    }
  /* In the popup's own coordinates, not the root's, which Wayland doesn't have. */
  if (gdk_window_get_toplevel (event_window) == toplevel)
    {
      while (event_window != toplevel)
        {
          gdk_window_coords_to_parent (event_window, x, y, &x, &y);
          event_window = gdk_window_get_effective_parent (event_window);
        }
      gtk_widget_get_allocation (window, &allocation);
      if (x >= 0 && y >= 0 && x < allocation.width && y < allocation.height)
        return FALSE;
    }
  caminito_applet_popup_hide (popup);
  return TRUE;
}

static gboolean
on_window_key_press (GtkWidget *window, GdkEventKey *event, gpointer data)
{
  GtkWidget *focus = gtk_window_get_focus (GTK_WINDOW (window));

  if (event->keyval == GDK_KEY_Escape)
    {
      caminito_applet_popup_hide (data);
      return TRUE;
    }
  /* A button clicks at once: GtkButton's own activation takes a keyboard grab, which ends
     the popup's, so that the popup would close and the button drop its click (CMN-215). */
  if (GTK_IS_BUTTON (focus) && gtk_widget_is_sensitive (focus)
      && (event->keyval == GDK_KEY_space || event->keyval == GDK_KEY_KP_Space
          || event->keyval == GDK_KEY_Return || event->keyval == GDK_KEY_KP_Enter
          || event->keyval == GDK_KEY_ISO_Enter))
    {
      gtk_button_clicked (GTK_BUTTON (focus));
      return TRUE;
    }
  return FALSE;
}

static gboolean
on_grab_broken (GtkWidget *window G_GNUC_UNUSED, GdkEventGrabBroken *event G_GNUC_UNUSED,
                gpointer data)
{
  caminito_applet_popup_hide (data);
  return TRUE;
}

/* On Wayland the compositor ends an xdg_popup itself, with a click elsewhere, and GDK
   unmaps its window under the widget. */
static gboolean
on_unmap_event (GtkWidget *window G_GNUC_UNUSED, GdkEvent *event G_GNUC_UNUSED, gpointer data)
{
  caminito_applet_popup_hide (data);
  return FALSE;
}

static void
on_grab_notify (GtkWidget *window, gboolean was_grabbed, gpointer data)
{
  GtkWidget *grab;

  if (was_grabbed)
    return;
  /* A slider in the popup takes the grab for itself as it's dragged (gtkrange.c,
     range_grab_add), which shadows the window too. */
  grab = gtk_window_group_get_current_grab (gtk_window_get_group (GTK_WINDOW (window)));
  if (grab != NULL && gtk_widget_is_ancestor (grab, window))
    return;
  /* Another of the panel's windows took GTK's grab: a menu, or a dialog. */
  caminito_applet_popup_hide (data);
}

static gboolean
on_applet_button_press (GtkWidget *applet G_GNUC_UNUSED, GdkEventButton *event, gpointer data)
{
  /* Middle-click and right-click are the panel's: moving the applet, and its menu. */
  if (event->button != GDK_BUTTON_PRIMARY || event->type != GDK_BUTTON_PRESS)
    return FALSE;
  caminito_applet_popup_toggle (data);
  return TRUE;
}

static void
popup_free (gpointer data)
{
  CaminitoAppletPopup *popup = data;

  /* Ends the grab, if it's shown, before its handlers go. */
  caminito_applet_popup_hide (popup);
  g_signal_handlers_disconnect_by_data (popup->window, popup);
  gtk_widget_destroy (popup->window);
  g_object_unref (popup->window);
  g_free (popup);
}

/* Themes draw a border on a menu's window, and none on a window of this kind, so the
   popup would melt into what is behind it. The theme's own CSS wins over this. */
static void
add_border (void)
{
  static gboolean added;
  g_autoptr (GtkCssProvider) provider = NULL;

  if (added)
    return;
  added = TRUE;
  provider = gtk_css_provider_new ();
  gtk_css_provider_load_from_data (provider,
                                   ".caminito-applet-popup { border: 1px solid alpha(currentColor, 0.3); }",
                                   -1, NULL);
  gtk_style_context_add_provider_for_screen (gdk_screen_get_default (),
                                             GTK_STYLE_PROVIDER (provider),
                                             GTK_STYLE_PROVIDER_PRIORITY_FALLBACK);
}

CaminitoAppletPopup *
caminito_applet_popup_new (GtkWidget *applet)
{
  CaminitoAppletPopup *popup = g_new0 (CaminitoAppletPopup, 1);

  add_border ();
  popup->applet = applet;
  popup->orient = MATE_PANEL_APPLET_ORIENT_DOWN;
  popup->window = g_object_ref_sink (gtk_window_new (GTK_WINDOW_POPUP));
  gtk_window_set_type_hint (GTK_WINDOW (popup->window), GDK_WINDOW_TYPE_HINT_POPUP_MENU);
  gtk_style_context_add_class (gtk_widget_get_style_context (popup->window),
                               "caminito-applet-popup");
  gtk_widget_add_events (popup->window,
                         GDK_BUTTON_PRESS_MASK | GDK_KEY_PRESS_MASK | GDK_STRUCTURE_MASK);
  g_signal_connect (popup->window, "hide", G_CALLBACK (on_hide), popup);
  g_signal_connect_after (popup->window, "size-allocate", G_CALLBACK (on_size_allocate), popup);
  g_signal_connect (popup->window, "button-press-event",
                    G_CALLBACK (on_window_button_press), popup);
  g_signal_connect (popup->window, "key-press-event", G_CALLBACK (on_window_key_press), popup);
  g_signal_connect (popup->window, "grab-broken-event", G_CALLBACK (on_grab_broken), popup);
  g_signal_connect (popup->window, "grab-notify", G_CALLBACK (on_grab_notify), popup);
  g_signal_connect (popup->window, "unmap-event", G_CALLBACK (on_unmap_event), popup);
  g_signal_connect (applet, "button-press-event", G_CALLBACK (on_applet_button_press), popup);
  g_object_set_data_full (G_OBJECT (applet), "caminito-applet-popup", popup, popup_free);
  return popup;
}
