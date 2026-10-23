/* CMN-209: how the Sound applet would apply a pin, timed.
 *
 *   pin-probe APP SINK-NAME [DELAY-MS]
 *
 * Watches sink-input events through libpulse on the GLib main loop, as the
 * applet does. When a sink input of the application APP appears, it moves it
 * to the sink SINK-NAME with pa_context_move_sink_input_by_index, and prints
 * when the event came and when the move was acknowledged, in WirePlumber's
 * log format (local time), so the two logs line up. With DELAY-MS, it waits
 * that long after the event before it asks for the sink input, to stand for a
 * busy applet. Runs until killed.
 *
 *   cc -o pin-probe pin-probe.c $(pkg-config --cflags --libs libpulse-mainloop-glib glib-2.0)
 */

#include <glib.h>
#include <pulse/glib-mainloop.h>
#include <pulse/pulseaudio.h>
#include <stdio.h>
#include <string.h>

static const char *app_name;
static const char *sink_name;
static uint32_t sink_index = PA_INVALID_INDEX;
static guint delay_ms;

static void
print_time (const char *what, uint32_t idx)
{
  GDateTime *now = g_date_time_new_now_local ();
  printf ("%02d:%02d:%09.6f %s %u\n", g_date_time_get_hour (now),
          g_date_time_get_minute (now), g_date_time_get_seconds (now), what,
          idx);
  fflush (stdout);
  g_date_time_unref (now);
}

static void
on_moved (pa_context *c, int success, void *userdata)
{
  uint32_t idx = GPOINTER_TO_UINT (userdata);
  print_time (success ? "move-acked" : "move-failed", idx);
}

static void
on_sink_input (pa_context *c, const pa_sink_input_info *i, int eol,
               void *userdata)
{
  if (eol || i == NULL)
    return;
  const char *name = pa_proplist_gets (i->proplist, PA_PROP_APPLICATION_NAME);
  if (name == NULL || strcmp (name, app_name) != 0 || i->sink == sink_index)
    return;
  print_time ("move-sent", i->index);
  pa_operation_unref (pa_context_move_sink_input_by_index (
      c, i->index, sink_index, on_moved, GUINT_TO_POINTER (i->index)));
}

static pa_context *context;

static gboolean
look_up (gpointer userdata)
{
  pa_operation_unref (pa_context_get_sink_input_info (
      context, GPOINTER_TO_UINT (userdata), on_sink_input, NULL));
  return G_SOURCE_REMOVE;
}

static void
on_event (pa_context *c, pa_subscription_event_type_t t, uint32_t idx,
          void *userdata)
{
  if ((t & PA_SUBSCRIPTION_EVENT_FACILITY_MASK)
          != PA_SUBSCRIPTION_EVENT_SINK_INPUT
      || (t & PA_SUBSCRIPTION_EVENT_TYPE_MASK) != PA_SUBSCRIPTION_EVENT_NEW)
    return;
  print_time ("sink-input-new", idx);
  if (delay_ms > 0)
    g_timeout_add (delay_ms, look_up, GUINT_TO_POINTER (idx));
  else
    look_up (GUINT_TO_POINTER (idx));
}

static void
on_sink (pa_context *c, const pa_sink_info *i, int eol, void *userdata)
{
  if (eol || i == NULL)
    return;
  sink_index = i->index;
  pa_context_set_subscribe_callback (c, on_event, NULL);
  pa_operation_unref (
      pa_context_subscribe (c, PA_SUBSCRIPTION_MASK_SINK_INPUT, NULL, NULL));
  print_time ("ready, sink", sink_index);
}

static void
on_state (pa_context *c, void *userdata)
{
  switch (pa_context_get_state (c))
    {
    case PA_CONTEXT_READY:
      pa_operation_unref (
          pa_context_get_sink_info_by_name (c, sink_name, on_sink, NULL));
      break;
    case PA_CONTEXT_FAILED:
    case PA_CONTEXT_TERMINATED:
      g_main_loop_quit (userdata);
      break;
    default:
      break;
    }
}

int
main (int argc, char **argv)
{
  if (argc != 3 && argc != 4)
    {
      fprintf (stderr, "usage: %s APP SINK-NAME [DELAY-MS]\n", argv[0]);
      return 2;
    }
  app_name = argv[1];
  sink_name = argv[2];
  delay_ms = argc == 4 ? (guint) g_ascii_strtoull (argv[3], NULL, 10) : 0;

  GMainLoop *loop = g_main_loop_new (NULL, FALSE);
  pa_glib_mainloop *ml = pa_glib_mainloop_new (NULL);
  context = pa_context_new (pa_glib_mainloop_get_api (ml), "pin-probe");
  pa_context_set_state_callback (context, on_state, loop);
  pa_context_connect (context, NULL, PA_CONTEXT_NOFLAGS, NULL);
  g_main_loop_run (loop);
  return 1;
}
