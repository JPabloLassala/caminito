/*
 * The Sound popup's output and input rows (CMN-215), drawn from statuses built by hand: the
 * mute buttons, the sliders and the pills, the device lists and the port dropdown, the
 * input's meter, the wheel, the height against the room it's given, the scroll position
 * across an update, and the keyboard. Its Applications (CMN-216): System sounds, a row for
 * each Sound application with its loudest stream's slider, the Recording heading, the
 * expanded row's devices and streams, the badge and the activity bars. What the user does is
 * read back from the calls it makes. Without a display, GTK can't start, and it's skipped.
 *
 * The popup's window is shown off every output, without the grab: it's the popup's own kind
 * of window, which follows its content's minimum height as the real one does, where an
 * offscreen window wouldn't (CMN-210). Wheel and button events go in through
 * gtk_main_do_event() on the slider's own GdkWindow, which is where GDK delivers a real one.
 */

#include "sd-popup.h"

#define PERCENT(p) ((pa_volume_t) (((guint64) (p) * PA_VOLUME_NORM + 50) / 100))
#define WAIT_LIMIT_USEC (5 * G_USEC_PER_SEC)

typedef struct
{
  /* The panel's stand-in, which the popup is placed against. */
  GtkWidget           *panel;
  GtkWidget           *anchor;
  CaminitoAppletPopup *applet_popup;
  SdPopup             *popup;
  /* What the popup called back, as "volume output 50;", "default input usb;"… */
  GString             *calls;
} Fixture;

static const char *const side_names[] = { "output", "input" };

static void
on_set_volume (SdSide side, pa_volume_t volume, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "volume %s %u;", side_names[side],
                          sd_state_percent (volume));
}

static void
on_set_muted (SdSide side, gboolean muted, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "mute %s %s;", side_names[side], muted ? "on" : "off");
}

static void
on_set_default (SdSide side, const char *device, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "default %s %s;", side_names[side], device);
}

static void
on_set_port (SdSide side, const char *device, const char *port, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "port %s %s %s;", side_names[side], device, port);
}

static void
on_open_settings (gpointer data)
{
  Fixture *fixture = data;

  g_string_append (fixture->calls, "settings;");
}

static void
on_set_app_volume (SdSide side, const char *app, pa_volume_t volume, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "app %s %s %u;", side_names[side], app,
                          sd_state_percent (volume));
}

static void
on_set_stream_volume (SdSide side, guint32 stream, pa_volume_t volume, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "stream %s %u %u;", side_names[side], stream,
                          sd_state_percent (volume));
}

static void
on_move_app (SdSide side, const char *app, const char *device, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "move %s %s %s;", side_names[side], app,
                          device != NULL ? device : "default");
}

static void
on_set_system_volume (pa_volume_t volume, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "system %u;", sd_state_percent (volume));
}

static const SdPopupActions actions = {
  .set_volume = on_set_volume,
  .set_muted = on_set_muted,
  .set_default = on_set_default,
  .set_port = on_set_port,
  .open_settings = on_open_settings,
  .set_app_volume = on_set_app_volume,
  .set_stream_volume = on_set_stream_volume,
  .move_app = on_move_app,
  .set_system_volume = on_set_system_volume,
};

static GtkWidget *
window_of (Fixture *fixture)
{
  return caminito_applet_popup_get_window (fixture->applet_popup);
}

typedef struct
{
  GtkWidget *window;
  guint      quiet;
} Settle;

static void
on_after_paint (GdkFrameClock *clock G_GNUC_UNUSED, gpointer data)
{
  Settle *settle = data;

  /* A frame with an event or an idle (the popup's fit) still to run isn't a quiet one. */
  if (gtk_events_pending ())
    settle->quiet = 0;
  else
    settle->quiet++;
}

/* Runs frames until three in a row have nothing left to lay out or to run. */
static void
settle (Fixture *fixture)
{
  GtkWidget *window = window_of (fixture);
  Settle state = { window, 0 };
  gint64 end = g_get_monotonic_time () + WAIT_LIMIT_USEC;
  GdkFrameClock *clock;
  gulong handler;

  while (gtk_widget_get_frame_clock (window) == NULL)
    g_main_context_iteration (NULL, TRUE);
  clock = gtk_widget_get_frame_clock (window);
  handler = g_signal_connect (clock, "after-paint", G_CALLBACK (on_after_paint), &state);
  while (state.quiet < 3)
    {
      if (g_get_monotonic_time () > end)
        g_error ("timed out waiting for the popup to settle");
      gdk_frame_clock_request_phase (clock, GDK_FRAME_CLOCK_PHASE_PAINT);
      g_main_context_iteration (NULL, FALSE);
      g_usleep (1000);
    }
  g_signal_handler_disconnect (clock, handler);
}

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  GtkWidget *window;

  fixture->panel = gtk_window_new (GTK_WINDOW_POPUP);
  gtk_window_move (GTK_WINDOW (fixture->panel), -10000, -10000);
  fixture->anchor = g_object_ref (gtk_button_new ());
  gtk_container_add (GTK_CONTAINER (fixture->panel), fixture->anchor);
  gtk_widget_show_all (fixture->panel);
  fixture->applet_popup = caminito_applet_popup_new (fixture->anchor);
  fixture->popup = sd_popup_new (fixture->applet_popup, &actions, fixture);
  fixture->calls = g_string_new (NULL);
  sd_popup_set_volume_settings (fixture->popup, 5, FALSE);
  /* Off every output, without the grab, which needs a panel. */
  window = window_of (fixture);
  gtk_window_set_transient_for (GTK_WINDOW (window), GTK_WINDOW (fixture->panel));
  gtk_window_move (GTK_WINDOW (window), -10000, -10000);
  gtk_widget_show (window);
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  sd_popup_free (fixture->popup);
  /* The applet's popup lives as long as the applet: its button here. */
  gtk_widget_destroy (fixture->panel);
  g_object_unref (fixture->anchor);
  g_string_free (fixture->calls, TRUE);
}

typedef struct
{
  const char *name;
  const char *text;
  GtkWidget  *found;
} Search;

