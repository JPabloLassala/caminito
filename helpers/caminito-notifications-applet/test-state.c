/*
 * The Notifications applet's state (CMN-260): entries read from a List reply built by hand,
 * as the daemon's private API gives it, an entry's age, the tooltip, the history grouped by
 * app (CMN-265), the icon, and when Do Not Disturb ends (CMN-263).
 */

#include "nt-state.h"

/* An entry of List's reply, with an image given as a string. */
static GVariant *
entry (guint32 id, const char *app, const char *summary, const char *image, gboolean has_default,
       const char *reply, guint64 time)
{
  GVariantBuilder actions;

  g_variant_builder_init (&actions, G_VARIANT_TYPE ("a(ss)"));
  g_variant_builder_add (&actions, "(ss)", "reply", "Reply");
  g_variant_builder_add (&actions, "(ss)", "archive", "Archive");
  return g_variant_new ("(usssssv@a(ss)bbst)", id, app, "Mail", "mail-unread", summary,
                        "From <b>Ana</b>", g_variant_new_string (image),
                        g_variant_builder_end (&actions), has_default, FALSE, reply, time);
}

static GVariant *
list (GVariant **entries, guint n)
{
  GVariantBuilder builder;

  g_variant_builder_init (&builder, G_VARIANT_TYPE ("a(usssssva(ss)bbst)"));
  for (guint i = 0; i < n; i++)
    g_variant_builder_add_value (&builder, entries[i]);
  return g_variant_ref_sink (g_variant_new ("(a(usssssva(ss)bbst))", &builder));
}

static void
test_entries_keep_the_lists_order_and_fields (void)
{
  GVariant *entries[] = {
    entry (7, "org.gnome.Evolution", "Lunch?", "/tmp/photo.png", TRUE, "Reply to Ana", 1700000300),
    entry (3, "org.gnome.Evolution", "Invoice", "", FALSE, "", 1700000000),
  };
  g_autoptr (GVariant) reply = list (entries, G_N_ELEMENTS (entries));
  g_autoptr (GPtrArray) read = nt_entries_from_list (reply);
  const NtEntry *newest, *oldest;
  const NtAction *action;

  g_assert_nonnull (read);
  g_assert_cmpuint (read->len, ==, 2);
  newest = g_ptr_array_index (read, 0);
  oldest = g_ptr_array_index (read, 1);
  g_assert_cmpuint (newest->id, ==, 7);
  g_assert_cmpuint (oldest->id, ==, 3);
  g_assert_cmpstr (newest->app, ==, "org.gnome.Evolution");
  g_assert_cmpstr (newest->app_name, ==, "Mail");
  g_assert_cmpstr (newest->app_icon, ==, "mail-unread");
  g_assert_cmpstr (newest->summary, ==, "Lunch?");
  g_assert_cmpstr (newest->body, ==, "From <b>Ana</b>");
  g_assert_cmpstr (newest->image_icon, ==, "/tmp/photo.png");
  g_assert_null (newest->image_pixels);
  g_assert_true (newest->has_default);
  g_assert_false (oldest->has_default);
  g_assert_cmpstr (newest->reply, ==, "Reply to Ana");
  g_assert_cmpstr (oldest->reply, ==, "");
  g_assert_cmpint (newest->time, ==, 1700000300);
  g_assert_cmpuint (newest->actions->len, ==, 2);
  action = g_ptr_array_index (newest->actions, 1);
  g_assert_cmpstr (action->key, ==, "archive");
  g_assert_cmpstr (action->label, ==, "Archive");
}

