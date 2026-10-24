/*
 * The Dock's launch and attention feedback (CMN-221), from windows built by hand and a clock
 * the tests move.
 */

#include "dk-feedback.h"

#define SECOND G_USEC_PER_SEC

typedef struct
{
  DkCatalog *catalog;
  DkFeedback *feedback;
  GArray *windows;
  guint64 clock;
} Desk;

static Desk *
desk_new (void)
{
  Desk *desk = g_new0 (Desk, 1);

  desk->catalog = dk_catalog_new ();
  dk_catalog_add (desk->catalog, "firefox.desktop", "Navigator");
  dk_catalog_add (desk->catalog, "mate-terminal.desktop", NULL);
  desk->feedback = dk_feedback_new ();
  desk->windows = g_array_sized_new (FALSE, TRUE, sizeof (DkWindow), 64);
  return desk;
}

static void
desk_free (Desk *desk)
{
  dk_catalog_free (desk->catalog);
  dk_feedback_free (desk->feedback);
  g_array_unref (desk->windows);
  g_free (desk);
}

static DkWindow *
open_window (Desk *desk, const char *app_id)
{
  DkWindow window = { .handle = GUINT_TO_POINTER (desk->windows->len + 1), .app_id = app_id,
                      .title = app_id, .opened = ++desk->clock, .activated = 0 };

  g_array_append_val (desk->windows, window);
  return &g_array_index (desk->windows, DkWindow, desk->windows->len - 1);
}

static void
focus (Desk *desk, DkWindow *window)
{
  for (guint i = 0; i < desk->windows->len; i++)
    {
      g_array_index (desk->windows, DkWindow, i).active = FALSE;
    }
  window->active = TRUE;
  window->urgent = FALSE;
  window->activated = ++desk->clock;
}

static GArray *
build (Desk *desk, const char *const *kept)
{
  return dk_items_build (desk->catalog, kept, (DkWindow *) desk->windows->data,
                         desk->windows->len);
}

/* The desk's items at now, as the dock updates the feedback with them. */
static void
update (Desk *desk, const char *const *kept, gint64 now)
{
  g_autoptr (GArray) items = build (desk, kept);

  dk_feedback_update (desk->feedback, items, now);
}

/* The dock launches the item with key at now. */
static void
launch (Desk *desk, const char *const *kept, const char *key, gint64 now)
{
  g_autoptr (GArray) items = build (desk, kept);

  dk_feedback_launch (desk->feedback, key, desk->clock, now);
  dk_feedback_update (desk->feedback, items, now);
}

static void
test_a_launch_pulses_until_the_programs_first_window_maps (void)
{
  const char *kept[] = { "firefox.desktop", NULL };
  Desk *desk = desk_new ();

  g_assert_false (dk_feedback_get (desk->feedback, "firefox.desktop", 0).pulsing);
  launch (desk, kept, "firefox.desktop", 0);
  g_assert_true (dk_feedback_get (desk->feedback, "firefox.desktop", 0).pulsing);
  update (desk, kept, 2 * SECOND);
  g_assert_true (dk_feedback_get (desk->feedback, "firefox.desktop", 2 * SECOND).pulsing);

  open_window (desk, "firefox");
  update (desk, kept, 3 * SECOND);

  g_assert_false (dk_feedback_get (desk->feedback, "firefox.desktop", 3 * SECOND).pulsing);
  /* And it doesn't come back with the window's changes. */
  update (desk, kept, 4 * SECOND);
  g_assert_false (dk_feedback_get (desk->feedback, "firefox.desktop", 4 * SECOND).pulsing);
  desk_free (desk);
}

static void
test_a_launch_pulses_for_ten_seconds_for_a_program_that_maps_no_window (void)
{
  const char *kept[] = { "firefox.desktop", NULL };
  Desk *desk = desk_new ();

  launch (desk, kept, "firefox.desktop", 5 * SECOND);

  g_assert_cmpint (dk_feedback_deadline (desk->feedback, 5 * SECOND), ==, 15 * SECOND);
  update (desk, kept, 15 * SECOND - 1);
  g_assert_true (dk_feedback_get (desk->feedback, "firefox.desktop", 15 * SECOND - 1).pulsing);
  update (desk, kept, 15 * SECOND);
  g_assert_false (dk_feedback_get (desk->feedback, "firefox.desktop", 15 * SECOND).pulsing);
  g_assert_cmpint (dk_feedback_deadline (desk->feedback, 15 * SECOND), ==, -1);
  desk_free (desk);
}

