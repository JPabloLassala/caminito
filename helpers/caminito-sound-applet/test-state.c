/*
 * The icon's names, its strike, the tooltip, the scroll's volume and the recording names
 * (CMN-213), from statuses built by hand.
 */

#include "sd-state.h"

/* Rounded, as pactl reads it back. */
#define PERCENT(p) ((pa_volume_t) (((guint64) (p) * PA_VOLUME_NORM + 50) / 100))

static SdStatus
playing (const char *name, guint percent)
{
  return (SdStatus) {
    .running = TRUE,
    .has_output = TRUE,
    .output_name = (char *) name,
    .volume = PERCENT (percent),
  };
}

static void
test_no_sound_server (void)
{
  SdStatus status = { .running = FALSE };
  const char *const *names = sd_state_icon_names (&status);
  g_autofree char *tooltip = sd_state_tooltip (&status);

  /* The speaker with no waves, MATE's, then the lowest level that the others have, struck
     through by the applet. */
  g_assert_cmpstr (names[0], ==, "audio-volume-off");
  g_assert_cmpstr (names[1], ==, "audio-volume-low-symbolic");
  g_assert_null (names[2]);
  g_assert_true (sd_state_icon_struck (&status));
  g_assert_cmpstr (tooltip, ==, "No sound server");
}

static void
test_speaker_levels (void)
{
  struct { guint percent; const char *name; } cases[] = {
    { 1, "audio-volume-low" },
    { 33, "audio-volume-low" },
    { 34, "audio-volume-medium" },
    { 66, "audio-volume-medium" },
    { 67, "audio-volume-high" },
    { 100, "audio-volume-high" },
    { 150, "audio-volume-high" },
  };

  for (guint i = 0; i < G_N_ELEMENTS (cases); i++)
    {
      SdStatus status = playing ("Speakers", cases[i].percent);
      const char *const *names = sd_state_icon_names (&status);
      g_autofree char *symbolic = g_strconcat (cases[i].name, "-symbolic", NULL);

      g_assert_cmpstr (names[0], ==, cases[i].name);
      g_assert_cmpstr (names[1], ==, symbolic);
      g_assert_null (names[2]);
      g_assert_false (sd_state_icon_struck (&status));
    }
}

static void
test_speaker_muted_or_at_zero (void)
{
  SdStatus muted = playing ("Speakers", 45);
  SdStatus zero = playing ("Speakers", 0);
  g_autofree char *tooltip = NULL;

  muted.muted = TRUE;
  g_assert_cmpstr (sd_state_icon_names (&muted)[0], ==, "audio-volume-muted");
  g_assert_cmpstr (sd_state_icon_names (&muted)[1], ==, "audio-volume-muted-symbolic");
  g_assert_cmpstr (sd_state_icon_names (&zero)[0], ==, "audio-volume-muted");
  g_assert_false (sd_state_icon_struck (&muted));
  tooltip = sd_state_tooltip (&muted);
  g_assert_cmpstr (tooltip, ==, "Speakers: 45% (muted)");
}

static void
test_headphones (void)
{
  SdStatus status = playing ("WH-1000XM4", 45);
  g_autofree char *tooltip = NULL;

  status.headphones = TRUE;
  g_assert_cmpstr (sd_state_icon_names (&status)[0], ==, "audio-headphones");
  g_assert_cmpstr (sd_state_icon_names (&status)[1], ==, "audio-headphones-symbolic");
  g_assert_null (sd_state_icon_names (&status)[2]);
  g_assert_false (sd_state_icon_struck (&status));
  tooltip = sd_state_tooltip (&status);
  g_assert_cmpstr (tooltip, ==, "WH-1000XM4: 45%");
}

static void
test_headphones_muted_are_struck_out (void)
{
  SdStatus muted = playing ("WH-1000XM4", 45);
  SdStatus zero = playing ("WH-1000XM4", 0);

  muted.headphones = TRUE;
  muted.muted = TRUE;
  zero.headphones = TRUE;
  /* Still the headphones, which only the strike tells apart. */
  g_assert_cmpstr (sd_state_icon_names (&muted)[0], ==, "audio-headphones");
  g_assert_true (sd_state_icon_struck (&muted));
  /* At 0 they aren't muted: they're as loud as the user set them. */
  g_assert_false (sd_state_icon_struck (&zero));
}

