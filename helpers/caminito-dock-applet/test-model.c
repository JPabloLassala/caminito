/*
 * The Dock's items and what a click on one does (CMN-219), from windows built by hand.
 */

#include "dk-model.h"

typedef struct
{
  DkCatalog *catalog;
  guint64 clock;
  GArray *windows;
} Desk;

static Desk *
desk_new (void)
{
  Desk *desk = g_new0 (Desk, 1);

  desk->catalog = dk_catalog_new ();
  dk_catalog_add (desk->catalog, "firefox.desktop", "Navigator");
  dk_catalog_add (desk->catalog, "mate-terminal.desktop", NULL);
  dk_catalog_add (desk->catalog, "org.gnome.Nautilus.desktop", NULL);
  /* Room for the tests' windows, so that the pointers open_window returns stay valid. */
  desk->windows = g_array_sized_new (FALSE, TRUE, sizeof (DkWindow), 64);
  return desk;
}

static void
desk_free (Desk *desk)
{
  dk_catalog_free (desk->catalog);
  g_array_unref (desk->windows);
  g_free (desk);
}

/* A window opens and is the most recent one. */
static DkWindow *
open_window (Desk *desk, const char *app_id)
{
  DkWindow window = { .handle = GUINT_TO_POINTER (desk->windows->len + 1), .app_id = app_id,
                      .title = app_id, .opened = ++desk->clock, .activated = desk->clock };

  g_array_append_val (desk->windows, window);
  return &g_array_index (desk->windows, DkWindow, desk->windows->len - 1);
}

static void
focus (Desk *desk, DkWindow *window)
{
  for (guint i = 0; i < desk->windows->len; i++)
    g_array_index (desk->windows, DkWindow, i).active = FALSE;
  window->active = TRUE;
  window->activated = ++desk->clock;
}

static GArray *
build (Desk *desk, const char *const *kept)
{
  return dk_items_build (desk->catalog, kept, (DkWindow *) desk->windows->data,
                         desk->windows->len);
}

static DkItem *
item (GArray *items, guint i)
{
  return &g_array_index (items, DkItem, i);
}

static void
test_one_item_for_two_processes_of_one_program (void)
{
  Desk *desk = desk_new ();
  GArray *items;

  open_window (desk, "mate-terminal");
  open_window (desk, "org.gnome.Nautilus");
  open_window (desk, "mate-terminal");
  items = build (desk, NULL);
  g_assert_cmpuint (items->len, ==, 2);
  g_assert_cmpstr (item (items, 0)->key, ==, "mate-terminal.desktop");
  g_assert_cmpuint (item (items, 0)->windows->len, ==, 2);
  g_assert_cmpstr (item (items, 1)->key, ==, "org.gnome.Nautilus.desktop");
  g_array_unref (items);
  desk_free (desk);
}

static void
test_an_x11_window_joins_its_entrys_item_by_startup_wm_class (void)
{
  Desk *desk = desk_new ();
  GArray *items;

  open_window (desk, "firefox");
  open_window (desk, "Navigator");
  items = build (desk, NULL);
  g_assert_cmpuint (items->len, ==, 1);
  g_assert_cmpuint (item (items, 0)->windows->len, ==, 2);
  g_array_unref (items);
  desk_free (desk);
}

static void
test_a_window_with_no_entry_has_an_item_by_its_app_id (void)
{
  Desk *desk = desk_new ();
  GArray *items;

  open_window (desk, "mystery");
  open_window (desk, "mystery");
  open_window (desk, "other-mystery");
  items = build (desk, NULL);
  g_assert_cmpuint (items->len, ==, 2);
  g_assert_cmpstr (item (items, 0)->key, ==, "window:mystery");
  g_assert_null (item (items, 0)->entry_id);
  g_assert_cmpuint (item (items, 0)->windows->len, ==, 2);
  g_assert_cmpstr (item (items, 1)->key, ==, "window:other-mystery");
  g_array_unref (items);
  desk_free (desk);
}

static void
test_kept_items_come_first_in_the_users_order_and_stay_with_no_window (void)
{
  Desk *desk = desk_new ();
  const char *const kept[] = { "org.gnome.Nautilus.desktop", "firefox.desktop", NULL };
  GArray *items;

  open_window (desk, "mate-terminal");
  open_window (desk, "firefox");
  items = build (desk, kept);
  g_assert_cmpuint (items->len, ==, 3);
  g_assert_cmpstr (item (items, 0)->key, ==, "org.gnome.Nautilus.desktop");
  g_assert_true (item (items, 0)->kept);
  g_assert_cmpuint (item (items, 0)->windows->len, ==, 0);
  g_assert_cmpstr (item (items, 1)->key, ==, "firefox.desktop");
  g_assert_true (item (items, 1)->kept);
  g_assert_cmpuint (item (items, 1)->windows->len, ==, 1);
  g_assert_cmpstr (item (items, 2)->key, ==, "mate-terminal.desktop");
  g_assert_false (item (items, 2)->kept);
  g_array_unref (items);
  desk_free (desk);
}