static void
test_an_image_of_pixels_is_kept_as_pixels (void)
{
  static const guint8 pixels[] = { 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255 };
  GVariant *data = g_variant_new_fixed_array (G_VARIANT_TYPE_BYTE, pixels, sizeof pixels, 1);
  GVariant *image = g_variant_new ("(iiibii@ay)", 2, 2, 6, FALSE, 8, 3, data);
  GVariant *entries[] = {
    g_variant_new ("(usssssv@a(ss)bbst)", 1, "app", "App", "", "Hi", "",
                   image, g_variant_new_array (G_VARIANT_TYPE ("(ss)"), NULL, 0), FALSE,
                   FALSE, "", (guint64) 1),
  };
  g_autoptr (GVariant) reply = list (entries, 1);
  g_autoptr (GPtrArray) read = nt_entries_from_list (reply);
  const NtEntry *only = g_ptr_array_index (read, 0);

  g_assert_null (only->image_icon);
  g_assert_nonnull (only->image_pixels);
  g_assert_true (g_variant_is_of_type (only->image_pixels, G_VARIANT_TYPE ("(iiibiiay)")));
  g_assert_cmpuint (only->actions->len, ==, 0);
}

static void
test_a_reply_of_another_type_is_refused (void)
{
  g_autoptr (GVariant) reply = g_variant_ref_sink (g_variant_new ("(as)", NULL));

  g_assert_null (nt_entries_from_list (reply));
}

static void
test_the_age_reads_as_a_person_would_say_it (void)
{
  const gint64 now = 1700000000;
  const struct
  {
    gint64      ago;
    const char *said;
  } cases[] = {
    { 0, "now" },
    { 59, "now" },
    { 60, "1 min ago" },
    { 5 * 60 + 30, "5 min ago" },
    { 59 * 60 + 59, "59 min ago" },
    { 60 * 60, "1 h ago" },
    { 23 * 60 * 60 + 59 * 60, "23 h ago" },
    { 24 * 60 * 60, "yesterday" },
    { 47 * 60 * 60, "yesterday" },
    { 48 * 60 * 60, "2 days ago" },
    { 6 * 24 * 60 * 60, "6 days ago" },
    /* A clock set back since: it can't have come from the future. */
    { -120, "now" },
  };

  for (guint i = 0; i < G_N_ELEMENTS (cases); i++)
    {
      g_autofree char *age = nt_state_age (now - cases[i].ago, now);

      g_assert_cmpstr (age, ==, cases[i].said);
    }
}

static void
test_the_tooltip_counts_the_notifications (void)
{
  g_autofree char *none = nt_state_tooltip (0);
  g_autofree char *one = nt_state_tooltip (1);
  g_autofree char *three = nt_state_tooltip (3);

  g_assert_cmpstr (none, ==, "No notifications");
  g_assert_cmpstr (one, ==, "1 notification");
  g_assert_cmpstr (three, ==, "3 notifications");
}

static void
test_the_icon_is_a_bell (void)
{
  const char *const *names = nt_state_icon_names (FALSE);
  guint n = g_strv_length ((char **) names);

  g_assert_true (g_strv_contains (names, "notifications-symbolic"));
  /* MATE's own is a speech bubble, and would be found before any other theme's bell. */
  g_assert_false (g_strv_contains (names, "preferences-system-notifications-symbolic"));
  /* Caminito's own, in hicolor, where every theme ends. */
  g_assert_cmpstr (names[n - 1], ==, "caminito-bell-symbolic");
}

/* An entry of app's, built by hand, with nothing else. */
static NtEntry *
app_entry (guint32 id, const char *app)
{
  NtEntry *entry = g_new0 (NtEntry, 1);

  entry->id = id;
  entry->app = g_strdup (app);
  entry->actions = g_ptr_array_new ();
  return entry;
}

/* The ids of a group's entries, as "9 7". */
static char *
ids_of (const NtGroup *group)
{
  GString *ids = g_string_new (NULL);

  for (guint i = 0; i < group->entries->len; i++)
    {
      const NtEntry *entry = g_ptr_array_index (group->entries, i);

      g_string_append_printf (ids, i == 0 ? "%u" : " %u", entry->id);
    }
  return g_string_free (ids, FALSE);
}