static void
test_no_output (void)
{
  SdStatus status = { .running = TRUE };
  g_autofree char *tooltip = sd_state_tooltip (&status);

  g_assert_cmpstr (sd_state_icon_names (&status)[0], ==, "audio-volume-muted");
  g_assert_cmpstr (tooltip, ==, "No sound device");
}

static void
test_tooltip_rounds_as_pactl_does (void)
{
  /* pactl's 45% for 29491. */
  SdStatus status = playing ("Speakers", 0);
  g_autofree char *tooltip = NULL;

  status.volume = 29491;
  tooltip = sd_state_tooltip (&status);
  g_assert_cmpstr (tooltip, ==, "Speakers: 45%");
}

static void
test_tooltip_names_who_records (void)
{
  char *recording[] = { "Discord", "OBS Studio", NULL };
  SdStatus status = playing ("WH-1000XM4", 45);
  SdStatus stopped = { .running = FALSE, .recording = recording };
  g_autofree char *tooltip = NULL;
  g_autofree char *stopped_tooltip = NULL;

  status.recording = recording;
  tooltip = sd_state_tooltip (&status);
  g_assert_cmpstr (tooltip, ==, "WH-1000XM4: 45%\nRecording: Discord, OBS Studio");
  stopped_tooltip = sd_state_tooltip (&stopped);
  g_assert_cmpstr (stopped_tooltip, ==, "No sound server");
}

static void
test_scroll_steps (void)
{
  g_assert_cmpuint (sd_state_scroll_volume (PERCENT (45), 5, TRUE, FALSE), ==, PERCENT (50));
  g_assert_cmpuint (sd_state_scroll_volume (PERCENT (45), 5, FALSE, FALSE), ==, PERCENT (40));
  g_assert_cmpuint (sd_state_scroll_volume (PERCENT (45), 10, TRUE, FALSE), ==, PERCENT (55));
  g_assert_cmpuint (sd_state_scroll_volume (PERCENT (3), 5, FALSE, FALSE), ==, 0);
  g_assert_cmpuint (sd_state_scroll_volume (0, 5, FALSE, FALSE), ==, 0);
}

static void
test_scroll_stops_at_100_percent (void)
{
  g_assert_cmpuint (sd_state_scroll_volume (PERCENT (98), 5, TRUE, FALSE), ==, PA_VOLUME_NORM);
  g_assert_cmpuint (sd_state_scroll_volume (PA_VOLUME_NORM, 5, TRUE, FALSE), ==,
                    PA_VOLUME_NORM);
}

static void
test_scroll_goes_to_150_percent_with_the_gnome_key (void)
{
  g_assert_cmpuint (sd_state_scroll_volume (PA_VOLUME_NORM, 5, TRUE, TRUE), ==, PERCENT (105));
  g_assert_cmpuint (sd_state_scroll_volume (PERCENT (148), 5, TRUE, TRUE), ==, PERCENT (150));
  g_assert_cmpuint (sd_state_scroll_volume (PERCENT (150), 5, TRUE, TRUE), ==, PERCENT (150));
}

static void
test_scroll_up_past_the_cap_keeps_the_volume (void)
{
  /* Set to 120% elsewhere, with the key off. */
  g_assert_cmpuint (sd_state_scroll_volume (PERCENT (120), 5, TRUE, FALSE), ==, PERCENT (120));
  g_assert_cmpuint (sd_state_scroll_volume (PERCENT (120), 5, FALSE, FALSE), ==, PERCENT (115));
}

static void
test_recording_names_share_a_row_per_program (void)
{
  const SdStream streams[] = {
    { .name = "Discord", .binary = "electron" },
    { .name = "OBS Studio", .binary = "obs" },
    /* Another of Discord's streams. */
    { .name = "Discord", .binary = "electron" },
    /* Two Electron programs, apart by name and not by binary. */
    { .name = "Slack", .binary = "electron" },
    /* A Flatpak's, by its app ID whatever its streams call themselves. */
    { .app_id = "com.spotify.Client", .name = "Spotify", .binary = "spotify" },
    { .app_id = "com.spotify.Client", .name = "spotify", .binary = "spotify" },
    /* Nameless, by its binary. */
    { .binary = "arecord" },
  };
  g_auto (GStrv) names = sd_state_recording_names (streams, G_N_ELEMENTS (streams));
  const char *const expected[] = { "Discord", "OBS Studio", "Slack", "Spotify", "arecord", NULL };

  g_assert_cmpstrv (names, expected);
}

