/*
 * The applets' popup (ADR 0035): where it goes for each edge of the panel, and, with a
 * display, that the applet's left-click toggles it, a click elsewhere and Escape close it,
 * and it opens against the applet.
 */

#include "caminito-applet-popup.h"
#include "caminito-applet-icon.h"

static void
test_anchors (void)
{
  GdkGravity rect, window;
  GdkAnchorHints hints;

  /* A top panel: the popup's top-left at the applet's bottom-left. */
  caminito_applet_popup_anchors (MATE_PANEL_APPLET_ORIENT_DOWN, &rect, &window, &hints);
  g_assert_cmpint (rect, ==, GDK_GRAVITY_SOUTH_WEST);
  g_assert_cmpint (window, ==, GDK_GRAVITY_NORTH_WEST);
  g_assert_cmpint (hints, ==, GDK_ANCHOR_FLIP_Y | GDK_ANCHOR_SLIDE_X);

  caminito_applet_popup_anchors (MATE_PANEL_APPLET_ORIENT_UP, &rect, &window, &hints);
  g_assert_cmpint (rect, ==, GDK_GRAVITY_NORTH_WEST);
  g_assert_cmpint (window, ==, GDK_GRAVITY_SOUTH_WEST);
  g_assert_cmpint (hints, ==, GDK_ANCHOR_FLIP_Y | GDK_ANCHOR_SLIDE_X);

  /* A panel on the left edge opens to the right. */
  caminito_applet_popup_anchors (MATE_PANEL_APPLET_ORIENT_RIGHT, &rect, &window, &hints);
  g_assert_cmpint (rect, ==, GDK_GRAVITY_NORTH_EAST);
  g_assert_cmpint (window, ==, GDK_GRAVITY_NORTH_WEST);
  g_assert_cmpint (hints, ==, GDK_ANCHOR_FLIP_X | GDK_ANCHOR_SLIDE_Y);

  caminito_applet_popup_anchors (MATE_PANEL_APPLET_ORIENT_LEFT, &rect, &window, &hints);
  g_assert_cmpint (rect, ==, GDK_GRAVITY_NORTH_WEST);
  g_assert_cmpint (window, ==, GDK_GRAVITY_NORTH_EAST);
  g_assert_cmpint (hints, ==, GDK_ANCHOR_FLIP_X | GDK_ANCHOR_SLIDE_Y);
}

static void
test_icon_size (void)
{
  g_assert_cmpint (caminito_applet_icon_pixel_size (16), ==, 16);
  g_assert_cmpint (caminito_applet_icon_pixel_size (24), ==, 16);
  g_assert_cmpint (caminito_applet_icon_pixel_size (26), ==, 22);
  g_assert_cmpint (caminito_applet_icon_pixel_size (28), ==, 24);
  g_assert_cmpint (caminito_applet_icon_pixel_size (36), ==, 32);
  g_assert_cmpint (caminito_applet_icon_pixel_size (64), ==, 48);
}

/* Dispatches what's queued. The clicks are delivered synchronously, and showing and
   hiding are too, so there's no state to wait for after that. */
static void
settle (void)
{
  gdk_display_sync (gdk_display_get_default ());
  while (g_main_context_iteration (NULL, FALSE))
    ;
}

static void
click (GtkWidget *widget, guint button)
{
  GdkEvent *event = gdk_event_new (GDK_BUTTON_PRESS);
  GdkSeat *seat = gdk_display_get_default_seat (gtk_widget_get_display (widget));
  int x, y;

  event->button.window = g_object_ref (gtk_widget_get_window (widget));
  event->button.button = button;
  gdk_window_get_origin (event->button.window, &x, &y);
  event->button.x = event->button.y = 2;
  event->button.x_root = x + 2;
  event->button.y_root = y + 2;
  event->button.time = GDK_CURRENT_TIME;
  gdk_event_set_device (event, gdk_seat_get_pointer (seat));
  gtk_main_do_event (event);
  gdk_event_free (event);
}