static void
test_the_state_is_false_past_the_timeout_even_before_an_update (void)
{
  const char *kept[] = { "firefox.desktop", NULL };
  Desk *desk = desk_new ();

  launch (desk, kept, "firefox.desktop", 0);

  g_assert_false (dk_feedback_get (desk->feedback, "firefox.desktop", DK_LAUNCH_TIMEOUT).pulsing);
  desk_free (desk);
}

static void
test_the_timeout_can_be_shortened (void)
{
  const char *kept[] = { "firefox.desktop", NULL };
  Desk *desk = desk_new ();

  dk_feedback_set_launch_timeout (desk->feedback, SECOND / 2);
  launch (desk, kept, "firefox.desktop", 0);

  g_assert_cmpint (dk_feedback_deadline (desk->feedback, 0), ==, SECOND / 2);
  update (desk, kept, SECOND / 2);
  g_assert_false (dk_feedback_get (desk->feedback, "firefox.desktop", SECOND / 2).pulsing);
  desk_free (desk);
}

static void
test_only_the_launched_item_pulses (void)
{
  const char *kept[] = { "firefox.desktop", "mate-terminal.desktop", NULL };
  Desk *desk = desk_new ();

  launch (desk, kept, "mate-terminal.desktop", 0);

  g_assert_false (dk_feedback_get (desk->feedback, "firefox.desktop", 0).pulsing);
  g_assert_true (dk_feedback_get (desk->feedback, "mate-terminal.desktop", 0).pulsing);
  /* A window of the other program doesn't end it. */
  open_window (desk, "firefox");
  update (desk, kept, SECOND);
  g_assert_true (dk_feedback_get (desk->feedback, "mate-terminal.desktop", SECOND).pulsing);
  desk_free (desk);
}

static void
test_a_new_instance_of_a_running_program_pulses_until_its_own_window (void)
{
  const char *kept[] = { NULL };
  Desk *desk = desk_new ();

  open_window (desk, "mate-terminal");
  launch (desk, kept, "mate-terminal.desktop", 0);

  g_assert_true (dk_feedback_get (desk->feedback, "mate-terminal.desktop", 0).pulsing);
  update (desk, kept, SECOND);
  g_assert_true (dk_feedback_get (desk->feedback, "mate-terminal.desktop", SECOND).pulsing);

  open_window (desk, "mate-terminal");
  update (desk, kept, 2 * SECOND);

  g_assert_false (dk_feedback_get (desk->feedback, "mate-terminal.desktop", 2 * SECOND).pulsing);
  desk_free (desk);
}

/* The dock tells windows by their XfwWindow, whose address a later window can have. */
static void
test_a_new_window_ends_the_pulse_even_with_the_handle_of_one_that_closed (void)
{
  const char *kept[] = { "mate-terminal.desktop", NULL };
  Desk *desk = desk_new ();

  open_window (desk, "mate-terminal");
  launch (desk, kept, "mate-terminal.desktop", 0);
  g_array_set_size (desk->windows, 0);
  update (desk, kept, SECOND);
  g_assert_true (dk_feedback_get (desk->feedback, "mate-terminal.desktop", SECOND).pulsing);

  open_window (desk, "mate-terminal");
  update (desk, kept, 2 * SECOND);

  g_assert_false (dk_feedback_get (desk->feedback, "mate-terminal.desktop", 2 * SECOND).pulsing);
  desk_free (desk);
}

static void
test_a_second_launch_while_pulsing_extends_the_pulse (void)
{
  const char *kept[] = { "firefox.desktop", NULL };
  Desk *desk = desk_new ();

  launch (desk, kept, "firefox.desktop", 0);
  launch (desk, kept, "firefox.desktop", 6 * SECOND);

  update (desk, kept, 12 * SECOND);
  g_assert_true (dk_feedback_get (desk->feedback, "firefox.desktop", 12 * SECOND).pulsing);
  update (desk, kept, 16 * SECOND);
  g_assert_false (dk_feedback_get (desk->feedback, "firefox.desktop", 16 * SECOND).pulsing);
  desk_free (desk);
}

static void
test_the_pulse_ends_with_an_item_that_goes (void)
{
  const char *kept[] = { "firefox.desktop", NULL };
  const char *none[] = { NULL };
  Desk *desk = desk_new ();

  launch (desk, kept, "firefox.desktop", 0);
  update (desk, none, SECOND);

  g_assert_false (dk_feedback_get (desk->feedback, "firefox.desktop", SECOND).pulsing);
  /* And it doesn't come back with the item. */
  update (desk, kept, 2 * SECOND);
  g_assert_false (dk_feedback_get (desk->feedback, "firefox.desktop", 2 * SECOND).pulsing);
  desk_free (desk);
}