static void
visit (GtkWidget *widget, gpointer data)
{
  Search *search = data;

  if (search->found != NULL)
    return;
  if (search->name != NULL && g_strcmp0 (gtk_widget_get_name (widget), search->name) == 0)
    search->found = widget;
  else if (search->text != NULL && GTK_IS_LABEL (widget)
           && g_strcmp0 (gtk_label_get_text (GTK_LABEL (widget)), search->text) == 0)
    search->found = widget;
  if (GTK_IS_CONTAINER (widget))
    gtk_container_forall (GTK_CONTAINER (widget), visit, search);
}

/* The widget the popup named so: "output-scale", "input-pill"… */
static GtkWidget *
named (Fixture *fixture, const char *name)
{
  Search search = { name, NULL, NULL };

  visit (window_of (fixture), &search);
  g_assert_nonnull (search.found);
  return search.found;
}

static GtkWidget *
label_under (GtkWidget *widget, const char *text)
{
  Search search = { NULL, text, NULL };

  visit (widget, &search);
  return search.found;
}

/* Whether widget would show: it and every widget above it are visible, and every revealer
   above it is open. */
static gboolean
widget_shows (Fixture *fixture, GtkWidget *found)
{
  GtkWidget *window = window_of (fixture);

  if (found == NULL)
    return FALSE;
  for (GtkWidget *widget = found; widget != window; widget = gtk_widget_get_parent (widget))
    {
      if (!gtk_widget_get_visible (widget) || !gtk_widget_get_child_visible (widget))
        return FALSE;
      if (GTK_IS_REVEALER (widget) && !gtk_revealer_get_reveal_child (GTK_REVEALER (widget)))
        return FALSE;
    }
  return TRUE;
}

static gboolean
shows (Fixture *fixture, const char *text)
{
  return widget_shows (fixture, label_under (window_of (fixture), text));
}

/* The button around the label with text. */
static GtkWidget *
button_of (Fixture *fixture, const char *text)
{
  GtkWidget *widget = label_under (window_of (fixture), text);

  g_assert_nonnull (widget);
  while (widget != NULL && !GTK_IS_BUTTON (widget))
    widget = gtk_widget_get_parent (widget);
  g_assert_nonnull (widget);
  return widget;
}

static double
value_of (Fixture *fixture, const char *scale)
{
  return gtk_range_get_value (GTK_RANGE (named (fixture, scale)));
}

static GtkAdjustment *
vadjustment (Fixture *fixture)
{
  return gtk_scrolled_window_get_vadjustment (GTK_SCROLLED_WINDOW (named (fixture, "scrolled")));
}

static const char *
first_icon_name (GtkWidget *image)
{
  GIcon *gicon = NULL;

  gtk_image_get_gicon (GTK_IMAGE (image), &gicon, NULL);
  g_assert_true (G_IS_THEMED_ICON (gicon));
  return g_themed_icon_get_names (G_THEMED_ICON (gicon))[0];
}

static SdPort speaker_ports[] = { { "analog-output-speaker", "Speakers" },
                                  { "analog-output-lineout", "Line Out" } };

/* Three outputs, the headset the default at 45%, and two microphones. */
static SdStatus
devices (void)
{
  static SdDevice outputs[] = {
    { .name = "alsa", .description = "Built-in Audio", .form = SD_FORM_SPEAKER,
      .volume = PERCENT (60), .ports = speaker_ports, .n_ports = 1,
      .active_port = "analog-output-speaker", .active_port_description = "Speakers" },
    { .name = "headset", .description = "WH-1000XM4", .form = SD_FORM_HEADPHONES,
      .volume = PERCENT (45) },
    { .name = "easyeffects", .description = "Easy Effects Sink", .form = SD_FORM_SPEAKER,
      .virtual_device = TRUE, .volume = PERCENT (100) },
  };
  static SdDevice inputs[] = {
    { .name = "usb", .description = "Yeti Stereo Microphone", .form = SD_FORM_MICROPHONE,
      .volume = PERCENT (80) },
    { .name = "mic", .description = "Internal Microphone", .form = SD_FORM_MICROPHONE,
      .volume = PERCENT (30) },
  };

  return (SdStatus) {
    .running = TRUE,
    .has_output = TRUE,
    .output_name = "WH-1000XM4",
    .volume = PERCENT (45),
    .headphones = TRUE,
    .has_input = TRUE,
    .outputs = outputs,
    .n_outputs = G_N_ELEMENTS (outputs),
    .inputs = inputs,
    .n_inputs = G_N_ELEMENTS (inputs),
    .default_output = "headset",
    .default_input = "usb",
  };
}

static void
test_no_sound_server_is_said_over_the_footer (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdStatus none = { .running = FALSE };
  SdStatus status = devices ();

  sd_popup_update (fixture->popup, &none);
  g_assert_true (shows (fixture, "No sound server"));
  g_assert_true (shows (fixture, "Sound settings…"));
  g_assert_false (widget_shows (fixture, named (fixture, "output-row")));
  g_assert_false (widget_shows (fixture, named (fixture, "input-row")));
  sd_popup_update (fixture->popup, &status);
  g_assert_false (shows (fixture, "No sound server"));
  g_assert_true (widget_shows (fixture, named (fixture, "output-row")));
  g_assert_true (widget_shows (fixture, named (fixture, "input-row")));
}

static void
test_the_rows_show_the_defaults (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();

  sd_popup_update (fixture->popup, &status);
  g_assert_true (shows (fixture, "WH-1000XM4"));
  g_assert_true (shows (fixture, "Yeti Stereo Microphone"));
  g_assert_cmpfloat (value_of (fixture, "output-scale"), ==, 45);
  g_assert_cmpfloat (value_of (fixture, "input-scale"), ==, 80);
  g_assert_cmpstr (first_icon_name (named (fixture, "output-mute-image")), ==, "audio-headphones");
  g_assert_cmpstr (first_icon_name (named (fixture, "input-mute-image")), ==,
                   "microphone-sensitivity-high");
  /* The lists are closed. */
  g_assert_false (shows (fixture, "Easy Effects Sink"));
  g_assert_false (shows (fixture, "Internal Microphone"));
}

