/*
 * The popup's Stay awake row and the list of what keeps the computer awake (265), from
 * inhibitors built by hand: what each state lists, and what the switch calls. Without a
 * display, GTK can't start, and it's skipped.
 */

#include "pw-awake.h"

typedef struct
{
  PwAwake   *awake;
  GtkWidget *box;
  GString   *calls;
} Fixture;

static void
on_set_stay_awake (gboolean on, gpointer data)
{
  g_string_append_printf (((Fixture *) data)->calls, "stay awake %d;", on);
}

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  static const PwAwakeActions actions = { on_set_stay_awake };

  fixture->calls = g_string_new (NULL);
  fixture->awake = pw_awake_new (&actions, fixture);
  fixture->box = g_object_ref_sink (gtk_box_new (GTK_ORIENTATION_VERTICAL, 0));
  gtk_box_pack_start (GTK_BOX (fixture->box), pw_awake_row (fixture->awake), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (fixture->box), pw_awake_inhibitors (fixture->awake), FALSE, FALSE,
                      0);
  /* The popup shows its content as a whole. */
  gtk_widget_show_all (fixture->box);
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  gtk_widget_destroy (fixture->box);
  g_object_unref (fixture->box);
  pw_awake_free (fixture->awake);
  g_string_free (fixture->calls, TRUE);
}

typedef struct
{
  GString *joined;
} Search;

static void
visit (GtkWidget *widget, gpointer data)
{
  Search *search = data;

  if (GTK_IS_LABEL (widget) && gtk_widget_get_visible (widget))
    g_string_append_printf (search->joined, "%s%s", search->joined->len > 0 ? "|" : "",
                            gtk_label_get_text (GTK_LABEL (widget)));
  if (GTK_IS_CONTAINER (widget))
    gtk_container_forall (GTK_CONTAINER (widget), visit, search);
}

/* The labels of widget, in order, joined with "|". */
static char *
texts (GtkWidget *widget)
{
  Search search = { g_string_new (NULL) };

  visit (widget, &search);
  return g_string_free (search.joined, FALSE);
}

static GtkWidget *
find_switch (GtkWidget *widget)
{
  GtkWidget *found = NULL;
  GList *children;

  if (GTK_IS_SWITCH (widget))
    return widget;
  if (!GTK_IS_CONTAINER (widget))
    return NULL;
  children = gtk_container_get_children (GTK_CONTAINER (widget));
  for (GList *l = children; l != NULL && found == NULL; l = l->next)
    found = find_switch (l->data);
  g_list_free (children);
  return found;
}

static GPtrArray *
inhibitors (const PwInhibitor *first, ...)
{
  GPtrArray *array = g_ptr_array_new_with_free_func ((GDestroyNotify) pw_inhibitor_free);
  va_list args;

  va_start (args, first);
  for (const PwInhibitor *i = first; i != NULL; i = va_arg (args, const PwInhibitor *))
    g_ptr_array_add (array, pw_inhibitor_new (i->app_id, i->reason, i->flags));
  va_end (args);
  return array;
}

static void
test_the_row_says_what_it_does (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *row = texts (pw_awake_row (fixture->awake));

  g_assert_cmpstr (row, ==, "Stay awake|Don't sleep or lock the screen");
  g_assert_false (gtk_switch_get_active (GTK_SWITCH (find_switch (fixture->box))));
}

static void
test_the_switch_calls_back_and_following_it_does_not (Fixture *fixture,
                                                      gconstpointer data G_GNUC_UNUSED)
{
  GtkSwitch *toggle = GTK_SWITCH (find_switch (pw_awake_row (fixture->awake)));

  gtk_switch_set_active (toggle, TRUE);
  g_assert_cmpstr (fixture->calls->str, ==, "stay awake 1;");
  gtk_switch_set_active (toggle, FALSE);
  g_assert_cmpstr (fixture->calls->str, ==, "stay awake 1;stay awake 0;");

  /* The switch as the applet has it, after a refused call or a restart: no call. */
  g_string_truncate (fixture->calls, 0);
  pw_awake_set_stay_awake (fixture->awake, TRUE);
  g_assert_true (gtk_switch_get_active (toggle));
  pw_awake_set_stay_awake (fixture->awake, FALSE);
  g_assert_false (gtk_switch_get_active (toggle));
  g_assert_cmpstr (fixture->calls->str, ==, "");
}

