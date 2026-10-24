/*
 * The Dock's Launcher entries (CMN-227), from Update signals' arguments built by hand.
 */

#include "dk-launcher.h"

typedef struct
{
  DkCatalog *catalog;
  DkLauncher *launcher;
} Desk;

static Desk *
desk_new (void)
{
  Desk *desk = g_new0 (Desk, 1);

  desk->catalog = dk_catalog_new ();
  dk_catalog_add (desk->catalog, "org.mate.Caja.desktop", NULL);
  dk_catalog_add (desk->catalog, "firefox.desktop", "Navigator");
  desk->launcher = dk_launcher_new ();
  return desk;
}

static void
desk_free (Desk *desk)
{
  dk_catalog_free (desk->catalog);
  dk_launcher_free (desk->launcher);
  g_free (desk);
}

/* An Update signal's properties, from the text gdbus would take: "{'count': <int64 5>}". */
static GVariant *
props (const char *text)
{
  GVariant *variant = g_variant_parse (G_VARIANT_TYPE ("a{sv}"), text, NULL, NULL, NULL);

  g_assert_nonnull (variant);
  return g_variant_ref_sink (variant);
}

static gboolean
update (Desk *desk, const char *sender, const char *uri, const char *text)
{
  g_autoptr (GVariant) properties = props (text);

  return dk_launcher_update (desk->launcher, desk->catalog, sender, uri, properties);
}

static DkLauncherEntry
caja (Desk *desk)
{
  return dk_launcher_get (desk->launcher, "org.mate.Caja.desktop");
}

static void
test_an_entry_has_nothing_to_show_until_it_is_told (void)
{
  Desk *desk = desk_new ();
  DkLauncherEntry entry = caja (desk);

  g_assert_false (entry.count_visible);
  g_assert_cmpint (entry.count, ==, 0);
  g_assert_false (entry.progress_visible);
  g_assert_cmpfloat (entry.progress, ==, 0.0);
  g_assert_false (entry.urgent);
  desk_free (desk);
}

static void
test_the_count_and_whether_it_is_visible_are_taken (void)
{
  Desk *desk = desk_new ();

  g_assert_true (update (desk, ":1.5", "application://org.mate.Caja.desktop",
                         "{'count': <int64 5>, 'count-visible': <true>}"));

  g_assert_cmpint (caja (desk).count, ==, 5);
  g_assert_true (caja (desk).count_visible);
  desk_free (desk);
}

static void
test_the_progress_and_whether_it_is_visible_are_taken (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop",
          "{'progress': <0.4>, 'progress-visible': <true>}");

  g_assert_cmpfloat_with_epsilon (caja (desk).progress, 0.4, 1e-9);
  g_assert_true (caja (desk).progress_visible);
  desk_free (desk);
}

static void
test_urgent_is_taken (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'urgent': <true>}");
  g_assert_true (caja (desk).urgent);
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'urgent': <false>}");
  g_assert_false (caja (desk).urgent);
  desk_free (desk);
}

static void
test_a_property_of_the_wrong_type_is_ignored (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop",
          "{'count': <int32 5>, 'count-visible': <'yes'>, 'progress': <'half'>, "
          "'progress-visible': <1>, 'urgent': <int64 1>}");

  g_assert_cmpint (caja (desk).count, ==, 0);
  g_assert_false (caja (desk).count_visible);
  g_assert_cmpfloat (caja (desk).progress, ==, 0.0);
  g_assert_false (caja (desk).progress_visible);
  g_assert_false (caja (desk).urgent);
  desk_free (desk);
}

static void
test_a_wrong_typed_property_leaves_the_value_it_had (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop",
          "{'count': <int64 7>, 'count-visible': <true>}");
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'count': <'many'>}");

  g_assert_cmpint (caja (desk).count, ==, 7);
  desk_free (desk);
}

static void
test_unknown_keys_are_ignored_the_quicklist_too (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop",
          "{'quicklist': <'/com/canonical/dbusmenu'>, 'colour': <'red'>, "
          "'count': <int64 3>}");

  g_assert_cmpint (caja (desk).count, ==, 3);
  g_assert_false (caja (desk).count_visible);
  desk_free (desk);
}