static void
test_the_pill_opens_the_list_with_the_default_checked (Fixture *fixture,
                                                       gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();
  GtkWidget *list;

  sd_popup_update (fixture->popup, &status);
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-pill")));
  g_assert_true (shows (fixture, "Built-in Audio"));
  g_assert_true (shows (fixture, "Easy Effects Sink"));
  /* The port, or "Virtual device". */
  g_assert_true (shows (fixture, "Speakers"));
  g_assert_true (shows (fixture, "Virtual device"));
  list = named (fixture, "output-list");
  g_assert_true (widget_shows (fixture, named (fixture, "output-check-headset")));
  g_assert_false (widget_shows (fixture, named (fixture, "output-check-alsa")));
  g_assert_false (widget_shows (fixture, named (fixture, "output-check-easyeffects")));
  g_assert_nonnull (list);
  /* The input's stays closed, and the pill again closes the output's. */
  g_assert_false (shows (fixture, "Internal Microphone"));
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-pill")));
  g_assert_false (shows (fixture, "Easy Effects Sink"));
}

static void
test_choosing_a_device_asks_for_it_as_the_default (Fixture *fixture,
                                                   gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();

  sd_popup_update (fixture->popup, &status);
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-pill")));
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "Easy Effects Sink")));
  gtk_button_clicked (GTK_BUTTON (named (fixture, "input-pill")));
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "Internal Microphone")));
  g_assert_cmpstr (fixture->calls->str, ==, "default output easyeffects;default input mic;");
  /* It shows what the server makes of it, and no more. */
  g_assert_true (widget_shows (fixture, named (fixture, "output-check-headset")));
  status.default_output = "easyeffects";
  sd_popup_update (fixture->popup, &status);
  g_assert_true (widget_shows (fixture, named (fixture, "output-check-easyeffects")));
  g_assert_false (widget_shows (fixture, named (fixture, "output-check-headset")));
  g_assert_true (shows (fixture, "Easy Effects Sink"));
}

static void
test_the_port_dropdown_needs_two_plugged_ports (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();
  GtkWidget *ports;

  status.default_output = "alsa";
  sd_popup_update (fixture->popup, &status);
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-pill")));
  ports = named (fixture, "output-ports");
  g_assert_false (widget_shows (fixture, ports));
  /* The line out plugged in: a second port. The dropdown opens in the popup, not in a menu
     of its own, whose grab would close the popup. */
  status.outputs[0].n_ports = 2;
  sd_popup_update (fixture->popup, &status);
  g_assert_true (widget_shows (fixture, ports));
  g_assert_nonnull (label_under (named (fixture, "output-ports-button"), "Speakers"));
  g_assert_false (shows (fixture, "Line Out"));
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-ports-button")));
  g_assert_true (shows (fixture, "Line Out"));
  g_assert_true (widget_shows (fixture, named (fixture, "output-port-check-analog-output-speaker")));
  g_assert_false (widget_shows (fixture,
                                named (fixture, "output-port-check-analog-output-lineout")));
  g_assert_cmpstr (fixture->calls->str, ==, "");
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-port-analog-output-lineout")));
  g_assert_cmpstr (fixture->calls->str, ==, "port output alsa analog-output-lineout;");
  /* And out again. */
  status.outputs[0].n_ports = 1;
  sd_popup_update (fixture->popup, &status);
  g_assert_false (widget_shows (fixture, ports));
  status.outputs[0].n_ports = 1;
}

static void
test_the_mute_buttons_ask_for_the_other_state (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();

  sd_popup_update (fixture->popup, &status);
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-mute")));
  status.inputs[0].muted = TRUE;
  sd_popup_update (fixture->popup, &status);
  gtk_button_clicked (GTK_BUTTON (named (fixture, "input-mute")));
  g_assert_cmpstr (fixture->calls->str, ==, "mute output on;mute input off;");
  g_assert_cmpstr (first_icon_name (named (fixture, "input-mute-image")), ==,
                   "microphone-sensitivity-muted");
  status.inputs[0].muted = FALSE;
}

static void
test_only_the_input_has_a_meter (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();
  Search search = { "output-meter", NULL, NULL };
  GtkWidget *meter;

  sd_popup_update (fixture->popup, &status);
  meter = named (fixture, "input-meter");
  g_assert_true (widget_shows (fixture, meter));
  visit (window_of (fixture), &search);
  g_assert_null (search.found);
  sd_popup_set_input_level (fixture->popup, 0.8);
  settle (fixture);
  g_assert_cmpfloat_with_epsilon (gtk_level_bar_get_value (GTK_LEVEL_BAR (meter)), 0.8, 1e-9);
}

static void
test_the_sliders_stop_at_100_or_150_percent (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();
  GtkAdjustment *adjustment;

  status.outputs[1].volume = PERCENT (120);
  sd_popup_update (fixture->popup, &status);
  adjustment = gtk_range_get_adjustment (GTK_RANGE (named (fixture, "output-scale")));
  g_assert_cmpfloat (gtk_adjustment_get_upper (adjustment), ==, 100);
  g_assert_cmpfloat (value_of (fixture, "output-scale"), ==, 100);
  sd_popup_set_volume_settings (fixture->popup, 5, TRUE);
  g_assert_cmpfloat (gtk_adjustment_get_upper (adjustment), ==, 150);
  g_assert_cmpfloat (value_of (fixture, "output-scale"), ==, 120);
  sd_popup_set_volume_settings (fixture->popup, 5, FALSE);
  /* A server's volume shown under the cap isn't a volume asked for. */
  g_assert_cmpstr (fixture->calls->str, ==, "");
  status.outputs[1].volume = PERCENT (45);
}

static GdkWindow *
event_window_of (GdkWindow *parent, GtkWidget *widget)
{
  gpointer user_data;

  gdk_window_get_user_data (parent, &user_data);
  if (user_data == widget && gdk_window_get_window_type (parent) == GDK_WINDOW_CHILD)
    return parent;
  for (GList *l = gdk_window_peek_children (parent); l != NULL; l = l->next)
    {
      GdkWindow *found = event_window_of (l->data, widget);

      if (found != NULL)
        return found;
    }
  return NULL;
}

static guint32 event_time = 1000;

