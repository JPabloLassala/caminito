/*
 * The sound server's reader (CMN-213) against a private PipeWire, WirePlumber and
 * pipewire-pulse, in a throwaway XDG_RUNTIME_DIR on a private session bus, with null sinks
 * and sources for sound devices and pw-record for an application recording. pactl sets up
 * the devices and reads back what the reader set. Skipped where PipeWire isn't installed.
 */

#include <glib/gstdio.h>
#include <gio/gio.h>

#include "sd-pulse.h"

#define PERCENT(p) ((pa_volume_t) (((guint64) (p) * PA_VOLUME_NORM + 50) / 100))

/* WirePlumber on null devices alone: none of the computer's own. */
static const char wireplumber_conf[] =
  "wireplumber.profiles = { main = { monitor.alsa = disabled, monitor.alsa-midi = disabled,"
  " monitor.bluez = disabled, monitor.bluez-midi = disabled, monitor.v4l2 = disabled,"
  " monitor.libcamera = disabled } }\n";

static char *runtime_dir;

typedef struct
{
  GSubprocess *pipewire;
  GSubprocess *wireplumber;
  GSubprocess *pulse;
} Server;

typedef struct
{
  SdPulse *pulse;
  guint    changes;
} Fixture;

static gboolean
on_tick (gpointer data)
{
  *(gboolean *) data = TRUE;
  return G_SOURCE_REMOVE;
}

/* Runs the main loop until cond holds, or fails after 15 seconds. */
static void
wait_until (gboolean (*cond) (gpointer data), gpointer data, const char *what)
{
  gint64 deadline = g_get_monotonic_time () + 15 * G_USEC_PER_SEC;

  while (!cond (data))
    {
      gboolean ticked = FALSE;
      guint tick;

      if (g_get_monotonic_time () > deadline)
        g_error ("timed out waiting for %s", what);
      /* The server's own changes wake the loop; the tick is for a socket appearing. */
      tick = g_timeout_add (50, on_tick, &ticked);
      while (!ticked && !cond (data))
        g_main_context_iteration (NULL, TRUE);
      if (!ticked)
        g_source_remove (tick);
    }
}

static GSubprocess *
spawn (const char *program)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GSubprocessLauncher) launcher =
    g_subprocess_launcher_new (G_SUBPROCESS_FLAGS_STDOUT_SILENCE | G_SUBPROCESS_FLAGS_STDERR_SILENCE);
  GSubprocess *process = g_subprocess_launcher_spawn (launcher, &error, program, NULL);

  g_assert_no_error (error);
  return process;
}

static gboolean
socket_exists (gpointer data)
{
  g_autofree char *path = g_build_filename (runtime_dir, data, NULL);

  return g_file_test (path, G_FILE_TEST_EXISTS);
}

/* pactl's output, failing the test if pactl does. */
static char *
pactl (const char *command)
{
  g_autofree char *line = g_strconcat ("pactl ", command, NULL);
  g_auto (GStrv) argv = NULL;
  g_autoptr (GError) error = NULL;
  char *out = NULL;
  int status;

  g_assert_true (g_shell_parse_argv (line, NULL, &argv, NULL));
  g_spawn_sync (NULL, argv, NULL, G_SPAWN_SEARCH_PATH | G_SPAWN_STDERR_TO_DEV_NULL, NULL, NULL,
                &out, NULL, &status, &error);
  g_assert_no_error (error);
  g_assert_true (g_spawn_check_wait_status (status, NULL));
  return out;
}

static gboolean
pulse_answers (gpointer data G_GNUC_UNUSED)
{
  const char *const argv[] = { "pactl", "info", NULL };
  int status;

  return g_spawn_sync (NULL, (char **) argv, NULL,
                       G_SPAWN_SEARCH_PATH | G_SPAWN_STDOUT_TO_DEV_NULL
                       | G_SPAWN_STDERR_TO_DEV_NULL,
                       NULL, NULL, NULL, NULL, &status, NULL)
         && g_spawn_check_wait_status (status, NULL);
}

static void
server_start (Server *server)
{
  server->pipewire = spawn ("pipewire");
  wait_until (socket_exists, "pipewire-0", "PipeWire's socket");
  server->wireplumber = spawn ("wireplumber");
  server->pulse = spawn ("pipewire-pulse");
  wait_until (pulse_answers, NULL, "pipewire-pulse");
}

static void
stop (GSubprocess **process)
{
  if (*process == NULL)
    return;
  g_subprocess_send_signal (*process, SIGTERM);
  g_subprocess_wait (*process, NULL, NULL);
  g_clear_object (process);
}

static void
server_stop (Server *server)
{
  stop (&server->pulse);
  stop (&server->wireplumber);
  stop (&server->pipewire);
}