static void
test_no_inhibitor_hides_the_list (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) none = inhibitors (NULL);
  PwInhibitor video = { "no-such-app", "Playing video", 8 };
  g_autoptr (GPtrArray) some = inhibitors (&video, NULL);

  pw_awake_set_inhibitors (fixture->awake, none);
  g_assert_false (gtk_widget_get_visible (pw_awake_inhibitors (fixture->awake)));

  pw_awake_set_inhibitors (fixture->awake, some);
  g_assert_true (gtk_widget_get_visible (pw_awake_inhibitors (fixture->awake)));
  pw_awake_set_inhibitors (fixture->awake, none);
  g_assert_false (gtk_widget_get_visible (pw_awake_inhibitors (fixture->awake)));
  /* Shown as a whole again, which the popup does when it opens: still hidden. */
  gtk_widget_show_all (fixture->box);
  g_assert_false (gtk_widget_get_visible (pw_awake_inhibitors (fixture->awake)));
}

static void
test_a_line_has_the_name_and_the_reason (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  PwInhibitor video = { "no-such-app", "Playing video", 8 };
  PwInhibitor nameless = { "", "Burning a disc", 4 };
  PwInhibitor silent = { "org.example.Quiet", "", 4 };
  g_autoptr (GPtrArray) all = inhibitors (&video, &nameless, &silent, NULL);
  g_autofree char *lines = NULL;

  pw_awake_set_inhibitors (fixture->awake, all);
  lines = texts (pw_awake_inhibitors (fixture->awake));
  /* With no desktop file the app ID stands for the name, and with no app ID, "An
     application". A reason that's empty leaves the name alone. */
  g_assert_cmpstr (lines, ==,
                   "no-such-app|Playing video|An application|Burning a disc|org.example.Quiet");
}

static void
test_stay_awake_is_its_own_line (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  PwInhibitor video = { "no-such-app", "Playing video", 8 };
  PwInhibitor mine = { PW_STAY_AWAKE_APP_ID, PW_STAY_AWAKE_REASON, 12 };
  g_autoptr (GPtrArray) all = inhibitors (&video, &mine, NULL);
  g_autofree char *lines = NULL;

  pw_awake_set_inhibitors (fixture->awake, all);
  lines = texts (pw_awake_inhibitors (fixture->awake));
  g_assert_cmpstr (lines, ==, "no-such-app|Playing video|You|Stay awake is on");
}

/* The same list set again is the same lines, not more of them. */
static void
test_setting_again_replaces_the_lines (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  PwInhibitor video = { "no-such-app", "Playing video", 8 };
  g_autoptr (GPtrArray) some = inhibitors (&video, NULL);
  g_autofree char *lines = NULL;

  pw_awake_set_inhibitors (fixture->awake, some);
  pw_awake_set_inhibitors (fixture->awake, some);
  lines = texts (pw_awake_inhibitors (fixture->awake));
  g_assert_cmpstr (lines, ==, "no-such-app|Playing video");
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
  g_test_add ("/awake/the-row-says-what-it-does", Fixture, NULL, setup, test_the_row_says_what_it_does, teardown);
  g_test_add ("/awake/the-switch-calls-back-and-following-it-does-not", Fixture, NULL, setup, test_the_switch_calls_back_and_following_it_does_not, teardown);
  g_test_add ("/awake/no-inhibitor-hides-the-list", Fixture, NULL, setup, test_no_inhibitor_hides_the_list, teardown);
  g_test_add ("/awake/a-line-has-the-name-and-the-reason", Fixture, NULL, setup, test_a_line_has_the_name_and_the_reason, teardown);
  g_test_add ("/awake/stay-awake-is-its-own-line", Fixture, NULL, setup, test_stay_awake_is_its_own_line, teardown);
  g_test_add ("/awake/setting-again-replaces-the-lines", Fixture, NULL, setup, test_setting_again_replaces_the_lines, teardown);
  return g_test_run ();
}