/* An event of type at widget's middle, on its own GdkWindow. */
static GdkEvent *
event_at (GtkWidget *widget, GdkEventType type)
{
  GdkWindow *window = event_window_of (gtk_widget_get_window (gtk_widget_get_toplevel (widget)),
                                       widget);
  GdkSeat *seat = gdk_display_get_default_seat (gtk_widget_get_display (widget));
  GdkEvent *event = gdk_event_new (type);

  if (window == NULL)
    window = gtk_widget_get_window (widget);
  event->any.window = g_object_ref (window);
  gdk_event_set_device (event, gdk_seat_get_pointer (seat));
  gdk_event_set_source_device (event, gdk_seat_get_pointer (seat));
  return event;
}

/* A wheel event over widget: smooth with delta_y, or a whole click when direction isn't
   GDK_SCROLL_SMOOTH. */
static void
scroll (GtkWidget *widget, GdkScrollDirection direction, double delta_y, guint state)
{
  GdkEvent *event = event_at (widget, GDK_SCROLL);

  event->scroll.direction = direction;
  event->scroll.delta_y = direction == GDK_SCROLL_SMOOTH ? delta_y : 0;
  event->scroll.state = state;
  event->scroll.time = ++event_time;
  event->scroll.x = gdk_window_get_width (event->any.window) / 2.0;
  event->scroll.y = gdk_window_get_height (event->any.window) / 2.0;
  gtk_main_do_event (event);
  gdk_event_free (event);
}

static void
button (GtkWidget *widget, GdkEventType type)
{
  GdkEvent *event = event_at (widget, type);
  int width = gdk_window_get_width (event->any.window);
  GtkAllocation allocation;

  gtk_widget_get_allocation (widget, &allocation);
  event->button.button = GDK_BUTTON_PRIMARY;
  event->button.time = ++event_time;
  /* On the slider's knob, at 45%, so that a press doesn't move it. */
  event->button.x = width * gtk_range_get_value (GTK_RANGE (widget)) / 100.0;
  event->button.y = gdk_window_get_height (event->any.window) / 2.0;
  event->button.state = type == GDK_BUTTON_RELEASE ? GDK_BUTTON1_MASK : 0;
  gtk_main_do_event (event);
  gdk_event_free (event);
}

static void
test_a_slider_follows_the_server_unless_dragged (Fixture *fixture,
                                                 gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();
  GtkWidget *scale;

  sd_popup_update (fixture->popup, &status);
  settle (fixture);
  scale = named (fixture, "output-scale");
  button (scale, GDK_BUTTON_PRESS);
  status.outputs[1].volume = PERCENT (70);
  sd_popup_update (fixture->popup, &status);
  g_assert_cmpfloat (value_of (fixture, "output-scale"), !=, 70);
  button (scale, GDK_BUTTON_RELEASE);
  sd_popup_update (fixture->popup, &status);
  g_assert_cmpfloat (value_of (fixture, "output-scale"), ==, 70);
  status.outputs[1].volume = PERCENT (45);
}

static gboolean
output_scale_at_45 (gpointer data)
{
  Fixture *fixture = data;

  return value_of (fixture, "output-scale") == 45;
}

static void
test_a_stale_volume_doesnt_undo_a_step (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();
  GtkWidget *scale;
  gint64 end = g_get_monotonic_time () + WAIT_LIMIT_USEC;

  sd_popup_update (fixture->popup, &status);
  settle (fixture);
  scale = named (fixture, "output-scale");
  scroll (scale, GDK_SCROLL_DOWN, 0, GDK_SHIFT_MASK);
  /* The server's answer to an earlier change, read before this step reached it. */
  sd_popup_update (fixture->popup, &status);
  g_assert_cmpfloat (value_of (fixture, "output-scale"), ==, 40);
  scroll (scale, GDK_SCROLL_DOWN, 0, GDK_SHIFT_MASK);
  g_assert_cmpstr (fixture->calls->str, ==, "volume output 40;volume output 35;");
  /* The server's volume shows once the user leaves the slider, if nothing else comes: here
     it never took the steps. */
  while (!output_scale_at_45 (fixture))
    {
      if (g_get_monotonic_time () > end)
        g_error ("timed out waiting for the server's volume to show");
      g_main_context_iteration (NULL, FALSE);
      g_usleep (1000);
    }
}

/* Enough outputs, with the list open, for the popup to scroll in a short room. */
static SdStatus
many_outputs (SdDevice *outputs, guint n)
{
  SdStatus status = devices ();

  for (guint i = 0; i < n; i++)
    outputs[i] = (SdDevice) {
      .name = g_strdup_printf ("sink%u", i),
      .description = g_strdup_printf ("Output %u", i),
      .form = SD_FORM_SPEAKER,
      .volume = PERCENT (50),
    };
  status.outputs = outputs;
  status.n_outputs = n;
  status.default_output = "sink0";
  return status;
}

static void
many_outputs_free (SdDevice *outputs, guint n)
{
  for (guint i = 0; i < n; i++)
    {
      g_free (outputs[i].name);
      g_free (outputs[i].description);
    }
}