static void
test_a_window_that_demands_attention_bounces_then_keeps_a_highlight (void)
{
  const char *none[] = { NULL };
  Desk *desk = desk_new ();
  DkWindow *window = open_window (desk, "mate-terminal");
  DkFeedbackState state;

  update (desk, none, 0);
  state = dk_feedback_get (desk->feedback, "mate-terminal.desktop", 0);
  g_assert_false (state.bouncing);
  g_assert_false (state.highlighted);

  window->urgent = TRUE;
  update (desk, none, 4 * SECOND);

  state = dk_feedback_get (desk->feedback, "mate-terminal.desktop", 4 * SECOND);
  g_assert_true (state.bouncing);
  g_assert_cmpint (state.bounce_start, ==, 4 * SECOND);
  g_assert_true (state.highlighted);
  g_assert_cmpint (dk_feedback_deadline (desk->feedback, 4 * SECOND), ==, 4 * SECOND + DK_BOUNCE_LENGTH);

  /* A few bounces, and then the highlight alone. */
  update (desk, none, 4 * SECOND + DK_BOUNCE_LENGTH);
  state = dk_feedback_get (desk->feedback, "mate-terminal.desktop", 4 * SECOND + DK_BOUNCE_LENGTH);
  g_assert_false (state.bouncing);
  g_assert_true (state.highlighted);
  update (desk, none, 60 * SECOND);
  g_assert_true (dk_feedback_get (desk->feedback, "mate-terminal.desktop", 60 * SECOND).highlighted);
  g_assert_cmpint (dk_feedback_deadline (desk->feedback, 60 * SECOND), ==, -1);
  desk_free (desk);
}

static void
test_the_highlight_clears_when_the_demanding_window_is_focused (void)
{
  const char *none[] = { NULL };
  Desk *desk = desk_new ();
  DkWindow *window = open_window (desk, "mate-terminal");

  window->urgent = TRUE;
  update (desk, none, 0);
  g_assert_true (dk_feedback_get (desk->feedback, "mate-terminal.desktop", SECOND).highlighted);

  focus (desk, window);
  update (desk, none, SECOND);

  g_assert_false (dk_feedback_get (desk->feedback, "mate-terminal.desktop", SECOND).highlighted);
  g_assert_false (dk_feedback_get (desk->feedback, "mate-terminal.desktop", SECOND).bouncing);
  desk_free (desk);
}

static void
test_focusing_another_window_of_the_item_leaves_the_highlight (void)
{
  const char *none[] = { NULL };
  Desk *desk = desk_new ();
  DkWindow *first = open_window (desk, "mate-terminal");
  DkWindow *second = open_window (desk, "mate-terminal");

  second->urgent = TRUE;
  update (desk, none, 0);
  focus (desk, first);
  update (desk, none, SECOND);

  g_assert_true (dk_feedback_get (desk->feedback, "mate-terminal.desktop", SECOND).highlighted);

  focus (desk, second);
  update (desk, none, 2 * SECOND);
  g_assert_false (dk_feedback_get (desk->feedback, "mate-terminal.desktop", 2 * SECOND).highlighted);
  desk_free (desk);
}

static void
test_a_window_already_demanding_does_not_bounce_again_but_a_new_one_does (void)
{
  const char *none[] = { NULL };
  Desk *desk = desk_new ();
  DkWindow *first = open_window (desk, "mate-terminal");
  DkWindow *second = open_window (desk, "mate-terminal");
  DkFeedbackState state;

  first->urgent = TRUE;
  update (desk, none, 0);
  update (desk, none, 10 * SECOND);
  state = dk_feedback_get (desk->feedback, "mate-terminal.desktop", 10 * SECOND);
  g_assert_false (state.bouncing);
  g_assert_true (state.highlighted);

  second->urgent = TRUE;
  update (desk, none, 11 * SECOND);
  state = dk_feedback_get (desk->feedback, "mate-terminal.desktop", 11 * SECOND);
  g_assert_true (state.bouncing);
  g_assert_cmpint (state.bounce_start, ==, 11 * SECOND);
  desk_free (desk);
}

static void
test_a_window_that_demands_attention_again_bounces_again (void)
{
  const char *none[] = { NULL };
  Desk *desk = desk_new ();
  DkWindow *window = open_window (desk, "mate-terminal");

  window->urgent = TRUE;
  update (desk, none, 0);
  focus (desk, window);
  update (desk, none, SECOND);
  window->active = FALSE;
  window->urgent = TRUE;
  update (desk, none, 20 * SECOND);

  g_assert_true (dk_feedback_get (desk->feedback, "mate-terminal.desktop", 20 * SECOND).bouncing);
  desk_free (desk);
}