static void
test_toggle (void)
{
  GtkWidget *panel, *applet, *label;
  CaminitoAppletPopup *popup;
  GdkEvent *escape;
  int panel_x, panel_y, x, y;

  /* A stand-in panel at the top of the screen, as mate-panel's toplevel holds an
     in-process applet. */
  panel = gtk_window_new (GTK_WINDOW_POPUP);
  gtk_window_move (GTK_WINDOW (panel), 100, 0);
  gtk_window_set_default_size (GTK_WINDOW (panel), 200, 24);
  /* An event box in the applet's place: a MatePanelApplet needs the panel's D-Bus. */
  applet = gtk_event_box_new ();
  gtk_widget_set_size_request (applet, 24, 24);
  gtk_widget_set_halign (applet, GTK_ALIGN_START);
  gtk_container_add (GTK_CONTAINER (panel), applet);
  popup = caminito_applet_popup_new (applet);
  label = gtk_label_new ("A popup");
  gtk_widget_set_size_request (label, 150, 80);
  gtk_container_add (GTK_CONTAINER (caminito_applet_popup_get_window (popup)), label);
  gtk_widget_show_all (panel);
  gtk_widget_show (label);
  settle ();
  g_assert_false (caminito_applet_popup_is_shown (popup));

  /* Right-click and middle-click are the panel's. */
  click (applet, GDK_BUTTON_SECONDARY);
  click (applet, GDK_BUTTON_MIDDLE);
  settle ();
  g_assert_false (caminito_applet_popup_is_shown (popup));

  click (applet, GDK_BUTTON_PRIMARY);
  settle ();
  g_assert_true (caminito_applet_popup_is_shown (popup));
  gdk_window_get_origin (gtk_widget_get_window (panel), &panel_x, &panel_y);
  gdk_window_get_origin (gtk_widget_get_window (caminito_applet_popup_get_window (popup)),
                         &x, &y);
  /* Under the applet, at its left edge. */
  g_assert_cmpint (x, ==, panel_x);
  g_assert_cmpint (y, ==, panel_y + 24);

  /* The applet's click goes to the popup's grab, which closes it, and doesn't open it
     again at once. */
  click (applet, GDK_BUTTON_PRIMARY);
  settle ();
  g_assert_false (caminito_applet_popup_is_shown (popup));
  click (applet, GDK_BUTTON_PRIMARY);
  settle ();
  g_assert_false (caminito_applet_popup_is_shown (popup));

  /* Past the popup's own delay against reopening (250 ms), which is a time, not a state
     to wait for. */
  g_usleep (300 * 1000);
  click (applet, GDK_BUTTON_PRIMARY);
  settle ();
  g_assert_true (caminito_applet_popup_is_shown (popup));
  /* A click inside leaves it. The label draws on the popup's own window. */
  click (label, GDK_BUTTON_PRIMARY);
  settle ();
  g_assert_true (caminito_applet_popup_is_shown (popup));

  escape = gdk_event_new (GDK_KEY_PRESS);
  escape->key.window =
    g_object_ref (gtk_widget_get_window (caminito_applet_popup_get_window (popup)));
  escape->key.keyval = GDK_KEY_Escape;
  gdk_event_set_device (escape, gdk_seat_get_keyboard (gdk_display_get_default_seat (
                                  gdk_display_get_default ())));
  gtk_main_do_event (escape);
  gdk_event_free (escape);
  settle ();
  g_assert_false (caminito_applet_popup_is_shown (popup));

  gtk_widget_destroy (panel);
  settle ();
}

/* On X11 a click outside the popup, on the applet, say, is reported to the popup's window
   (the grab's) with coordinates of the window the click was in, so that the popup's own
   rectangle can contain them. Only the root coordinates tell. */
static void
test_click_reported_to_the_popup_from_outside (void)
{
  GtkWidget *panel, *applet, *label;
  CaminitoAppletPopup *popup;
  GdkEvent *event;
  GdkSeat *seat;
  GdkWindow *window;
  int x, y;

  if (!g_str_equal (G_OBJECT_TYPE_NAME (gdk_display_get_default ()), "GdkX11Display"))
    {
      g_test_skip ("not X11");
      return;
    }
  panel = gtk_window_new (GTK_WINDOW_POPUP);
  gtk_window_move (GTK_WINDOW (panel), 100, 0);
  gtk_window_set_default_size (GTK_WINDOW (panel), 200, 24);
  applet = gtk_event_box_new ();
  gtk_widget_set_size_request (applet, 24, 24);
  gtk_widget_set_halign (applet, GTK_ALIGN_START);
  gtk_container_add (GTK_CONTAINER (panel), applet);
  popup = caminito_applet_popup_new (applet);
  label = gtk_label_new ("A popup");
  gtk_widget_set_size_request (label, 150, 80);
  gtk_container_add (GTK_CONTAINER (caminito_applet_popup_get_window (popup)), label);
  gtk_widget_show_all (panel);
  gtk_widget_show (label);
  settle ();

  click (applet, GDK_BUTTON_PRIMARY);
  settle ();
  g_assert_true (caminito_applet_popup_is_shown (popup));

  /* The click on the applet, as the grab reports it: to the popup's window, with the
     applet's own coordinates (2, 2), which are inside the popup's 150 x 80 as well. */
  window = gtk_widget_get_window (caminito_applet_popup_get_window (popup));
  seat = gdk_display_get_default_seat (gdk_display_get_default ());
  gdk_window_get_origin (gtk_widget_get_window (applet), &x, &y);
  event = gdk_event_new (GDK_BUTTON_PRESS);
  event->button.window = g_object_ref (window);
  event->button.button = GDK_BUTTON_PRIMARY;
  event->button.x = event->button.y = 2;
  event->button.x_root = x + 2;
  event->button.y_root = y + 2;
  event->button.time = GDK_CURRENT_TIME;
  gdk_event_set_device (event, gdk_seat_get_pointer (seat));
  gtk_main_do_event (event);
  gdk_event_free (event);
  settle ();
  g_assert_false (caminito_applet_popup_is_shown (popup));

  gtk_widget_destroy (panel);
  settle ();
}