static void
test_the_plain_wheel_scrolls_and_shift_moves_the_slider (Fixture *fixture,
                                                         gconstpointer data G_GNUC_UNUSED)
{
  SdDevice outputs[12];
  SdStatus status = many_outputs (outputs, G_N_ELEMENTS (outputs));
  GtkWidget *scale;
  double top;

  sd_popup_set_room (fixture->popup, 300);
  sd_popup_update (fixture->popup, &status);
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-pill")));
  settle (fixture);
  scale = named (fixture, "output-scale");
  top = gtk_adjustment_get_value (vadjustment (fixture));

  /* A click of a smooth wheel, and a whole click, down: the popup scrolls. */
  scroll (scale, GDK_SCROLL_SMOOTH, 1, 0);
  scroll (scale, GDK_SCROLL_DOWN, 0, 0);
  settle (fixture);
  g_assert_cmpfloat (gtk_adjustment_get_value (vadjustment (fixture)), >, top);
  g_assert_cmpfloat (value_of (fixture, "output-scale"), ==, 50);
  g_assert_cmpstr (fixture->calls->str, ==, "");

  /* With Shift the slider moves by the step, the way the wheel turns, and the popup stays. */
  top = gtk_adjustment_get_value (vadjustment (fixture));
  scroll (scale, GDK_SCROLL_UP, 0, GDK_SHIFT_MASK);
  g_assert_cmpfloat (value_of (fixture, "output-scale"), ==, 55);
  /* X11's smooth click is 1.0 (CMN-210), in eighths as a glide sends it. */
  for (int i = 0; i < 8; i++)
    scroll (scale, GDK_SCROLL_SMOOTH, 0.125, GDK_SHIFT_MASK);
  g_assert_cmpfloat_with_epsilon (value_of (fixture, "output-scale"), 50, 1e-9);
  settle (fixture);
  g_assert_cmpfloat (gtk_adjustment_get_value (vadjustment (fixture)), ==, top);
  g_assert_true (g_str_has_prefix (fixture->calls->str, "volume output 55;"));
  g_assert_true (g_str_has_suffix (fixture->calls->str, "volume output 50;"));

  /* The step is volume-step's. */
  sd_popup_set_volume_settings (fixture->popup, 10, FALSE);
  scroll (scale, GDK_SCROLL_DOWN, 0, GDK_SHIFT_MASK);
  g_assert_cmpfloat_with_epsilon (value_of (fixture, "output-scale"), 40, 1e-9);
  many_outputs_free (outputs, G_N_ELEMENTS (outputs));
}

static void
test_a_short_room_scrolls_with_the_footer_shown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdDevice outputs[12];
  SdStatus status = many_outputs (outputs, G_N_ELEMENTS (outputs));
  GtkAdjustment *adjustment;
  GtkWidget *window = window_of (fixture);
  int closed;

  sd_popup_update (fixture->popup, &status);
  settle (fixture);
  closed = gtk_widget_get_allocated_height (window);
  sd_popup_set_room (fixture->popup, 300);
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-pill")));
  settle (fixture);
  adjustment = vadjustment (fixture);
  g_assert_cmpint (gtk_widget_get_allocated_height (window), <=, 300);
  g_assert_cmpint (gtk_widget_get_allocated_height (window), >=, 290);
  g_assert_cmpfloat (gtk_adjustment_get_upper (adjustment), >,
                     gtk_adjustment_get_page_size (adjustment));
  g_assert_true (gtk_widget_get_mapped (named (fixture, "footer")));
  /* Closed again, it's as tall as its rows. */
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-pill")));
  settle (fixture);
  g_assert_cmpint (gtk_widget_get_allocated_height (window), ==, closed);
  many_outputs_free (outputs, G_N_ELEMENTS (outputs));
}

static void
test_an_update_keeps_the_scroll_position (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdDevice outputs[12];
  SdStatus status = many_outputs (outputs, G_N_ELEMENTS (outputs));

  sd_popup_set_room (fixture->popup, 300);
  sd_popup_update (fixture->popup, &status);
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-pill")));
  settle (fixture);
  gtk_adjustment_set_value (vadjustment (fixture), 100);
  settle (fixture);
  /* A device unplugged: the list is built again. */
  status.n_outputs--;
  sd_popup_update (fixture->popup, &status);
  settle (fixture);
  g_assert_cmpfloat (gtk_adjustment_get_value (vadjustment (fixture)), ==, 100);
  many_outputs_free (outputs, G_N_ELEMENTS (outputs));
}

/* The focus the popup's grab gives its window: GDK makes the grab's focus-in, on X11 and
   Wayland alike, a focus-in, and a GtkButton takes the focus from Tab only in a window that
   has it. */
static void
focus_in (Fixture *fixture)
{
  GtkWidget *window = window_of (fixture);
  GdkSeat *seat = gdk_display_get_default_seat (gtk_widget_get_display (window));
  GdkEvent *event = gdk_event_new (GDK_FOCUS_CHANGE);

  event->focus_change.window = g_object_ref (gtk_widget_get_window (window));
  event->focus_change.in = TRUE;
  gdk_event_set_device (event, gdk_seat_get_keyboard (seat));
  gtk_main_do_event (event);
  gdk_event_free (event);
  g_assert_true (gtk_window_has_toplevel_focus (GTK_WINDOW (window)));
}

/* A key press, delivered to the popup's window as its grab would. */
static void
key (Fixture *fixture, guint keyval)
{
  GtkWidget *window = window_of (fixture);
  GdkSeat *seat = gdk_display_get_default_seat (gtk_widget_get_display (window));
  GdkEvent *event = gdk_event_new (GDK_KEY_PRESS);

  event->key.window = g_object_ref (gtk_widget_get_window (window));
  event->key.keyval = keyval;
  event->key.time = ++event_time;
  gdk_event_set_device (event, gdk_seat_get_keyboard (seat));
  gtk_main_do_event (event);
  gdk_event_free (event);
}

/* The names of what Tab reaches from the top, in order, once round. */
static char *
tab_order (Fixture *fixture)
{
  GtkWindow *window = GTK_WINDOW (window_of (fixture));
  GString *order = g_string_new (NULL);
  GtkWidget *first = NULL;

  focus_in (fixture);
  gtk_window_set_focus (window, NULL);
  for (int i = 0; i < 64; i++)
    {
      GtkWidget *focus;

      /* What Tab does: GtkWindow's binding, which wraps round at the end. */
      if (!gtk_bindings_activate (G_OBJECT (window), GDK_KEY_Tab, 0))
        break;
      focus = gtk_window_get_focus (window);
      if (focus == NULL)
        break;
      if (focus == first)
        break;
      if (first == NULL)
        first = focus;
      g_string_append_printf (order, "%s ", gtk_widget_get_name (focus));
    }
  return g_string_free (order, FALSE);
}