static void
test_a_partial_update_keeps_the_other_properties (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop",
          "{'count': <int64 5>, 'count-visible': <true>, 'urgent': <true>}");
  update (desk, ":1.5", "application://org.mate.Caja.desktop",
          "{'progress': <0.25>, 'progress-visible': <true>}");
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'count': <int64 6>}");

  g_assert_cmpint (caja (desk).count, ==, 6);
  g_assert_true (caja (desk).count_visible);
  g_assert_true (caja (desk).urgent);
  g_assert_cmpfloat_with_epsilon (caja (desk).progress, 0.25, 1e-9);
  g_assert_true (caja (desk).progress_visible);
  desk_free (desk);
}

static void
test_an_empty_update_changes_nothing (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'count': <int64 5>}");
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "@a{sv} {}");

  g_assert_cmpint (caja (desk).count, ==, 5);
  desk_free (desk);
}

static void
test_the_count_above_ninety_nine_reads_ninety_nine_plus (void)
{
  g_autofree char *five = dk_launcher_badge_text (5);
  g_autofree char *ninety_nine = dk_launcher_badge_text (99);
  g_autofree char *hundred = dk_launcher_badge_text (100);
  g_autofree char *huge = dk_launcher_badge_text (G_MAXINT64);

  g_assert_cmpstr (five, ==, "5");
  g_assert_cmpstr (ninety_nine, ==, "99");
  g_assert_cmpstr (hundred, ==, "99+");
  g_assert_cmpstr (huge, ==, "99+");
}

static void
test_a_count_of_zero_or_less_has_no_badge (void)
{
  g_assert_null (dk_launcher_badge_text (0));
  g_assert_null (dk_launcher_badge_text (-3));
  g_assert_null (dk_launcher_badge_text (G_MININT64));
}

static void
test_the_progress_is_clamped_to_zero_and_one (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'progress': <1.7>}");
  g_assert_cmpfloat (caja (desk).progress, ==, 1.0);
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'progress': <-0.5>}");
  g_assert_cmpfloat (caja (desk).progress, ==, 0.0);
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'progress': <0.5>}");
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'progress': <double nan>}");
  g_assert_cmpfloat (caja (desk).progress, ==, 0.5);
  desk_free (desk);
}

static void
test_an_update_clears_nothing_but_what_it_carries (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop",
          "{'count': <int64 5>, 'count-visible': <true>}");
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'count-visible': <false>}");

  g_assert_false (caja (desk).count_visible);
  g_assert_cmpint (caja (desk).count, ==, 5);
  desk_free (desk);
}

static void
test_an_application_uri_resolves_with_or_without_desktop (void)
{
  Desk *desk = desk_new ();

  g_assert_true (update (desk, ":1.5", "application://org.mate.Caja.desktop",
                         "{'count': <int64 1>}"));
  g_assert_true (update (desk, ":1.5", "application://org.mate.Caja", "{'count': <int64 2>}"));
  g_assert_cmpint (caja (desk).count, ==, 2);
  g_assert_true (update (desk, ":1.5", "application://ORG.MATE.CAJA",
                         "{'count': <int64 3>}"));
  g_assert_cmpint (caja (desk).count, ==, 3);
  desk_free (desk);
}

static void
test_an_application_uri_falls_back_to_startup_wm_class (void)
{
  Desk *desk = desk_new ();

  g_assert_true (update (desk, ":1.5", "application://navigator", "{'count': <int64 9>}"));

  g_assert_cmpint (dk_launcher_get (desk->launcher, "firefox.desktop").count, ==, 9);
  desk_free (desk);
}

static void
test_a_uri_with_another_prefix_is_ignored (void)
{
  Desk *desk = desk_new ();

  g_assert_false (update (desk, ":1.5", "file://org.mate.Caja.desktop", "{'count': <int64 5>}"));
  g_assert_false (update (desk, ":1.5", "org.mate.Caja.desktop", "{'count': <int64 5>}"));
  g_assert_false (update (desk, ":1.5", "Application://org.mate.Caja.desktop",
                          "{'count': <int64 5>}"));
  g_assert_false (update (desk, ":1.5", "", "{'count': <int64 5>}"));

  g_assert_cmpint (caja (desk).count, ==, 0);
  desk_free (desk);
}

static void
test_a_uri_that_resolves_to_no_entry_is_ignored (void)
{
  Desk *desk = desk_new ();

  g_assert_false (update (desk, ":1.5", "application://nothing.desktop", "{'count': <int64 5>}"));
  g_assert_false (update (desk, ":1.5", "application://", "{'count': <int64 5>}"));
  g_assert_false (dk_launcher_get (desk->launcher, "nothing.desktop").count_visible);
  desk_free (desk);
}