static void
test_the_history_is_grouped_by_app_newest_group_first_but_an_ungrouped_apps_entries_stand_alone (void)
{
  g_autoptr (GPtrArray) entries = g_ptr_array_new_with_free_func ((GDestroyNotify) nt_entry_free);
  const char *const ungrouped[] = { "chat", NULL };
  g_autoptr (GPtrArray) groups = NULL;
  const struct
  {
    const char *app;
    gboolean    grouped;
    const char *ids;
  } expected[] = {
    { "mail", TRUE, "9 7" },
    { "chat", FALSE, "8" },
    { "chat", FALSE, "6" },
    { "calendar", TRUE, "5" },
  };

  g_ptr_array_add (entries, app_entry (9, "mail"));
  g_ptr_array_add (entries, app_entry (8, "chat"));
  g_ptr_array_add (entries, app_entry (7, "mail"));
  g_ptr_array_add (entries, app_entry (6, "chat"));
  g_ptr_array_add (entries, app_entry (5, "calendar"));
  groups = nt_state_groups (entries, ungrouped);

  g_assert_cmpuint (groups->len, ==, G_N_ELEMENTS (expected));
  for (guint i = 0; i < groups->len; i++)
    {
      const NtGroup *group = g_ptr_array_index (groups, i);
      g_autofree char *ids = ids_of (group);

      g_assert_cmpstr (group->app, ==, expected[i].app);
      g_assert_cmpint (group->grouped, ==, expected[i].grouped);
      g_assert_cmpstr (ids, ==, expected[i].ids);
    }
}

static void
test_the_icon_is_struck_through_during_do_not_disturb (void)
{
  const char *const *names = nt_state_icon_names (TRUE);
  guint n = g_strv_length ((char **) names);

  g_assert_cmpstr (names[0], ==, "notifications-disabled-symbolic");
  g_assert_false (g_strv_contains (names, "notifications-symbolic"));
  g_assert_cmpstr (names[n - 1], ==, "caminito-bell-disabled-symbolic");
}

static void
test_do_not_disturb_says_when_it_ends (void)
{
  g_autoptr (GTimeZone) utc = g_time_zone_new_utc ();
  /* 22:13:20 on 14 November 2023, in UTC. */
  gint64 now = 1700000000;
  g_autofree char *never = nt_state_until (0, now, utc);
  g_autofree char *today = nt_state_until (now + 30 * 60, now, utc);
  g_autofree char *tomorrow = nt_state_until (now + 4 * 60 * 60, now, utc);

  g_assert_cmpstr (never, ==, "Until turned off");
  g_assert_cmpstr (today, ==, "Until 22:43");
  g_assert_cmpstr (tomorrow, ==, "Until tomorrow, 02:13");
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/state/entries-keep-the-lists-order-and-fields", test_entries_keep_the_lists_order_and_fields);
  g_test_add_func ("/state/an-image-of-pixels-is-kept-as-pixels", test_an_image_of_pixels_is_kept_as_pixels);
  g_test_add_func ("/state/a-reply-of-another-type-is-refused", test_a_reply_of_another_type_is_refused);
  g_test_add_func ("/state/the-age-reads-as-a-person-would-say-it", test_the_age_reads_as_a_person_would_say_it);
  g_test_add_func ("/state/the-tooltip-counts-the-notifications", test_the_tooltip_counts_the_notifications);
  g_test_add_func ("/state/the-icon-is-a-bell", test_the_icon_is_a_bell);
  g_test_add_func ("/state/the-history-is-grouped-by-app-newest-group-first-but-an-ungrouped-apps-entries-stand-alone",
                   test_the_history_is_grouped_by_app_newest_group_first_but_an_ungrouped_apps_entries_stand_alone);
  g_test_add_func ("/state/the-icon-is-struck-through-during-do-not-disturb", test_the_icon_is_struck_through_during_do_not_disturb);
  g_test_add_func ("/state/do-not-disturb-says-when-it-ends", test_do_not_disturb_says_when_it_ends);
  return g_test_run ();
}