static void
test_the_keyboard_reaches_every_control (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();
  g_autofree char *closed = NULL;
  g_autofree char *open = NULL;
  GtkWidget *mute, *scale;

  sd_popup_update (fixture->popup, &status);
  settle (fixture);
  closed = tab_order (fixture);
  g_assert_cmpstr (closed, ==,
                   "output-mute output-scale output-pill input-mute input-scale input-pill "
                   "settings ");
  gtk_button_clicked (GTK_BUTTON (named (fixture, "output-pill")));
  settle (fixture);
  open = tab_order (fixture);
  g_assert_cmpstr (open, ==,
                   "output-mute output-scale output-pill output-device-alsa "
                   "output-device-headset output-device-easyeffects input-mute input-scale "
                   "input-pill settings ");

  /* Space on a mute button, and the arrows on a slider. */
  mute = named (fixture, "output-mute");
  gtk_widget_grab_focus (mute);
  /* Space as the popup's grab delivers it, to the window. */
  key (fixture, GDK_KEY_space);
  scale = named (fixture, "output-scale");
  gtk_widget_grab_focus (scale);
  g_assert_true (gtk_bindings_activate (G_OBJECT (scale), GDK_KEY_Right, 0));
  g_assert_cmpstr (fixture->calls->str, ==, "mute output on;volume output 50;");
}

static void
test_each_opening_starts_the_keyboard_at_the_top (Fixture *fixture,
                                                  gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();
  GtkWindow *window = GTK_WINDOW (window_of (fixture));

  sd_popup_update (fixture->popup, &status);
  settle (fixture);
  gtk_widget_grab_focus (named (fixture, "input-scale"));
  gtk_widget_hide (GTK_WIDGET (window));
  gtk_widget_show (GTK_WIDGET (window));
  g_assert_true (gtk_widget_get_mapped (GTK_WIDGET (window)));
  focus_in (fixture);
  /* GtkWindow would focus the first control as it maps, with no ring, and Tab would skip
     it; or keep the last opening's. */
  g_assert_null (gtk_window_get_focus (window));
  g_assert_true (gtk_bindings_activate (G_OBJECT (window), GDK_KEY_Tab, 0));
  g_assert_cmpstr (gtk_widget_get_name (gtk_window_get_focus (window)), ==, "output-mute");
}

static void
test_sound_settings_closes_the_popup_and_calls_back (Fixture *fixture,
                                                      gconstpointer data G_GNUC_UNUSED)
{
  SdStatus status = devices ();

  sd_popup_update (fixture->popup, &status);
  gtk_button_clicked (GTK_BUTTON (named (fixture, "settings")));
  g_assert_cmpstr (fixture->calls->str, ==, "settings;");
  g_assert_false (gtk_widget_get_visible (window_of (fixture)));
}

static SdStreamVolume mpv_streams[] = { { 40, "song.flac", PERCENT (80) },
                                     { 41, "talk.webm", PERCENT (40) } };
static SdStreamVolume firefox_streams[] = { { 42, "A video", PERCENT (100) } };
static SdStreamVolume obs_streams[] = { { 50, "Mic/Aux", PERCENT (90) } };

/* devices(), with System sounds at 70%, mpv's two streams and Firefox playing, and OBS
   Studio recording. */
static SdStatus
with_apps (SdApp *apps)
{
  SdStatus status = devices ();

  apps[0] = (SdApp) { .id = "mpv", .name = "mpv", .icon = "mpv", .side = SD_OUTPUT,
                      .volume = PERCENT (80), .streams = mpv_streams, .n_streams = 2 };
  apps[1] = (SdApp) { .id = "Firefox", .name = "Firefox", .icon = "firefox", .side = SD_OUTPUT,
                      .volume = PERCENT (100), .streams = firefox_streams, .n_streams = 1 };
  apps[2] = (SdApp) { .id = "OBS Studio", .name = "OBS Studio", .side = SD_INPUT,
                      .volume = PERCENT (90), .streams = obs_streams, .n_streams = 1 };
  status.apps = apps;
  status.n_apps = 3;
  status.has_system_sounds = TRUE;
  status.system_volume = PERCENT (70);
  return status;
}

/* Whether a is above b in the popup. */
static gboolean
above (Fixture *fixture, GtkWidget *a, GtkWidget *b)
{
  int ay, by;

  settle (fixture);
  g_assert_true (gtk_widget_translate_coordinates (a, window_of (fixture), 0, 0, NULL, &ay));
  g_assert_true (gtk_widget_translate_coordinates (b, window_of (fixture), 0, 0, NULL, &by));
  return ay < by;
}

static void
test_system_sounds_is_the_first_application (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdApp apps[3];
  SdStatus status = with_apps (apps);

  sd_popup_update (fixture->popup, &status);
  g_assert_true (shows (fixture, "System sounds"));
  g_assert_cmpfloat (value_of (fixture, "system-scale"), ==, 70);
  g_assert_true (above (fixture, named (fixture, "input-row"), named (fixture, "system-row")));
  g_assert_true (above (fixture, named (fixture, "system-row"), named (fixture, "app-output-mpv")));
  gtk_range_set_value (GTK_RANGE (named (fixture, "system-scale")), 40);
  g_assert_cmpstr (fixture->calls->str, ==, "system 40;");
  /* A server without the stream-restore extension: nothing to set. */
  status.has_system_sounds = FALSE;
  sd_popup_update (fixture->popup, &status);
  g_assert_false (gtk_widget_is_sensitive (named (fixture, "system-scale")));
}

static void
test_an_apps_row_shows_its_loudest_stream (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdApp apps[3];
  SdStatus status = with_apps (apps);

  sd_popup_update (fixture->popup, &status);
  g_assert_true (shows (fixture, "mpv"));
  g_assert_true (shows (fixture, "Firefox"));
  g_assert_cmpfloat (value_of (fixture, "app-output-mpv-scale"), ==, 80);
  g_assert_cmpstr (gtk_label_get_text (GTK_LABEL (named (fixture, "app-output-mpv-percent"))), ==,
                   "80%");
  g_assert_true (above (fixture, named (fixture, "app-output-mpv"),
                        named (fixture, "app-output-Firefox")));
  /* The application's own icon, and the activity bar under it. */
  g_assert_cmpstr (first_icon_name (named (fixture, "app-output-mpv-icon")), ==, "mpv");
  g_assert_true (widget_shows (fixture, named (fixture, "app-output-mpv-bar")));
  /* The pin is CMN-217's. */
  g_assert_false (widget_shows (fixture, named (fixture, "app-output-mpv-pin")));
  /* Following the default: no badge. */
  g_assert_false (widget_shows (fixture, named (fixture, "app-output-mpv-badge")));
  /* Its slider sets the application: the reader keeps its streams' ratio. */
  gtk_range_set_value (GTK_RANGE (named (fixture, "app-output-mpv-scale")), 60);
  g_assert_cmpstr (fixture->calls->str, ==, "app output mpv 60;");
  g_assert_cmpstr (gtk_label_get_text (GTK_LABEL (named (fixture, "app-output-mpv-percent"))), ==,
                   "60%");
}

