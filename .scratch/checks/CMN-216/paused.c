/* A paused Sound application for CMN-216's check: a client named as argv[1] whose stream
   starts corked, which pw-cat can't make. It runs until it's killed. */

#include <pulse/pulseaudio.h>
#include <stdio.h>

static void
on_state (pa_context *context, void *data)
{
  pa_stream **stream = data;
  pa_sample_spec spec = { .format = PA_SAMPLE_S16LE, .rate = 48000, .channels = 2 };

  if (pa_context_get_state (context) != PA_CONTEXT_READY || *stream != NULL)
    return;
  *stream = pa_stream_new (context, "paused song", &spec, NULL);
  if (*stream == NULL
      || pa_stream_connect_playback (*stream, NULL, NULL, PA_STREAM_START_CORKED, NULL, NULL) < 0)
    fprintf (stderr, "can't open the paused stream\n");
}

int
main (int argc, char **argv)
{
  pa_mainloop *loop = pa_mainloop_new ();
  pa_proplist *properties = pa_proplist_new ();
  pa_context *context;
  pa_stream *stream = NULL;

  pa_proplist_sets (properties, PA_PROP_APPLICATION_NAME, argc > 1 ? argv[1] : "Paused");
  context = pa_context_new_with_proplist (pa_mainloop_get_api (loop), NULL, properties);
  pa_context_set_state_callback (context, on_state, &stream);
  pa_context_connect (context, NULL, PA_CONTEXT_NOAUTOSPAWN, NULL);
  return pa_mainloop_run (loop, NULL);
}