static void
test_an_item_that_isnt_kept_goes_with_its_last_window (void)
{
  Desk *desk = desk_new ();
  GArray *items;

  open_window (desk, "mate-terminal");
  g_array_set_size (desk->windows, 0);
  items = build (desk, NULL);
  g_assert_cmpuint (items->len, ==, 0);
  g_array_unref (items);
  desk_free (desk);
}

static void
test_running_items_keep_the_order_their_programs_were_opened_in (void)
{
  Desk *desk = desk_new ();
  GArray *items;
  DkWindow *terminal;

  terminal = open_window (desk, "mate-terminal");
  open_window (desk, "firefox");
  focus (desk, terminal);
  items = build (desk, NULL);
  g_assert_cmpstr (item (items, 0)->key, ==, "mate-terminal.desktop");
  g_assert_cmpstr (item (items, 1)->key, ==, "firefox.desktop");
  g_array_unref (items);
  desk_free (desk);
}

static void
test_a_kept_key_that_is_a_duplicate_or_a_window_item_is_one_item (void)
{
  Desk *desk = desk_new ();
  const char *const kept[] = { "firefox.desktop", "firefox.desktop", NULL };
  GArray *items;

  items = build (desk, kept);
  g_assert_cmpuint (items->len, ==, 1);
  g_array_unref (items);
  desk_free (desk);
}

static void
test_a_click_launches_a_program_that_isnt_running (void)
{
  Desk *desk = desk_new ();
  const char *const kept[] = { "firefox.desktop", NULL };
  const DkWindow *target = GINT_TO_POINTER (1);
  GArray *items = build (desk, kept);

  g_assert_cmpint (dk_item_click (item (items, 0), &target), ==, DK_CLICK_LAUNCH);
  g_assert_null (target);
  g_array_unref (items);
  desk_free (desk);
}

static void
test_a_click_activates_the_most_recent_window (void)
{
  Desk *desk = desk_new ();
  const DkWindow *target;
  GArray *items;
  DkWindow *first = open_window (desk, "mate-terminal");

  open_window (desk, "mate-terminal");
  open_window (desk, "mate-terminal");
  focus (desk, first);
  items = build (desk, NULL);
  /* The focused one is the most recent, though another was opened last; with several windows
     a click doesn't minimise it. */
  g_assert_cmpint (dk_item_click (item (items, 0), &target), ==, DK_CLICK_ACTIVATE);
  g_assert_true (target->handle == first->handle);
  g_array_unref (items);

  first->active = FALSE;
  items = build (desk, NULL);
  g_assert_cmpint (dk_item_click (item (items, 0), &target), ==, DK_CLICK_ACTIVATE);
  g_assert_true (target->handle == first->handle);
  g_array_unref (items);
  desk_free (desk);
}

static void
test_a_click_minimises_the_one_window_of_a_focused_item (void)
{
  Desk *desk = desk_new ();
  const DkWindow *target;
  GArray *items;
  DkWindow *window = open_window (desk, "mate-terminal");

  focus (desk, window);
  items = build (desk, NULL);
  g_assert_cmpint (dk_item_click (item (items, 0), &target), ==, DK_CLICK_MINIMISE);
  g_assert_true (target->handle == window->handle);
  g_array_unref (items);
  desk_free (desk);
}

static void
test_a_click_on_a_minimised_window_activates_it (void)
{
  Desk *desk = desk_new ();
  const DkWindow *target;
  GArray *items;
  DkWindow *window = open_window (desk, "mate-terminal");

  window->minimised = TRUE;
  items = build (desk, NULL);
  g_assert_cmpint (dk_item_click (item (items, 0), &target), ==, DK_CLICK_ACTIVATE);
  /* Even if the compositor still reports it active. */
  window->active = TRUE;
  g_array_unref (items);
  items = build (desk, NULL);
  g_assert_cmpint (dk_item_click (item (items, 0), &target), ==, DK_CLICK_ACTIVATE);
  g_array_unref (items);
  desk_free (desk);
}