static void
test_recording_names_without_streams (void)
{
  g_assert_null (sd_state_recording_names (NULL, 0));
}

static void
test_apps_share_a_row_per_program (void)
{
  const SdStream streams[] = {
    /* Two copies of one program, whose loudest stream the row shows. */
    { .name = "mpv", .binary = "mpv", .icon = "mpv", .title = "song.flac", .index = 40,
      .device = "speaker", .volume = PERCENT (50) },
    { .name = "Firefox", .binary = "firefox", .icon = "firefox", .title = "A video", .index = 41,
      .device = "speaker", .volume = PERCENT (100) },
    { .name = "mpv", .binary = "mpv", .icon = "mpv", .title = "talk.webm", .index = 42,
      .device = "speaker", .volume = PERCENT (80) },
    /* Two Electron programs, apart by name and not by binary. */
    { .name = "Discord", .binary = "electron", .title = "playStream", .index = 43,
      .device = "speaker", .volume = PERCENT (90) },
    { .name = "Slack", .binary = "electron", .title = "playback", .index = 44,
      .device = "speaker", .volume = PERCENT (70) },
    /* A Flatpak's, by its app ID whatever its streams call themselves. */
    { .app_id = "com.spotify.Client", .name = "Spotify", .binary = "spotify", .title = "Spotify",
      .index = 45, .device = "headset", .device_description = "WH-1000XM4",
      .volume = PERCENT (60) },
    { .app_id = "com.spotify.Client", .name = "spotify", .binary = "spotify", .title = "ad",
      .index = 46, .device = "speaker", .volume = PERCENT (60) },
    /* Nameless, by its binary. */
    { .binary = "aplay", .index = 47, .device = "speaker", .volume = PERCENT (100) },
    /* No identity at all: left out. */
    { .index = 48, .device = "speaker", .volume = PERCENT (100) },
  };
  guint n = 0;
  SdApp *apps = sd_state_apps (streams, G_N_ELEMENTS (streams), SD_OUTPUT, "speaker", &n);

  g_assert_cmpuint (n, ==, 6);
  g_assert_cmpstr (apps[0].id, ==, "mpv");
  g_assert_cmpstr (apps[0].icon, ==, "mpv");
  g_assert_cmpint (apps[0].side, ==, SD_OUTPUT);
  g_assert_cmpuint (apps[0].n_streams, ==, 2);
  g_assert_cmpuint (apps[0].streams[0].index, ==, 40);
  g_assert_cmpstr (apps[0].streams[1].title, ==, "talk.webm");
  g_assert_cmpuint (apps[0].volume, ==, PERCENT (80));
  g_assert_null (apps[0].device);
  g_assert_cmpstr (apps[1].name, ==, "Firefox");
  g_assert_cmpstr (apps[2].name, ==, "Discord");
  g_assert_cmpstr (apps[3].name, ==, "Slack");
  g_assert_cmpstr (apps[4].id, ==, "com.spotify.Client");
  g_assert_cmpstr (apps[4].name, ==, "Spotify");
  g_assert_cmpuint (apps[4].n_streams, ==, 2);
  /* A stream off the default: the application is moved. */
  g_assert_cmpstr (apps[4].device, ==, "headset");
  g_assert_cmpstr (apps[4].device_description, ==, "WH-1000XM4");
  g_assert_null (apps[0].device_description);
  g_assert_cmpstr (apps[5].id, ==, "aplay");
  g_assert_cmpstr (apps[5].name, ==, "aplay");
  for (guint i = 0; i < n; i++)
    sd_app_clear (&apps[i]);
  g_free (apps);
  g_assert_null (sd_state_apps (NULL, 0, SD_INPUT, "mic", &n));
  g_assert_cmpuint (n, ==, 0);
}