static void
test_the_recording_applications_have_a_heading (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdApp apps[3];
  SdStatus status = with_apps (apps);

  sd_popup_update (fixture->popup, &status);
  g_assert_true (widget_shows (fixture, named (fixture, "recording-heading")));
  g_assert_true (above (fixture, named (fixture, "app-output-Firefox"),
                        named (fixture, "recording-heading")));
  g_assert_true (above (fixture, named (fixture, "recording-heading"),
                        named (fixture, "app-input-OBS Studio")));
  /* OBS stops recording: no heading, and its row is gone. */
  status.n_apps = 2;
  sd_popup_update (fixture->popup, &status);
  g_assert_false (widget_shows (fixture, named (fixture, "recording-heading")));
  g_assert_false (shows (fixture, "OBS Studio"));
}

static void
test_the_name_opens_the_devices_and_the_streams (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdApp apps[3];
  SdStatus status = with_apps (apps);

  sd_popup_update (fixture->popup, &status);
  g_assert_false (shows (fixture, "Plays to"));
  gtk_button_clicked (GTK_BUTTON (named (fixture, "app-output-mpv-name")));
  g_assert_true (shows (fixture, "Plays to"));
  g_assert_true (shows (fixture, "Follow the default (WH-1000XM4)"));
  g_assert_true (widget_shows (fixture, named (fixture, "app-output-mpv-check-follow")));
  g_assert_true (widget_shows (fixture, named (fixture, "app-output-mpv-device-alsa")));
  g_assert_false (widget_shows (fixture, named (fixture, "app-output-mpv-check-alsa")));
  /* A slider for each of its two streams, by its title. */
  g_assert_true (shows (fixture, "song.flac"));
  g_assert_true (shows (fixture, "talk.webm"));
  g_assert_cmpfloat (value_of (fixture, "app-output-mpv-stream-41"), ==, 40);
  gtk_range_set_value (GTK_RANGE (named (fixture, "app-output-mpv-stream-41")), 50);
  g_assert_cmpstr (fixture->calls->str, ==, "stream output 41 50;");
  /* Firefox's stays closed, and has one stream: no stream sliders when it opens. */
  gtk_button_clicked (GTK_BUTTON (named (fixture, "app-output-Firefox-name")));
  g_assert_false (shows (fixture, "A video"));
  /* A recording application records from. */
  gtk_button_clicked (GTK_BUTTON (named (fixture, "app-input-OBS Studio-name")));
  g_assert_true (shows (fixture, "Records from"));
  g_assert_true (shows (fixture, "Follow the default (Yeti Stereo Microphone)"));
  g_assert_true (widget_shows (fixture, named (fixture, "app-input-OBS Studio-device-mic")));
  /* The name again closes it. */
  gtk_button_clicked (GTK_BUTTON (named (fixture, "app-output-mpv-name")));
  g_assert_false (shows (fixture, "song.flac"));
}

static void
test_choosing_a_device_moves_the_app (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdApp apps[3];
  SdStatus status = with_apps (apps);
  GtkWidget *badge;

  sd_popup_update (fixture->popup, &status);
  gtk_button_clicked (GTK_BUTTON (named (fixture, "app-output-mpv-name")));
  gtk_button_clicked (GTK_BUTTON (named (fixture, "app-output-mpv-device-alsa")));
  g_assert_cmpstr (fixture->calls->str, ==, "move output mpv alsa;");
  /* It shows what the server makes of it: moved, with its device as a badge. */
  g_assert_false (widget_shows (fixture, named (fixture, "app-output-mpv-badge")));
  apps[0].device = "alsa";
  apps[0].device_description = "Built-in Audio";
  sd_popup_update (fixture->popup, &status);
  badge = named (fixture, "app-output-mpv-badge");
  g_assert_true (widget_shows (fixture, badge));
  g_assert_cmpstr (gtk_label_get_text (GTK_LABEL (badge)), ==, "Built-in Audio");
  g_assert_true (widget_shows (fixture, named (fixture, "app-output-mpv-check-alsa")));
  g_assert_false (widget_shows (fixture, named (fixture, "app-output-mpv-check-follow")));
  /* And back to following the default. */
  gtk_button_clicked (GTK_BUTTON (named (fixture, "app-output-mpv-follow")));
  g_assert_cmpstr (fixture->calls->str, ==, "move output mpv alsa;move output mpv default;");
  apps[0].device = NULL;
  apps[0].device_description = NULL;
  sd_popup_update (fixture->popup, &status);
  g_assert_false (widget_shows (fixture, badge));
  /* Recording an output's monitor, which no list has: the server's description. */
  apps[2].device = "alsa.monitor";
  apps[2].device_description = "Monitor of Built-in Audio";
  sd_popup_update (fixture->popup, &status);
  g_assert_cmpstr (gtk_label_get_text (GTK_LABEL (named (fixture, "app-input-OBS Studio-badge"))),
                   ==, "Monitor of Built-in Audio");
  apps[2].device = NULL;
  apps[2].device_description = NULL;
  /* The default changed: the item names the new one. */
  status.default_output = "easyeffects";
  sd_popup_update (fixture->popup, &status);
  g_assert_true (shows (fixture, "Follow the default (Easy Effects Sink)"));
}