static void
test_keeping_adds_at_the_end_and_once (void)
{
  GPtrArray *kept = g_ptr_array_new_with_free_func (g_free);

  g_assert_true (dk_kept_add (kept, "a.desktop"));
  g_assert_true (dk_kept_add (kept, "b.desktop"));
  g_assert_false (dk_kept_add (kept, "a.desktop"));
  g_assert_cmpuint (kept->len, ==, 2);
  g_assert_cmpstr (kept->pdata[1], ==, "b.desktop");
  g_ptr_array_unref (kept);
}

static void
test_removing_takes_a_key_out_and_only_a_kept_one (void)
{
  GPtrArray *kept = g_ptr_array_new_with_free_func (g_free);

  dk_kept_add (kept, "a.desktop");
  dk_kept_add (kept, "b.desktop");
  g_assert_true (dk_kept_remove (kept, "a.desktop"));
  g_assert_false (dk_kept_remove (kept, "a.desktop"));
  g_assert_cmpuint (kept->len, ==, 1);
  g_assert_cmpstr (kept->pdata[0], ==, "b.desktop");
  g_ptr_array_unref (kept);
}

static void
test_moving_puts_a_key_at_a_position (void)
{
  GPtrArray *kept = g_ptr_array_new_with_free_func (g_free);

  dk_kept_add (kept, "a.desktop");
  dk_kept_add (kept, "b.desktop");
  dk_kept_add (kept, "c.desktop");
  /* To the front. */
  g_assert_true (dk_kept_move (kept, "c.desktop", 0));
  g_assert_cmpstr (kept->pdata[0], ==, "c.desktop");
  g_assert_cmpstr (kept->pdata[1], ==, "a.desktop");
  /* Dropped before the position it came from, or just after: where it is. */
  g_assert_false (dk_kept_move (kept, "c.desktop", 0));
  g_assert_false (dk_kept_move (kept, "c.desktop", 1));
  /* To the end: any position past the last. */
  g_assert_true (dk_kept_move (kept, "c.desktop", 3));
  g_assert_cmpstr (kept->pdata[2], ==, "c.desktop");
  g_assert_cmpstr (kept->pdata[0], ==, "a.desktop");
  /* A key that isn't kept is added there. */
  g_assert_true (dk_kept_move (kept, "d.desktop", 1));
  g_assert_cmpuint (kept->len, ==, 4);
  g_assert_cmpstr (kept->pdata[1], ==, "d.desktop");
  g_ptr_array_unref (kept);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/model/one-item-for-two-processes-of-one-program",
                   test_one_item_for_two_processes_of_one_program);
  g_test_add_func ("/model/an-x11-window-joins-its-entrys-item-by-startup-wm-class",
                   test_an_x11_window_joins_its_entrys_item_by_startup_wm_class);
  g_test_add_func ("/model/a-window-with-no-entry-has-an-item-by-its-app-id",
                   test_a_window_with_no_entry_has_an_item_by_its_app_id);
  g_test_add_func ("/model/kept-items-come-first-in-the-users-order-and-stay-with-no-window",
                   test_kept_items_come_first_in_the_users_order_and_stay_with_no_window);
  g_test_add_func ("/model/an-item-that-isnt-kept-goes-with-its-last-window",
                   test_an_item_that_isnt_kept_goes_with_its_last_window);
  g_test_add_func ("/model/running-items-keep-the-order-their-programs-were-opened-in",
                   test_running_items_keep_the_order_their_programs_were_opened_in);
  g_test_add_func ("/model/a-duplicate-kept-key-is-one-item",
                   test_a_kept_key_that_is_a_duplicate_or_a_window_item_is_one_item);
  g_test_add_func ("/click/launches-a-program-that-isnt-running",
                   test_a_click_launches_a_program_that_isnt_running);
  g_test_add_func ("/click/activates-the-most-recent-window",
                   test_a_click_activates_the_most_recent_window);
  g_test_add_func ("/click/minimises-the-one-window-of-a-focused-item",
                   test_a_click_minimises_the_one_window_of_a_focused_item);
  g_test_add_func ("/click/on-a-minimised-window-activates-it",
                   test_a_click_on_a_minimised_window_activates_it);
  g_test_add_func ("/kept/adds-at-the-end-and-once", test_keeping_adds_at_the_end_and_once);
  g_test_add_func ("/kept/removes-only-a-kept-key", test_removing_takes_a_key_out_and_only_a_kept_one);
  g_test_add_func ("/kept/moves-a-key-to-a-position", test_moving_puts_a_key_at_a_position);
  return g_test_run ();
}
