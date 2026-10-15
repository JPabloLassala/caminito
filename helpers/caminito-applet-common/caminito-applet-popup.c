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
  MatePanelAppletOrient orient = MATE_PANEL_APPLET_ORIENT_DOWN;
  GtkAllocation allocation;
  GdkRectangle rect;
  GdkGravity rect_anchor, window_anchor;
  GdkAnchorHints hints;

  gtk_widget_get_allocation (applet, &allocation);
  gtk_widget_translate_coordinates (applet, toplevel, 0, 0, &rect.x, &rect.y);
  rect.width = allocation.width;
  rect.height = allocation.height;
  if (MATE_PANEL_IS_APPLET (applet))
    orient = mate_panel_applet_get_orient (MATE_PANEL_APPLET (applet));
  caminito_applet_popup_anchors (orient, &rect_anchor, &window_anchor, &hints);
  gdk_window_move_to_rect (gtk_widget_get_window (popup->window), &rect, rect_anchor,
                           window_anchor, hints, 0, 0);
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
on_window_key_press (GtkWidget *window G_GNUC_UNUSED, GdkEventKey *event, gpointer data)
{
  if (event->keyval != GDK_KEY_Escape)
    return FALSE;
  caminito_applet_popup_hide (data);
  return TRUE;
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
on_grab_notify (GtkWidget *window G_GNUC_UNUSED, gboolean was_grabbed, gpointer data)
{
  /* Another of the panel's windows took GTK's grab: a menu, or a dialog. */
  if (!was_grabbed)
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

CaminitoAppletPopup *
caminito_applet_popup_new (GtkWidget *applet)
{
  CaminitoAppletPopup *popup = g_new0 (CaminitoAppletPopup, 1);

  popup->applet = applet;
  popup->window = g_object_ref_sink (gtk_window_new (GTK_WINDOW_POPUP));
  gtk_window_set_type_hint (GTK_WINDOW (popup->window), GDK_WINDOW_TYPE_HINT_POPUP_MENU);
  gtk_style_context_add_class (gtk_widget_get_style_context (popup->window),
                               "caminito-applet-popup");
  gtk_widget_add_events (popup->window,
                         GDK_BUTTON_PRESS_MASK | GDK_KEY_PRESS_MASK | GDK_STRUCTURE_MASK);
  g_signal_connect (popup->window, "hide", G_CALLBACK (on_hide), popup);
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