static void
test_clearing_by_sender_clears_that_senders_entries_only (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'count': <int64 5>}");
  update (desk, ":1.9", "application://firefox.desktop", "{'count': <int64 2>}");

  g_assert_true (dk_launcher_clear_sender (desk->launcher, ":1.5"));

  g_assert_cmpint (caja (desk).count, ==, 0);
  g_assert_cmpint (dk_launcher_get (desk->launcher, "firefox.desktop").count, ==, 2);
  g_assert_false (dk_launcher_clear_sender (desk->launcher, ":1.5"));
  desk_free (desk);
}

static void
test_an_entry_belongs_to_the_sender_of_its_latest_update (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'count': <int64 5>}");
  update (desk, ":1.6", "application://org.mate.Caja.desktop", "{'count': <int64 6>}");

  g_assert_false (dk_launcher_clear_sender (desk->launcher, ":1.5"));
  g_assert_cmpint (caja (desk).count, ==, 6);
  g_assert_true (dk_launcher_clear_sender (desk->launcher, ":1.6"));
  g_assert_cmpint (caja (desk).count, ==, 0);
  desk_free (desk);
}

static void
test_clearing_by_id_clears_that_ids_entry_only (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'count': <int64 5>}");
  update (desk, ":1.5", "application://firefox.desktop", "{'count': <int64 2>}");

  g_assert_true (dk_launcher_clear_id (desk->launcher, "org.mate.Caja.desktop"));

  g_assert_cmpint (caja (desk).count, ==, 0);
  g_assert_cmpint (dk_launcher_get (desk->launcher, "firefox.desktop").count, ==, 2);
  g_assert_false (dk_launcher_clear_id (desk->launcher, "org.mate.Caja.desktop"));
  desk_free (desk);
}

static void
test_an_update_after_a_clear_starts_from_nothing (void)
{
  Desk *desk = desk_new ();

  update (desk, ":1.5", "application://org.mate.Caja.desktop",
          "{'count': <int64 5>, 'count-visible': <true>}");
  dk_launcher_clear_id (desk->launcher, "org.mate.Caja.desktop");
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'progress': <0.5>}");

  g_assert_false (caja (desk).count_visible);
  g_assert_cmpint (caja (desk).count, ==, 0);
  desk_free (desk);
}

/* The items as the dock builds them, for the keys given and the windows of the programs in
   running. */
static GArray *
items_of (Desk *desk, const char *const *kept, const char *const *running)
{
  g_autoptr (GArray) windows = g_array_new (FALSE, TRUE, sizeof (DkWindow));

  for (guint i = 0; running[i] != NULL; i++)
    {
      DkWindow window = { .handle = GUINT_TO_POINTER (i + 1), .app_id = running[i],
                          .title = running[i], .opened = i + 1 };

      g_array_append_val (windows, window);
    }
  return dk_items_build (desk->catalog, kept, (DkWindow *) windows->data, windows->len);
}

static void
sync (Desk *desk, const char *const *kept, const char *const *running)
{
  g_autoptr (GArray) items = items_of (desk, kept, running);

  dk_launcher_sync (desk->launcher, items);
}

static void
test_an_item_that_had_a_window_and_loses_it_loses_its_entry (void)
{
  const char *none[] = { NULL };
  const char *running[] = { "org.mate.Caja", NULL };
  Desk *desk = desk_new ();

  sync (desk, none, running);
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'count': <int64 5>}");
  sync (desk, none, running);
  g_assert_cmpint (caja (desk).count, ==, 5);

  sync (desk, none, none);

  g_assert_cmpint (caja (desk).count, ==, 0);
  desk_free (desk);
}

static void
test_a_kept_item_that_loses_its_window_loses_its_entry_too (void)
{
  const char *kept[] = { "org.mate.Caja.desktop", NULL };
  const char *running[] = { "org.mate.Caja", NULL };
  const char *none[] = { NULL };
  Desk *desk = desk_new ();

  sync (desk, kept, running);
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'count': <int64 5>}");

  sync (desk, kept, none);

  g_assert_cmpint (caja (desk).count, ==, 0);
  desk_free (desk);
}

static void
test_a_kept_item_that_never_had_a_window_keeps_its_entry (void)
{
  const char *kept[] = { "org.mate.Caja.desktop", NULL };
  const char *none[] = { NULL };
  Desk *desk = desk_new ();

  sync (desk, kept, none);
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'count': <int64 5>}");
  sync (desk, kept, none);

  g_assert_cmpint (caja (desk).count, ==, 5);
  desk_free (desk);
}