static void
test_an_apps_streams_keep_their_ratio (void)
{
  /* In percent, as pactl shows the server's volumes. The loudest at 80%, another at 40%:
     the slider to 60% takes them to 60% and 30%. */
  g_assert_cmpuint (sd_state_percent (sd_state_scaled_volume (PERCENT (80), PERCENT (80),
                                                              PERCENT (60))), ==, 60);
  g_assert_cmpuint (sd_state_percent (sd_state_scaled_volume (PERCENT (40), PERCENT (80),
                                                              PERCENT (60))), ==, 30);
  /* Up past the loudest's own, too. */
  g_assert_cmpuint (sd_state_percent (sd_state_scaled_volume (PERCENT (40), PERCENT (80),
                                                              PERCENT (100))), ==, 50);
  /* Every stream silent has no ratio to keep. */
  g_assert_cmpuint (sd_state_scaled_volume (0, 0, PERCENT (25)), ==, PERCENT (25));
}

static void
test_apps_equal_compares_streams_and_devices (void)
{
  SdStreamVolume streams[] = { { 40, "song.flac", PERCENT (50) }, { 42, "talk.webm", PERCENT (80) } };
  SdStreamVolume louder[] = { { 40, "song.flac", PERCENT (50) }, { 42, "talk.webm", PERCENT (70) } };
  SdApp app = { .id = "mpv", .name = "mpv", .icon = "mpv", .side = SD_OUTPUT, .device = "headset",
                .device_description = "WH-1000XM4", .volume = PERCENT (80), .streams = streams,
                .n_streams = 2 };
  SdApp other = app;
  SdStatus a = { .running = TRUE, .apps = &app, .n_apps = 1 };
  SdStatus b = { .running = TRUE, .apps = &other, .n_apps = 1 };

  g_assert_true (sd_app_equal (&app, &other));
  g_assert_true (sd_status_equal (&a, &b));
  other.streams = louder;
  g_assert_false (sd_app_equal (&app, &other));
  g_assert_false (sd_status_equal (&a, &b));
  other = app;
  other.device_description = "Headset";
  g_assert_false (sd_app_equal (&app, &other));
  g_assert_true (sd_status_app (&a, SD_OUTPUT, "mpv") == &app);
  g_assert_null (sd_status_app (&a, SD_INPUT, "mpv"));
  /* System sounds' volume is the status's too. */
  other = app;
  b.has_system_sounds = TRUE;
  g_assert_false (sd_status_equal (&a, &b));
}

static void
test_card_copy_is_equal_and_its_own (void)
{
  SdProfile profiles[] = { { "a2dp", "High Fidelity Playback (A2DP Sink)" },
                           { "hfp", "Headset Head Unit (HSP/HFP)" } };
  SdCard card = { 7, "WH-1000XM4", profiles, 2, "a2dp" };
  SdCard copy;

  sd_card_copy (&card, &copy);
  g_assert_true (sd_cards_equal (&card, 1, &copy, 1));
  g_assert_true (copy.profiles != card.profiles);
  g_assert_cmpstr (copy.profiles[1].name, ==, "hfp");
  sd_card_clear (&copy);
  g_assert_null (copy.profiles);
  g_assert_cmpuint (copy.n_profiles, ==, 0);
}

static void
test_status_equal_compares_cards (void)
{
  SdProfile duplex[] = { { "duplex", "Analog Stereo Duplex" }, { "off", "Off" } };
  SdCard card = { 49, "Built-in Audio", duplex, 2, "duplex" };
  SdCard other = card;
  SdStatus a = playing ("Speakers", 45), b = playing ("Speakers", 45);

  a.cards = &card;
  a.n_cards = 1;
  b.cards = &other;
  b.n_cards = 1;
  g_assert_true (sd_status_equal (&a, &b));
  other.active = "off";
  g_assert_false (sd_status_equal (&a, &b));
  g_assert_false (sd_cards_equal (a.cards, a.n_cards, b.cards, b.n_cards));
  other.active = "duplex";
  b.volume = PERCENT (50);
  g_assert_false (sd_status_equal (&a, &b));
  g_assert_true (sd_cards_equal (a.cards, a.n_cards, b.cards, b.n_cards));
}