/* An X11 client's urgency hint can outlast the focus. */
static void
test_a_window_still_urgent_after_the_focus_is_not_highlighted_again (void)
{
  const char *none[] = { NULL };
  Desk *desk = desk_new ();
  DkWindow *other = open_window (desk, "caja");
  DkWindow *window = open_window (desk, "mate-terminal");

  window->urgent = TRUE;
  update (desk, none, 0);
  focus (desk, window);
  window->urgent = TRUE;
  update (desk, none, SECOND);
  focus (desk, other);
  window->urgent = TRUE;
  update (desk, none, 2 * SECOND);

  g_assert_false (dk_feedback_get (desk->feedback, "mate-terminal.desktop", 2 * SECOND).highlighted);
  g_assert_false (dk_feedback_get (desk->feedback, "mate-terminal.desktop", 2 * SECOND).bouncing);

  window->urgent = FALSE;
  update (desk, none, 3 * SECOND);
  window->urgent = TRUE;
  update (desk, none, 4 * SECOND);

  g_assert_true (dk_feedback_get (desk->feedback, "mate-terminal.desktop", 4 * SECOND).bouncing);
  desk_free (desk);
}

static void
test_a_focused_window_does_not_count_as_demanding (void)
{
  const char *none[] = { NULL };
  Desk *desk = desk_new ();
  DkWindow *window = open_window (desk, "mate-terminal");

  window->active = TRUE;
  window->urgent = TRUE;
  update (desk, none, 0);

  g_assert_false (dk_feedback_get (desk->feedback, "mate-terminal.desktop", 0).highlighted);
  desk_free (desk);
}

static void
test_the_highlight_ends_with_the_window (void)
{
  const char *none[] = { NULL };
  Desk *desk = desk_new ();
  DkWindow *window = open_window (desk, "mate-terminal");

  window->urgent = TRUE;
  update (desk, none, 0);
  g_array_set_size (desk->windows, 0);
  update (desk, none, SECOND);

  g_assert_false (dk_feedback_get (desk->feedback, "mate-terminal.desktop", SECOND).highlighted);
  desk_free (desk);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/launch/pulses-until-the-programs-first-window-maps",
                   test_a_launch_pulses_until_the_programs_first_window_maps);
  g_test_add_func ("/launch/pulses-for-ten-seconds-for-a-program-that-maps-no-window",
                   test_a_launch_pulses_for_ten_seconds_for_a_program_that_maps_no_window);
  g_test_add_func ("/launch/is-over-past-the-timeout-even-before-an-update",
                   test_the_state_is_false_past_the_timeout_even_before_an_update);
  g_test_add_func ("/launch/timeout-can-be-shortened", test_the_timeout_can_be_shortened);
  g_test_add_func ("/launch/only-the-launched-item-pulses", test_only_the_launched_item_pulses);
  g_test_add_func ("/launch/new-instance-pulses-until-its-own-window",
                   test_a_new_instance_of_a_running_program_pulses_until_its_own_window);
  g_test_add_func ("/launch/new-window-with-a-reused-handle-ends-the-pulse",
                   test_a_new_window_ends_the_pulse_even_with_the_handle_of_one_that_closed);
  g_test_add_func ("/launch/second-launch-extends-the-pulse",
                   test_a_second_launch_while_pulsing_extends_the_pulse);
  g_test_add_func ("/launch/pulse-ends-with-an-item-that-goes",
                   test_the_pulse_ends_with_an_item_that_goes);
  g_test_add_func ("/attention/bounces-then-keeps-a-highlight",
                   test_a_window_that_demands_attention_bounces_then_keeps_a_highlight);
  g_test_add_func ("/attention/highlight-clears-when-the-window-is-focused",
                   test_the_highlight_clears_when_the_demanding_window_is_focused);
  g_test_add_func ("/attention/focusing-another-window-leaves-the-highlight",
                   test_focusing_another_window_of_the_item_leaves_the_highlight);
  g_test_add_func ("/attention/bounces-for-a-new-demand-only",
                   test_a_window_already_demanding_does_not_bounce_again_but_a_new_one_does);
  g_test_add_func ("/attention/demanding-again-bounces-again",
                   test_a_window_that_demands_attention_again_bounces_again);
  g_test_add_func ("/attention/still-urgent-after-the-focus-is-not-highlighted-again",
                   test_a_window_still_urgent_after_the_focus_is_not_highlighted_again);
  g_test_add_func ("/attention/a-focused-window-does-not-demand",
                   test_a_focused_window_does_not_count_as_demanding);
  g_test_add_func ("/attention/highlight-ends-with-the-window",
                   test_the_highlight_ends_with_the_window);
  return g_test_run ();
}