static void
test_an_update_keeps_the_rows_and_their_state (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdApp apps[3];
  SdStatus status = with_apps (apps);
  GtkWidget *mpv;

  sd_popup_update (fixture->popup, &status);
  mpv = named (fixture, "app-output-mpv");
  gtk_button_clicked (GTK_BUTTON (named (fixture, "app-output-mpv-name")));
  /* A new application before it, and the volume changed: the same row, still open. */
  apps[2] = apps[1];
  apps[1] = apps[0];
  apps[0] = (SdApp) { .id = "Discord", .name = "Discord", .side = SD_OUTPUT,
                      .volume = PERCENT (50), .streams = firefox_streams, .n_streams = 1 };
  apps[1].volume = PERCENT (30);
  sd_popup_update (fixture->popup, &status);
  g_assert_true (named (fixture, "app-output-mpv") == mpv);
  g_assert_true (shows (fixture, "song.flac"));
  g_assert_cmpfloat (value_of (fixture, "app-output-mpv-scale"), ==, 30);
  g_assert_true (above (fixture, named (fixture, "app-output-Discord"), mpv));
  /* Without an icon of its own, a generic one. */
  g_assert_cmpstr (first_icon_name (named (fixture, "app-output-Discord-icon")), ==,
                   "application-x-executable");
  g_assert_cmpstr (fixture->calls->str, ==, "");
}

static void
test_the_activity_bars_show_the_levels (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdApp apps[3];
  SdStatus status = with_apps (apps);
  GtkWidget *bar;
  gint64 end;

  sd_popup_update (fixture->popup, &status);
  bar = named (fixture, "app-output-mpv-bar");
  sd_popup_set_app_level (fixture->popup, SD_OUTPUT, "mpv", 0.6);
  /* Unknown to the popup: nothing. */
  sd_popup_set_app_level (fixture->popup, SD_OUTPUT, "Spotify", 0.9);
  settle (fixture);
  g_assert_cmpfloat_with_epsilon (gtk_level_bar_get_value (GTK_LEVEL_BAR (bar)), 0.6, 1e-9);
  g_assert_cmpfloat (gtk_level_bar_get_value (GTK_LEVEL_BAR (named (fixture,
                                                                    "app-output-Firefox-bar"))),
                     ==, 0);
  /* Closed, the bars go quiet: the reader stops hearing them. Opened again, the next frame
     shows it. */
  end = g_get_monotonic_time () + WAIT_LIMIT_USEC;
  gtk_widget_hide (window_of (fixture));
  /* The unmap's own event, which would close the popup shown again. */
  gdk_display_sync (gtk_widget_get_display (bar));
  while (gtk_events_pending ())
    gtk_main_iteration ();
  gtk_widget_show (window_of (fixture));
  while (gtk_level_bar_get_value (GTK_LEVEL_BAR (bar)) != 0)
    {
      if (g_get_monotonic_time () > end)
        g_error ("timed out waiting for the bar to go quiet");
      g_main_context_iteration (NULL, FALSE);
      g_usleep (1000);
    }
}

static void
test_the_keyboard_reaches_the_applications (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  SdApp apps[3];
  SdStatus status = with_apps (apps);
  g_autofree char *order = NULL;

  sd_popup_update (fixture->popup, &status);
  settle (fixture);
  order = tab_order (fixture);
  g_assert_cmpstr (order, ==,
                   "output-mute output-scale output-pill input-mute input-scale input-pill "
                   "system-scale app-output-mpv-name app-output-mpv-scale "
                   "app-output-Firefox-name app-output-Firefox-scale "
                   "app-input-OBS Studio-name app-input-OBS Studio-scale settings ");
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  if (!gtk_init_check (&argc, &argv))
    {
      g_test_skip ("no display");
      return g_test_run ();
    }
  g_object_set (gtk_settings_get_default (), "gtk-enable-animations", FALSE, NULL);

#define ADD(path, test) g_test_add ("/popup/" path, Fixture, NULL, setup, test, teardown)
  ADD ("no-sound-server-is-said-over-the-footer", test_no_sound_server_is_said_over_the_footer);
  ADD ("the-rows-show-the-defaults", test_the_rows_show_the_defaults);
  ADD ("the-pill-opens-the-list-with-the-default-checked",
       test_the_pill_opens_the_list_with_the_default_checked);
  ADD ("choosing-a-device-asks-for-it-as-the-default",
       test_choosing_a_device_asks_for_it_as_the_default);
  ADD ("the-port-dropdown-needs-two-plugged-ports", test_the_port_dropdown_needs_two_plugged_ports);
  ADD ("the-mute-buttons-ask-for-the-other-state", test_the_mute_buttons_ask_for_the_other_state);
  ADD ("only-the-input-has-a-meter", test_only_the_input_has_a_meter);
  ADD ("the-sliders-stop-at-100-or-150-percent", test_the_sliders_stop_at_100_or_150_percent);
  ADD ("a-slider-follows-the-server-unless-dragged",
       test_a_slider_follows_the_server_unless_dragged);
  ADD ("a-stale-volume-doesnt-undo-a-step", test_a_stale_volume_doesnt_undo_a_step);
  ADD ("the-plain-wheel-scrolls-and-shift-moves-the-slider",
       test_the_plain_wheel_scrolls_and_shift_moves_the_slider);
  ADD ("a-short-room-scrolls-with-the-footer-shown",
       test_a_short_room_scrolls_with_the_footer_shown);
  ADD ("an-update-keeps-the-scroll-position", test_an_update_keeps_the_scroll_position);
  ADD ("the-keyboard-reaches-every-control", test_the_keyboard_reaches_every_control);
  ADD ("each-opening-starts-the-keyboard-at-the-top",
       test_each_opening_starts_the_keyboard_at_the_top);
  ADD ("sound-settings-closes-the-popup-and-calls-back",
       test_sound_settings_closes_the_popup_and_calls_back);
  ADD ("system-sounds-is-the-first-application", test_system_sounds_is_the_first_application);
  ADD ("an-apps-row-shows-its-loudest-stream", test_an_apps_row_shows_its_loudest_stream);
  ADD ("the-recording-applications-have-a-heading",
       test_the_recording_applications_have_a_heading);
  ADD ("the-name-opens-the-devices-and-the-streams",
       test_the_name_opens_the_devices_and_the_streams);
  ADD ("choosing-a-device-moves-the-app", test_choosing_a_device_moves_the_app);
  ADD ("an-update-keeps-the-rows-and-their-state", test_an_update_keeps_the_rows_and_their_state);
  ADD ("the-activity-bars-show-the-levels", test_the_activity_bars_show_the_levels);
  ADD ("the-keyboard-reaches-the-applications", test_the_keyboard_reaches_the_applications);
#undef ADD
  return g_test_run ();
}