static void
test_device_forms (void)
{
  g_assert_cmpint (sd_state_device_form (SD_OUTPUT, NULL, NULL, PA_DEVICE_PORT_TYPE_UNKNOWN), ==,
                   SD_FORM_SPEAKER);
  g_assert_cmpint (sd_state_device_form (SD_OUTPUT, "headset", NULL, PA_DEVICE_PORT_TYPE_UNKNOWN),
                   ==, SD_FORM_HEADPHONES);
  g_assert_cmpint (sd_state_device_form (SD_OUTPUT, "internal", "analog-output-headphones",
                                         PA_DEVICE_PORT_TYPE_HEADPHONES),
                   ==, SD_FORM_HEADPHONES);
  g_assert_cmpint (sd_state_device_form (SD_OUTPUT, "internal", "analog-output-speaker",
                                         PA_DEVICE_PORT_TYPE_SPEAKER),
                   ==, SD_FORM_SPEAKER);
  g_assert_cmpint (sd_state_device_form (SD_OUTPUT, NULL, "hdmi-output-0",
                                         PA_DEVICE_PORT_TYPE_HDMI),
                   ==, SD_FORM_DISPLAY);
  /* A server too old to give the port's type: ALSA's names say it. */
  g_assert_cmpint (sd_state_device_form (SD_OUTPUT, NULL, "hdmi-output-1",
                                         PA_DEVICE_PORT_TYPE_UNKNOWN),
                   ==, SD_FORM_DISPLAY);
  g_assert_cmpint (sd_state_device_form (SD_OUTPUT, "tv", NULL, PA_DEVICE_PORT_TYPE_UNKNOWN), ==,
                   SD_FORM_DISPLAY);
  /* A headset's microphone is a microphone. */
  g_assert_cmpint (sd_state_device_form (SD_INPUT, "headset", NULL, PA_DEVICE_PORT_TYPE_UNKNOWN),
                   ==, SD_FORM_MICROPHONE);
}

static void
test_unplugged_ports_are_left_out (void)
{
  SdPortInfo ports[] = {
    { "analog-output-speaker", "Speakers", FALSE },
    { "analog-output-headphones", "Headphones", TRUE },
    { "analog-output-lineout", "Line Out", FALSE },
  };
  guint n = 0;
  SdPort *plugged = sd_state_plugged_ports (ports, G_N_ELEMENTS (ports), &n);

  g_assert_cmpuint (n, ==, 2);
  g_assert_cmpstr (plugged[0].name, ==, "analog-output-speaker");
  g_assert_cmpstr (plugged[0].description, ==, "Speakers");
  g_assert_cmpstr (plugged[1].description, ==, "Line Out");
  for (guint i = 0; i < n; i++)
    {
      g_free (plugged[i].name);
      g_free (plugged[i].description);
    }
  g_free (plugged);
  g_assert_null (sd_state_plugged_ports (ports + 1, 1, &n));
  g_assert_cmpuint (n, ==, 0);
}

static void
test_device_detail_is_its_port_or_virtual (void)
{
  SdDevice hardware = { .name = "alsa_output.pci", .description = "Built-in Audio",
                        .active_port = "analog-output-speaker",
                        .active_port_description = "Speakers" };
  SdDevice easy_effects = { .name = "easyeffects_sink", .description = "Easy Effects Sink",
                            .virtual_device = TRUE };
  SdDevice bare = { .name = "bluez_output", .description = "WH-1000XM4" };

  g_assert_cmpstr (sd_state_device_detail (&hardware), ==, "Speakers");
  g_assert_cmpstr (sd_state_device_detail (&easy_effects), ==, "Virtual device");
  g_assert_null (sd_state_device_detail (&bare));
}

static void
test_device_icons_in_the_lists (void)
{
  SdDevice speaker = { .form = SD_FORM_SPEAKER };
  SdDevice headphones = { .form = SD_FORM_HEADPHONES };
  SdDevice display = { .form = SD_FORM_DISPLAY };
  SdDevice microphone = { .form = SD_FORM_MICROPHONE };

  g_assert_cmpstr (sd_state_device_icon_names (&speaker)[0], ==, "audio-speakers");
  g_assert_cmpstr (sd_state_device_icon_names (&speaker)[1], ==, "audio-speakers-symbolic");
  g_assert_cmpstr (sd_state_device_icon_names (&headphones)[0], ==, "audio-headphones");
  g_assert_cmpstr (sd_state_device_icon_names (&display)[0], ==, "video-display");
  g_assert_cmpstr (sd_state_device_icon_names (&microphone)[0], ==, "audio-input-microphone");
}

