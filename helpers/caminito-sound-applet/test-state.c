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

  /* The speaker struck through: MATE's and Breeze's, then the muted one that Adwaita has. */
  g_assert_cmpstr (names[0], ==, "audio-volume-off");
  g_assert_cmpstr (names[1], ==, "audio-off");
  g_assert_cmpstr (names[2], ==, "audio-volume-muted");
  g_assert_cmpstr (names[3], ==, "audio-volume-muted-symbolic");
  g_assert_null (names[4]);
  g_assert_false (sd_state_icon_struck (&status));
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
    { NULL, "Discord", "electron" },
    { NULL, "OBS Studio", "obs" },
    /* Another of Discord's streams. */
    { NULL, "Discord", "electron" },
    /* Two Electron programs, apart by name and not by binary. */
    { NULL, "Slack", "electron" },
    /* A Flatpak's, by its app ID whatever its streams call themselves. */
    { "com.spotify.Client", "Spotify", "spotify" },
    { "com.spotify.Client", "spotify", "spotify" },
    /* Nameless, by its binary. */
    { NULL, NULL, "arecord" },
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
  g_assert_true (sd_cards_equal (&a, &b));
  other.active = "off";
  g_assert_false (sd_status_equal (&a, &b));
  g_assert_false (sd_cards_equal (&a, &b));
  other.active = "duplex";
  b.volume = PERCENT (50);
  g_assert_false (sd_status_equal (&a, &b));
  g_assert_true (sd_cards_equal (&a, &b));
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
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
  g_test_add_func ("/state/status-equal-compares-cards", test_status_equal_compares_cards);
  return g_test_run ();
}