/* A popup that opens upwards, from a panel at the bottom, keeps its bottom edge against the
   panel when a row opens in it and it grows. */
static void
test_grows_upwards (void)
{
  GtkWidget *panel, *applet, *label;
  CaminitoAppletPopup *popup;
  GdkRectangle monitor;
  int panel_x, panel_y, x, y;

  gdk_monitor_get_geometry (gdk_display_get_primary_monitor (gdk_display_get_default ()),
                            &monitor);
  panel = gtk_window_new (GTK_WINDOW_POPUP);
  gtk_window_move (GTK_WINDOW (panel), 100, monitor.y + monitor.height - 24);
  gtk_window_set_default_size (GTK_WINDOW (panel), 200, 24);
  applet = gtk_event_box_new ();
  gtk_widget_set_size_request (applet, 24, 24);
  gtk_widget_set_halign (applet, GTK_ALIGN_START);
  gtk_container_add (GTK_CONTAINER (panel), applet);
  popup = caminito_applet_popup_new (applet);
  caminito_applet_popup_set_orient (popup, MATE_PANEL_APPLET_ORIENT_UP);
  label = gtk_label_new ("A popup");
  gtk_widget_set_size_request (label, 150, 80);
  gtk_container_add (GTK_CONTAINER (caminito_applet_popup_get_window (popup)), label);
  gtk_widget_show_all (panel);
  gtk_widget_show (label);
  settle ();

  click (applet, GDK_BUTTON_PRIMARY);
  settle ();
  g_assert_true (caminito_applet_popup_is_shown (popup));
  gdk_window_get_origin (gtk_widget_get_window (panel), &panel_x, &panel_y);
  gdk_window_get_origin (gtk_widget_get_window (caminito_applet_popup_get_window (popup)),
                         &x, &y);
  g_assert_cmpint (x, ==, panel_x);
  g_assert_cmpint (y + 80, ==, panel_y);

  gtk_widget_set_size_request (label, 150, 200);
  /* The window is resized in the frame clock's layout phase, not at once. */
  for (int i = 0; i < 100 && gtk_widget_get_allocated_height (
                               caminito_applet_popup_get_window (popup)) != 200; i++)
    {
      g_usleep (10 * 1000);
      settle ();
    }
  settle ();
  gdk_window_get_origin (gtk_widget_get_window (caminito_applet_popup_get_window (popup)),
                         &x, &y);
  g_assert_cmpint (y + 200, ==, panel_y);

  gtk_widget_destroy (panel);
  settle ();
}

int
main (int argc, char **argv)
{
  gboolean display;

  g_test_init (&argc, &argv, NULL);
  display = gtk_init_check (&argc, &argv);
  g_test_add_func ("/popup/opens-away-from-the-panels-edge", test_anchors);
  g_test_add_func ("/icon/leaves-the-panel-2-px-each-side", test_icon_size);
  if (display)
    {
      g_test_add_func ("/popup/left-click-toggles-and-a-click-elsewhere-closes", test_toggle);
      g_test_add_func ("/popup/a-click-reported-to-the-popup-from-outside-closes-it",
                       test_click_reported_to_the_popup_from_outside);
      g_test_add_func ("/popup/one-that-opens-upwards-keeps-its-bottom-at-the-panel-as-it-grows",
                       test_grows_upwards);
    }
  else
    g_test_message ("No display: the popup's clicks aren't tested");
  return g_test_run ();
}