static void
test_mute_button_shows_the_outputs_form (void)
{
  SdDevice speaker = { .form = SD_FORM_SPEAKER, .volume = PERCENT (50) };
  SdDevice loud = { .form = SD_FORM_SPEAKER, .volume = PERCENT (90) };
  SdDevice muted = { .form = SD_FORM_SPEAKER, .volume = PERCENT (50), .muted = TRUE };
  SdDevice headphones = { .form = SD_FORM_HEADPHONES, .volume = PERCENT (50) };
  SdDevice display = { .form = SD_FORM_DISPLAY, .volume = PERCENT (50), .muted = TRUE };

  g_assert_cmpstr (sd_state_mute_icon_names (SD_OUTPUT, &speaker)[0], ==, "audio-volume-medium");
  g_assert_cmpstr (sd_state_mute_icon_names (SD_OUTPUT, &loud)[0], ==, "audio-volume-high");
  g_assert_cmpstr (sd_state_mute_icon_names (SD_OUTPUT, &muted)[0], ==, "audio-volume-muted");
  g_assert_false (sd_state_mute_struck (SD_OUTPUT, &muted));
  g_assert_cmpstr (sd_state_mute_icon_names (SD_OUTPUT, &headphones)[0], ==, "audio-headphones");
  g_assert_false (sd_state_mute_struck (SD_OUTPUT, &headphones));
  headphones.muted = TRUE;
  g_assert_true (sd_state_mute_struck (SD_OUTPUT, &headphones));
  g_assert_cmpstr (sd_state_mute_icon_names (SD_OUTPUT, &display)[0], ==, "video-display");
  g_assert_true (sd_state_mute_struck (SD_OUTPUT, &display));
  g_assert_cmpstr (sd_state_mute_icon_names (SD_OUTPUT, NULL)[0], ==, "audio-volume-muted");
}

static void
test_mute_button_shows_the_microphones_level (void)
{
  SdDevice quiet = { .form = SD_FORM_MICROPHONE, .volume = PERCENT (20) };
  SdDevice loud = { .form = SD_FORM_MICROPHONE, .volume = PERCENT (100) };
  SdDevice muted = { .form = SD_FORM_MICROPHONE, .volume = PERCENT (100), .muted = TRUE };

  g_assert_cmpstr (sd_state_mute_icon_names (SD_INPUT, &quiet)[0], ==,
                   "microphone-sensitivity-low");
  g_assert_cmpstr (sd_state_mute_icon_names (SD_INPUT, &quiet)[1], ==,
                   "microphone-sensitivity-low-symbolic");
  g_assert_cmpstr (sd_state_mute_icon_names (SD_INPUT, &loud)[0], ==,
                   "microphone-sensitivity-high");
  g_assert_cmpstr (sd_state_mute_icon_names (SD_INPUT, &muted)[0], ==,
                   "microphone-sensitivity-muted");
  g_assert_false (sd_state_mute_struck (SD_INPUT, &muted));
  g_assert_cmpstr (sd_state_mute_icon_names (SD_INPUT, NULL)[0], ==,
                   "microphone-sensitivity-muted");
}

static void
test_default_device_by_name (void)
{
  SdDevice outputs[] = { { .name = "speaker" }, { .name = "hdmi" } };
  SdStatus status = { .running = TRUE, .outputs = outputs, .n_outputs = 2,
                      .default_output = "hdmi" };
  guint n = 0;

  g_assert_true (sd_status_default_device (&status, SD_OUTPUT) == &outputs[1]);
  g_assert_null (sd_status_default_device (&status, SD_INPUT));
  g_assert_true (sd_status_devices (&status, SD_OUTPUT, &n) == outputs);
  g_assert_cmpuint (n, ==, 2);
  status.default_output = "gone";
  g_assert_null (sd_status_default_device (&status, SD_OUTPUT));
}