static void
on_changed (SdPulse *pulse G_GNUC_UNUSED, gpointer data)
{
  Fixture *fixture = data;

  fixture->changes++;
}

static const SdStatus *
status_of (Fixture *fixture)
{
  return sd_pulse_status (fixture->pulse);
}

static gboolean
running (gpointer data)
{
  return status_of (data)->running;
}

static gboolean
stopped (gpointer data)
{
  return !status_of (data)->running;
}

static void
load_sink (const char *name, const char *properties)
{
  g_autofree char *command = g_strdup_printf ("load-module module-null-sink sink_name=%s "
                                              "'sink_properties=%s'",
                                              name, properties);
  g_autofree char *out = pactl (command);
}

static void
set_default (const char *what, const char *name)
{
  g_autofree char *command = g_strdup_printf ("set-default-%s %s", what, name);
  g_autofree char *out = pactl (command);
}

static void
fixture_set_up (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  fixture->pulse = sd_pulse_new (on_changed, fixture);
}

static void
fixture_tear_down (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  sd_pulse_free (fixture->pulse);
}

static void
test_waits_for_the_sound_server (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  Server server = { 0 };

  g_assert_false (status_of (fixture)->running);
  server_start (&server);
  wait_until (running, fixture, "the reader to connect");
  server_stop (&server);
}

static void
test_reconnects_when_the_server_comes_back (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  Server server = { 0 };

  server_start (&server);
  wait_until (running, fixture, "the reader to connect");
  server_stop (&server);
  wait_until (stopped, fixture, "the reader to see the server go");
  server_start (&server);
  wait_until (running, fixture, "the reader to connect again");
  server_stop (&server);
}

static gboolean
output_is_speakers (gpointer data)
{
  const SdStatus *status = status_of (data);

  return status->has_output && g_strcmp0 (status->output_name, "Speakers") == 0;
}

static gboolean
output_is_headset (gpointer data)
{
  const SdStatus *status = status_of (data);

  return status->has_output && g_strcmp0 (status->output_name, "WH-1000XM4") == 0;
}

static void
test_follows_the_default_output (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  Server server = { 0 };

  server_start (&server);
  load_sink ("speaker", "device.description=Speakers");
  load_sink ("headset", "device.description=WH-1000XM4 device.form_factor=headset");
  set_default ("sink", "speaker");
  wait_until (output_is_speakers, fixture, "the speakers");
  g_assert_false (status_of (fixture)->headphones);
  g_assert_cmpuint (status_of (fixture)->volume, ==, PA_VOLUME_NORM);
  g_assert_false (status_of (fixture)->muted);
  set_default ("sink", "headset");
  wait_until (output_is_headset, fixture, "the headset");
  g_assert_true (status_of (fixture)->headphones);
  server_stop (&server);
}

static gboolean
at_45_percent_and_muted (gpointer data)
{
  const SdStatus *status = status_of (data);

  return status->volume == PERCENT (45) && status->muted;
}

static void
test_sets_the_volume_and_mute_on_the_server (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  Server server = { 0 };
  g_autofree char *volume = NULL;
  g_autofree char *mute = NULL;

  server_start (&server);
  load_sink ("speaker", "device.description=Speakers");
  set_default ("sink", "speaker");
  wait_until (output_is_speakers, fixture, "the speakers");
  sd_pulse_set_volume (fixture->pulse, PERCENT (45));
  sd_pulse_set_muted (fixture->pulse, TRUE);
  wait_until (at_45_percent_and_muted, fixture, "45% and muted");
  volume = pactl ("get-sink-volume speaker");
  mute = pactl ("get-sink-mute speaker");
  g_assert_nonnull (strstr (volume, "/  45% /"));
  g_assert_cmpstr (mute, ==, "Mute: yes\n");
  server_stop (&server);
}

static gboolean
input_muted (gpointer data)
{
  const SdStatus *status = status_of (data);

  return status->has_input && status->input_muted;
}

static gboolean
has_input (gpointer data)
{
  return status_of (data)->has_input;
}

static void
test_mutes_the_default_input (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  Server server = { 0 };
  g_autofree char *mute = NULL;

  server_start (&server);
  load_sink ("mic", "media.class=Audio/Source/Virtual device.description=Microphone");
  set_default ("source", "mic");
  wait_until (has_input, fixture, "the microphone");
  sd_pulse_set_input_muted (fixture->pulse, TRUE);
  wait_until (input_muted, fixture, "the microphone muted");
  mute = pactl ("get-source-mute mic");
  g_assert_cmpstr (mute, ==, "Mute: yes\n");
  server_stop (&server);
}

static void
test_an_outputs_monitor_isnt_a_microphone (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  Server server = { 0 };

  server_start (&server);
  load_sink ("speaker", "device.description=Speakers");
  set_default ("sink", "speaker");
  set_default ("source", "speaker.monitor");
  wait_until (output_is_speakers, fixture, "the speakers");
  g_assert_false (status_of (fixture)->has_input);
  server_stop (&server);
}