static void
test_an_entry_with_no_item_waits_for_the_item_to_appear (void)
{
  const char *none[] = { NULL };
  const char *running[] = { "org.mate.Caja", NULL };
  Desk *desk = desk_new ();

  sync (desk, none, none);
  update (desk, ":1.5", "application://org.mate.Caja.desktop",
          "{'count': <int64 5>, 'count-visible': <true>}");
  sync (desk, none, none);
  g_assert_cmpint (caja (desk).count, ==, 5);

  sync (desk, none, running);

  g_assert_cmpint (caja (desk).count, ==, 5);
  g_assert_true (caja (desk).count_visible);
  desk_free (desk);
}

static void
test_an_entry_set_after_its_window_went_stays_for_the_next_one (void)
{
  const char *none[] = { NULL };
  const char *running[] = { "org.mate.Caja", NULL };
  Desk *desk = desk_new ();

  sync (desk, none, running);
  sync (desk, none, none);
  update (desk, ":1.5", "application://org.mate.Caja.desktop", "{'count': <int64 5>}");
  sync (desk, none, none);

  g_assert_cmpint (caja (desk).count, ==, 5);
  desk_free (desk);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/entry/has-nothing-to-show-until-it-is-told",
                   test_an_entry_has_nothing_to_show_until_it_is_told);
  g_test_add_func ("/parse/count-and-visibility", test_the_count_and_whether_it_is_visible_are_taken);
  g_test_add_func ("/parse/progress-and-visibility",
                   test_the_progress_and_whether_it_is_visible_are_taken);
  g_test_add_func ("/parse/urgent", test_urgent_is_taken);
  g_test_add_func ("/parse/wrong-types-are-ignored", test_a_property_of_the_wrong_type_is_ignored);
  g_test_add_func ("/parse/wrong-type-leaves-the-value",
                   test_a_wrong_typed_property_leaves_the_value_it_had);
  g_test_add_func ("/parse/unknown-keys-are-ignored", test_unknown_keys_are_ignored_the_quicklist_too);
  g_test_add_func ("/parse/partial-update-keeps-the-rest",
                   test_a_partial_update_keeps_the_other_properties);
  g_test_add_func ("/parse/empty-update-changes-nothing", test_an_empty_update_changes_nothing);
  g_test_add_func ("/parse/visibility-alone-keeps-the-count",
                   test_an_update_clears_nothing_but_what_it_carries);
  g_test_add_func ("/badge/ninety-nine-plus", test_the_count_above_ninety_nine_reads_ninety_nine_plus);
  g_test_add_func ("/badge/zero-or-less-has-none", test_a_count_of_zero_or_less_has_no_badge);
  g_test_add_func ("/parse/progress-is-clamped", test_the_progress_is_clamped_to_zero_and_one);
  g_test_add_func ("/uri/resolves-with-or-without-desktop",
                   test_an_application_uri_resolves_with_or_without_desktop);
  g_test_add_func ("/uri/falls-back-to-startup-wm-class",
                   test_an_application_uri_falls_back_to_startup_wm_class);
  g_test_add_func ("/uri/another-prefix-is-ignored", test_a_uri_with_another_prefix_is_ignored);
  g_test_add_func ("/uri/no-entry-is-ignored", test_a_uri_that_resolves_to_no_entry_is_ignored);
  g_test_add_func ("/clear/by-sender", test_clearing_by_sender_clears_that_senders_entries_only);
  g_test_add_func ("/clear/belongs-to-the-latest-sender",
                   test_an_entry_belongs_to_the_sender_of_its_latest_update);
  g_test_add_func ("/clear/by-id", test_clearing_by_id_clears_that_ids_entry_only);
  g_test_add_func ("/clear/update-after-clear-starts-from-nothing",
                   test_an_update_after_a_clear_starts_from_nothing);
  g_test_add_func ("/lifetime/item-loses-its-window",
                   test_an_item_that_had_a_window_and_loses_it_loses_its_entry);
  g_test_add_func ("/lifetime/kept-item-loses-its-window",
                   test_a_kept_item_that_loses_its_window_loses_its_entry_too);
  g_test_add_func ("/lifetime/kept-item-that-never-had-a-window",
                   test_a_kept_item_that_never_had_a_window_keeps_its_entry);
  g_test_add_func ("/lifetime/entry-waits-for-its-item",
                   test_an_entry_with_no_item_waits_for_the_item_to_appear);
  g_test_add_func ("/lifetime/entry-set-after-the-window-went-stays",
                   test_an_entry_set_after_its_window_went_stays_for_the_next_one);
  return g_test_run ();
}