static void
test_device_copy_is_equal_and_its_own (void)
{
  SdPort ports[] = { { "speaker", "Speakers" }, { "lineout", "Line Out" } };
  SdDevice device = { "alsa", "Built-in Audio", SD_FORM_SPEAKER, FALSE, PERCENT (40), FALSE,
                      ports, 2, "speaker", "Speakers" };
  SdDevice copy;

  sd_device_copy (&device, &copy);
  g_assert_true (sd_device_equal (&device, &copy));
  g_assert_true (copy.ports != device.ports);
  g_assert_cmpstr (copy.ports[1].description, ==, "Line Out");
  copy.ports[1].description[0] = 'X';
  g_assert_false (sd_device_equal (&device, &copy));
  sd_device_clear (&copy);
  g_assert_null (copy.name);
  g_assert_cmpuint (copy.n_ports, ==, 0);
}

static void
test_status_equal_compares_the_lists (void)
{
  SdDevice a_outputs[] = { { .name = "speaker", .description = "Speakers" } };
  SdDevice b_outputs[] = { { .name = "speaker", .description = "Speakers" } };
  SdStatus a = playing ("Speakers", 45), b = playing ("Speakers", 45);

  a.outputs = a_outputs;
  a.n_outputs = 1;
  b.outputs = b_outputs;
  b.n_outputs = 1;
  g_assert_true (sd_status_equal (&a, &b));
  b_outputs[0].volume = PERCENT (10);
  g_assert_false (sd_status_equal (&a, &b));
  b_outputs[0].volume = 0;
  b.default_input = "mic";
  g_assert_false (sd_status_equal (&a, &b));
}

static void
test_percent_round_trips (void)
{
  g_assert_cmpuint (sd_state_percent (PA_VOLUME_NORM), ==, 100);
  g_assert_cmpuint (sd_state_volume_of_percent (100), ==, PA_VOLUME_NORM);
  g_assert_cmpuint (sd_state_volume_of_percent (0), ==, 0);
  g_assert_cmpuint (sd_state_percent (sd_state_volume_of_percent (45)), ==, 45);
  g_assert_cmpuint (sd_state_percent (sd_state_volume_of_percent (150)), ==, 150);
  /* A glide's share is rounded where it's sent. */
  g_assert_cmpuint (sd_state_volume_of_percent (44.6), ==, sd_state_volume_of_percent (44.6));
  g_assert_cmpuint (sd_state_percent (sd_state_volume_of_percent (44.625)), ==, 45);
}

static void
test_room_under_a_top_panel_on_x11 (void)
{
  /* The work area leaves the panel's strut out. */
  SdPlace place = {
    .geometry = { 0, 0, 1920, 1080 },
    .workarea = { 0, 24, 1920, 1056 },
    .panel_known = TRUE,
    .panel = { 0, 0, 1920, 24 },
    .panel_thickness = 24,
  };

  g_assert_cmpint (sd_state_popup_room (&place), ==, 1056);
  /* A second monitor, below the first. */
  place.geometry = (SdRect) { 0, 1080, 1280, 720 };
  place.workarea = (SdRect) { 0, 1104, 1280, 696 };
  place.panel = (SdRect) { 0, 1080, 1280, 24 };
  g_assert_cmpint (sd_state_popup_room (&place), ==, 696);
}

static void
test_room_above_a_bottom_panel_on_x11 (void)
{
  SdPlace place = {
    .geometry = { 0, 0, 1920, 1080 },
    .workarea = { 0, 0, 1920, 1048 },
    .panel_known = TRUE,
    .panel = { 0, 1048, 1920, 32 },
    .panel_thickness = 32,
    .opens_up = TRUE,
  };

  g_assert_cmpint (sd_state_popup_room (&place), ==, 1048);
}

static void
test_room_on_wayland_from_the_panels_thickness (void)
{
  /* GDK3 gives the monitor's geometry for the work area, and the panel's place isn't known. */
  SdPlace place = {
    .geometry = { 0, 0, 1366, 768 },
    .workarea = { 0, 0, 1366, 768 },
    .panel_thickness = 24,
  };

  g_assert_cmpint (sd_state_popup_room (&place), ==, 744);
  place.opens_up = TRUE;
  g_assert_cmpint (sd_state_popup_room (&place), ==, 744);
}