static gboolean
obs_records (gpointer data)
{
  char **recording = status_of (data)->recording;

  return recording != NULL && g_strcmp0 (recording[0], "OBS Studio") == 0
         && recording[1] == NULL;
}

static gboolean
nobody_records (gpointer data)
{
  return status_of (data)->running && status_of (data)->recording == NULL;
}

static void
test_names_the_applications_recording (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  Server server = { 0 };
  g_autoptr (GError) error = NULL;
  g_autofree char *file = g_build_filename (runtime_dir, "recording.wav", NULL);
  g_autoptr (GSubprocess) recorder = NULL;

  server_start (&server);
  load_sink ("mic", "media.class=Audio/Source/Virtual device.description=Microphone");
  set_default ("source", "mic");
  wait_until (has_input, fixture, "the microphone");
  g_assert_null (status_of (fixture)->recording);
  recorder = g_subprocess_new (G_SUBPROCESS_FLAGS_STDERR_SILENCE, &error, "pw-record", "-P",
                               "{ application.name = \"OBS Studio\" }", file, NULL);
  g_assert_no_error (error);
  wait_until (obs_records, fixture, "OBS Studio recording");
  stop (&recorder);
  wait_until (nobody_records, fixture, "the recording to end");
  server_stop (&server);
}

int
main (int argc, char **argv)
{
  g_autofree char *root = NULL;
  g_autofree char *config_dir = NULL;
  g_autofree char *state_dir = NULL;
  g_autofree char *conf_dir = NULL;
  g_autofree char *conf = NULL;
  GTestDBus *bus;
  int result;

  g_test_init (&argc, &argv, NULL);
  if (g_find_program_in_path ("pipewire") == NULL
      || g_find_program_in_path ("wireplumber") == NULL
      || g_find_program_in_path ("pipewire-pulse") == NULL
      || g_find_program_in_path ("pactl") == NULL
      || g_find_program_in_path ("pw-record") == NULL)
    {
      g_test_skip ("PipeWire, WirePlumber, pipewire-pulse and pactl aren't all installed");
      return g_test_run ();
    }
  root = g_dir_make_tmp ("caminito-sound-XXXXXX", NULL);
  runtime_dir = g_build_filename (root, "run", NULL);
  config_dir = g_build_filename (root, "config", NULL);
  state_dir = g_build_filename (root, "state", NULL);
  conf_dir = g_build_filename (config_dir, "wireplumber", "wireplumber.conf.d", NULL);
  g_mkdir_with_parents (runtime_dir, 0700);
  g_mkdir_with_parents (conf_dir, 0700);
  conf = g_build_filename (conf_dir, "50-null-devices.conf", NULL);
  g_file_set_contents (conf, wireplumber_conf, -1, NULL);
  bus = g_test_dbus_new (G_TEST_DBUS_NONE);
  g_test_dbus_up (bus);
  /* Nothing of the user's own server, settings or state. After the bus, which unsets
     XDG_RUNTIME_DIR. */
  g_setenv ("XDG_RUNTIME_DIR", runtime_dir, TRUE);
  g_setenv ("XDG_CONFIG_HOME", config_dir, TRUE);
  g_setenv ("XDG_STATE_HOME", state_dir, TRUE);
  g_unsetenv ("PULSE_SERVER");
  g_unsetenv ("PULSE_RUNTIME_PATH");
  g_unsetenv ("PIPEWIRE_REMOTE");

#define ADD(path, test) \
  g_test_add ("/pulse/" path, Fixture, NULL, fixture_set_up, test, fixture_tear_down)
  ADD ("waits-for-the-sound-server", test_waits_for_the_sound_server);
  ADD ("reconnects-when-the-server-comes-back", test_reconnects_when_the_server_comes_back);
  ADD ("follows-the-default-output", test_follows_the_default_output);
  ADD ("sets-the-volume-and-mute-on-the-server", test_sets_the_volume_and_mute_on_the_server);
  ADD ("mutes-the-default-input", test_mutes_the_default_input);
  ADD ("an-outputs-monitor-isnt-a-microphone", test_an_outputs_monitor_isnt_a_microphone);
  ADD ("names-the-applications-recording", test_names_the_applications_recording);
#undef ADD
  result = g_test_run ();

  g_test_dbus_down (bus);
  g_object_unref (bus);
  {
    const char *const rm[] = { "rm", "-rf", root, NULL };

    g_spawn_sync (NULL, (char **) rm, NULL, G_SPAWN_SEARCH_PATH, NULL, NULL, NULL, NULL, NULL,
                  NULL);
  }
  return result;
}