static void
test_room_beside_a_vertical_panel (void)
{
  SdPlace place = {
    .geometry = { 0, 0, 1920, 1080 },
    .workarea = { 48, 0, 1872, 1080 },
    .panel_known = TRUE,
    .panel = { 0, 0, 48, 1080 },
    .panel_thickness = 48,
    .sideways = TRUE,
  };

  /* It slides along the panel: the whole height. */
  g_assert_cmpint (sd_state_popup_room (&place), ==, 1080);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/state/device-forms", test_device_forms);
  g_test_add_func ("/state/unplugged-ports-are-left-out", test_unplugged_ports_are_left_out);
  g_test_add_func ("/state/device-detail-is-its-port-or-virtual",
                   test_device_detail_is_its_port_or_virtual);
  g_test_add_func ("/state/device-icons-in-the-lists", test_device_icons_in_the_lists);
  g_test_add_func ("/state/mute-button-shows-the-outputs-form",
                   test_mute_button_shows_the_outputs_form);
  g_test_add_func ("/state/mute-button-shows-the-microphones-level",
                   test_mute_button_shows_the_microphones_level);
  g_test_add_func ("/state/default-device-by-name", test_default_device_by_name);
  g_test_add_func ("/state/device-copy-is-equal-and-its-own",
                   test_device_copy_is_equal_and_its_own);
  g_test_add_func ("/state/status-equal-compares-the-lists", test_status_equal_compares_the_lists);
  g_test_add_func ("/state/percent-round-trips", test_percent_round_trips);
  g_test_add_func ("/state/room-under-a-top-panel-on-x11", test_room_under_a_top_panel_on_x11);
  g_test_add_func ("/state/room-above-a-bottom-panel-on-x11",
                   test_room_above_a_bottom_panel_on_x11);
  g_test_add_func ("/state/room-on-wayland-from-the-panels-thickness",
                   test_room_on_wayland_from_the_panels_thickness);
  g_test_add_func ("/state/room-beside-a-vertical-panel", test_room_beside_a_vertical_panel);
  g_test_add_func ("/state/no-sound-server", test_no_sound_server);
  g_test_add_func ("/state/speaker-levels", test_speaker_levels);
  g_test_add_func ("/state/speaker-muted-or-at-zero", test_speaker_muted_or_at_zero);
  g_test_add_func ("/state/headphones", test_headphones);
  g_test_add_func ("/state/headphones-muted-are-struck-out",
                   test_headphones_muted_are_struck_out);
  g_test_add_func ("/state/no-output", test_no_output);
  g_test_add_func ("/state/tooltip-rounds-as-pactl-does", test_tooltip_rounds_as_pactl_does);
  g_test_add_func ("/state/tooltip-names-who-records", test_tooltip_names_who_records);
  g_test_add_func ("/state/scroll-steps", test_scroll_steps);
  g_test_add_func ("/state/scroll-stops-at-100-percent", test_scroll_stops_at_100_percent);
  g_test_add_func ("/state/scroll-goes-to-150-percent-with-the-gnome-key",
                   test_scroll_goes_to_150_percent_with_the_gnome_key);
  g_test_add_func ("/state/scroll-up-past-the-cap-keeps-the-volume",
                   test_scroll_up_past_the_cap_keeps_the_volume);
  g_test_add_func ("/state/recording-names-share-a-row-per-program",
                   test_recording_names_share_a_row_per_program);
  g_test_add_func ("/state/recording-names-without-streams",
                   test_recording_names_without_streams);
  g_test_add_func ("/state/apps-share-a-row-per-program", test_apps_share_a_row_per_program);
  g_test_add_func ("/state/an-apps-streams-keep-their-ratio", test_an_apps_streams_keep_their_ratio);
  g_test_add_func ("/state/apps-equal-compares-streams-and-devices",
                   test_apps_equal_compares_streams_and_devices);
  g_test_add_func ("/state/status-equal-compares-cards", test_status_equal_compares_cards);
  g_test_add_func ("/state/card-copy-is-equal-and-its-own", test_card_copy_is_equal_and_its_own);
  return g_test_run ();
}
