/*
 * caminito-notifications as the Notifications applet uses it (CMN-260), against a mock of
 * its private API (notifications-mock.py) on a private session bus: the history read and
 * followed through HistoryChanged, the apps with grouping off followed through
 * UngroupedChanged (CMN-265), the calls that dismiss, clear and act on its entries, the
 * Notification center's being open and the daemon asking for it (CMN-285), and the daemon
 * leaving the bus and coming back.
 *
 *   test-daemon <python> <notifications-mock.py>
 */

#include <gio/gio.h>

#include "nt-daemon.h"

#define DAEMON_NAME "org.mate.Caminito.Notifications"
#define DAEMON_PATH "/org/mate/Caminito/Notifications"
#define MOCK        "org.caminito.NotificationsMock"

static const char *python;
static const char *template;

typedef struct
{
  GTestDBus       *bus;
  GDBusConnection *connection;
  GSubprocess     *mock;
  NtDaemon        *daemon;
  guint            changes;
  guint            center_requests;
} Fixture;

static void
on_changed (NtDaemon *daemon G_GNUC_UNUSED, gpointer data)
{
  ((Fixture *) data)->changes++;
}

static void
on_center_requested (NtDaemon *daemon G_GNUC_UNUSED, gpointer data)
{
  ((Fixture *) data)->center_requests++;
}

static void
start_mock (Fixture *fixture)
{
  g_autoptr (GError) error = NULL;

  fixture->mock = g_subprocess_new (G_SUBPROCESS_FLAGS_STDOUT_SILENCE, &error, python, "-m",
                                    "dbusmock", "--template", template, NULL);
  g_assert_no_error (error);
  /* The mock owns the name once it serves the template's objects. */
  for (int i = 0; i < 100; i++)
    {
      g_autoptr (GVariant) has = NULL;
      gboolean owned;

      has = g_dbus_connection_call_sync (fixture->connection, "org.freedesktop.DBus",
                                         "/org/freedesktop/DBus", "org.freedesktop.DBus",
                                         "NameHasOwner", g_variant_new ("(s)", DAEMON_NAME),
                                         G_VARIANT_TYPE ("(b)"), G_DBUS_CALL_FLAGS_NONE, -1,
                                         NULL, &error);
      g_assert_no_error (error);
      g_variant_get (has, "(b)", &owned);
      if (owned)
        return;
      g_usleep (100 * 1000);
    }
  g_error ("The mock never owned %s", DAEMON_NAME);
}

static void
stop_mock (Fixture *fixture)
{
  g_subprocess_force_exit (fixture->mock);
  g_subprocess_wait (fixture->mock, NULL, NULL);
  g_clear_object (&fixture->mock);
}

static GVariant *
mock_call (Fixture *fixture, const char *method, GVariant *parameters)
{
  g_autoptr (GError) error = NULL;
  GVariant *reply = g_dbus_connection_call_sync (fixture->connection, DAEMON_NAME, DAEMON_PATH,
                                                 MOCK, method, parameters, NULL,
                                                 G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);

  g_assert_no_error (error);
  return reply;
}

/* A new entry from Mail, with a Reply action, which arrived a minute ago. */
static void
add_entry (Fixture *fixture, guint32 id, const char *summary, gboolean resident)
{
  GVariantBuilder actions;

  g_variant_builder_init (&actions, G_VARIANT_TYPE ("a(ss)"));
  g_variant_builder_add (&actions, "(ss)", "reply", "Reply");
  g_variant_unref (mock_call (fixture, "AddEntry",
                              g_variant_new ("(ussssssa(ss)bbt)", id, "org.gnome.Evolution",
                                             "Mail", "mail-unread", summary, "", "", &actions,
                                             TRUE, resident, (guint64) 60)));
}

/* The mock's log, one line after the other, ";"-joined. */
static char *
mock_log (Fixture *fixture)
{
  g_autoptr (GVariant) reply = mock_call (fixture, "GetLog", NULL);
  g_autoptr (GVariant) lines = g_variant_get_child_value (reply, 0);
  g_autofree const char **strv = g_variant_get_strv (lines, NULL);

  return g_strjoinv (";", (char **) strv);
}

typedef gboolean (*Condition) (Fixture *fixture, gconstpointer expected);

/* Runs the main loop until condition holds, or fails after 5 s. */
static void
wait_until (Fixture *fixture, Condition condition, gconstpointer expected)
{
  gint64 deadline = g_get_monotonic_time () + 5 * G_USEC_PER_SEC;

  while (!condition (fixture, expected))
    {
      g_assert_cmpint (g_get_monotonic_time (), <, deadline);
      g_main_context_iteration (NULL, FALSE);
      g_usleep (10 * 1000);
    }
}

/* The entries as "id summary", ";"-joined. */
static char *
listed (Fixture *fixture)
{
  const GPtrArray *entries = nt_daemon_entries (fixture->daemon);
  GString *joined = g_string_new (NULL);

  for (guint i = 0; i < entries->len; i++)
    {
      const NtEntry *entry = g_ptr_array_index (entries, i);

      g_string_append_printf (joined, "%s%u %s", i > 0 ? ";" : "", entry->id, entry->summary);
    }
  return g_string_free (joined, FALSE);
}

static gboolean
list_is (Fixture *fixture, gconstpointer expected)
{
  g_autofree char *now = listed (fixture);

  return g_strcmp0 (now, expected) == 0;
}

/* Tells the mock which apps have grouping off. */
static void
set_ungrouped (Fixture *fixture, const char *const *apps)
{
  g_variant_unref (mock_call (fixture, "SetUngrouped", g_variant_new ("(^as)", apps)));
}

static gboolean
ungrouped_is (Fixture *fixture, gconstpointer expected)
{
  g_autofree char *now = g_strjoinv (";", (char **) nt_daemon_ungrouped (fixture->daemon));

  return g_strcmp0 (now, expected) == 0;
}

static gboolean
log_is (Fixture *fixture, gconstpointer expected)
{
  g_autofree char *now = mock_log (fixture);

  return g_strcmp0 (now, expected) == 0;
}

static gboolean
is_running (Fixture *fixture, gconstpointer expected)
{
  return nt_daemon_running (fixture->daemon) == GPOINTER_TO_INT (expected);
}

static gboolean
has_changed (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return fixture->changes > 0;
}

static gboolean
center_requested (Fixture *fixture, gconstpointer expected)
{
  return fixture->center_requests == GPOINTER_TO_UINT (expected);
}

static gboolean
connection_gone (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return fixture->connection == NULL;
}

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GError) error = NULL;

  fixture->bus = g_test_dbus_new (G_TEST_DBUS_NONE);
  g_test_dbus_up (fixture->bus);
  /* GDBus and dbusmock both read the session bus's address from here. */
  fixture->connection = g_bus_get_sync (G_BUS_TYPE_SESSION, NULL, &error);
  g_assert_no_error (error);
  g_dbus_connection_set_exit_on_close (fixture->connection, FALSE);
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_clear_pointer (&fixture->daemon, nt_daemon_free);
  if (fixture->mock != NULL)
    stop_mock (fixture);
  /* GDBus keeps the session bus's connection while anything holds it, and the next
     test's bus is a new one: wait for this one to go before taking the bus down. */
  g_object_add_weak_pointer (G_OBJECT (fixture->connection), (gpointer *) &fixture->connection);
  g_object_unref (fixture->connection);
  wait_until (fixture, connection_gone, NULL);
  g_test_dbus_down (fixture->bus);
  g_clear_object (&fixture->bus);
}

static void
test_without_the_daemon_nothing_runs (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  fixture->daemon = nt_daemon_new (on_changed, on_center_requested, fixture);
  /* The name watch tells it the name has no owner. */
  wait_until (fixture, has_changed, NULL);
  g_assert_false (nt_daemon_running (fixture->daemon));
  g_assert_cmpuint (nt_daemon_entries (fixture->daemon)->len, ==, 0);
}

static void
test_the_history_is_read_when_the_daemon_is_there (Fixture *fixture,
                                                    gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  add_entry (fixture, 1, "Invoice", FALSE);
  add_entry (fixture, 2, "Lunch?", FALSE);
  fixture->daemon = nt_daemon_new (on_changed, on_center_requested, fixture);
  wait_until (fixture, list_is, "2 Lunch?;1 Invoice");
  g_assert_true (nt_daemon_running (fixture->daemon));
}

static void
test_the_history_follows_the_daemon (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->daemon = nt_daemon_new (on_changed, on_center_requested, fixture);
  wait_until (fixture, is_running, GINT_TO_POINTER (TRUE));
  wait_until (fixture, list_is, "");
  add_entry (fixture, 4, "Build passed", FALSE);
  wait_until (fixture, list_is, "4 Build passed");
  add_entry (fixture, 5, "Meeting in 5 min", FALSE);
  wait_until (fixture, list_is, "5 Meeting in 5 min;4 Build passed");
}

static void
test_the_calls_reach_the_daemon (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  add_entry (fixture, 1, "Invoice", FALSE);
  add_entry (fixture, 2, "Lunch?", FALSE);
  add_entry (fixture, 3, "Ana is calling", TRUE);
  g_variant_unref (mock_call (fixture, "SetReply", g_variant_new ("(us)", 3, "Reply to Ana")));
  fixture->daemon = nt_daemon_new (on_changed, on_center_requested, fixture);
  wait_until (fixture, list_is, "3 Ana is calling;2 Lunch?;1 Invoice");
  g_assert_cmpstr (((NtEntry *) g_ptr_array_index (nt_daemon_entries (fixture->daemon), 0))->reply,
                   ==, "Reply to Ana");

  nt_daemon_dismiss (fixture->daemon, 1);
  wait_until (fixture, list_is, "3 Ana is calling;2 Lunch?");
  nt_daemon_invoke_action (fixture->daemon, 2, "default", "xdg-token");
  wait_until (fixture, list_is, "3 Ana is calling");
  /* A resident one stays. */
  nt_daemon_invoke_action (fixture->daemon, 3, "reply", NULL);
  wait_until (fixture, log_is,
              "Dismiss 1;InvokeAction 2 default \"xdg-token\";InvokeAction 3 reply \"\"");
  g_assert_true (list_is (fixture, "3 Ana is calling"));
  nt_daemon_reply (fixture->daemon, 3, "On my way");
  wait_until (fixture, log_is,
              "Dismiss 1;InvokeAction 2 default \"xdg-token\";InvokeAction 3 reply \"\";"
              "Reply 3 \"On my way\"");
  nt_daemon_clear_app (fixture->daemon, "org.gnome.Evolution");
  wait_until (fixture, list_is, "");
  nt_daemon_clear_all (fixture->daemon);
  wait_until (fixture, log_is,
              "Dismiss 1;InvokeAction 2 default \"xdg-token\";InvokeAction 3 reply \"\";"
              "Reply 3 \"On my way\";ClearApp org.gnome.Evolution;ClearAll");
}

static void
test_the_apps_with_grouping_off_follow_the_daemon (Fixture *fixture,
                                                   gconstpointer data G_GNUC_UNUSED)
{
  const char *const chat[] = { "chat", NULL };
  const char *const both[] = { "chat", "mail", NULL };

  start_mock (fixture);
  set_ungrouped (fixture, chat);
  fixture->daemon = nt_daemon_new (on_changed, on_center_requested, fixture);
  wait_until (fixture, ungrouped_is, "chat");
  set_ungrouped (fixture, both);
  wait_until (fixture, ungrouped_is, "chat;mail");

  stop_mock (fixture);
  wait_until (fixture, ungrouped_is, "");
}

static void
test_the_daemon_leaving_and_coming_back (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  add_entry (fixture, 1, "Invoice", FALSE);
  fixture->daemon = nt_daemon_new (on_changed, on_center_requested, fixture);
  wait_until (fixture, list_is, "1 Invoice");

  stop_mock (fixture);
  wait_until (fixture, is_running, GINT_TO_POINTER (FALSE));
  g_assert_true (list_is (fixture, ""));

  start_mock (fixture);
  wait_until (fixture, is_running, GINT_TO_POINTER (TRUE));
  add_entry (fixture, 9, "Back", FALSE);
  wait_until (fixture, list_is, "9 Back");
}

static void
test_the_center_is_told_and_asked_for (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = NULL;

  start_mock (fixture);
  fixture->daemon = nt_daemon_new (on_changed, on_center_requested, fixture);
  wait_until (fixture, is_running, GINT_TO_POINTER (TRUE));
  nt_daemon_set_center_open (fixture->daemon, TRUE);
  wait_until (fixture, log_is, "SetCenterOpen true");

  /* As the bubbles' "N more notifications" asks. */
  reply = g_dbus_connection_call_sync (fixture->connection, DAEMON_NAME, DAEMON_PATH,
                                       DAEMON_NAME, "OpenCenter", NULL, NULL,
                                       G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);
  g_assert_no_error (error);
  wait_until (fixture, center_requested, GUINT_TO_POINTER (1));

  /* A daemon that comes back hears that the center is open. */
  stop_mock (fixture);
  wait_until (fixture, is_running, GINT_TO_POINTER (FALSE));
  start_mock (fixture);
  wait_until (fixture, log_is, "SetCenterOpen true");
  nt_daemon_set_center_open (fixture->daemon, FALSE);
  wait_until (fixture, log_is, "SetCenterOpen true;SetCenterOpen false");
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  if (argc < 3)
    g_error ("usage: test-daemon <python> <notifications-mock.py>");
  python = argv[1];
  template = argv[2];
  g_test_add ("/daemon/without-the-daemon-nothing-runs", Fixture, NULL, setup, test_without_the_daemon_nothing_runs, teardown);
  g_test_add ("/daemon/the-history-is-read-when-the-daemon-is-there", Fixture, NULL, setup, test_the_history_is_read_when_the_daemon_is_there, teardown);
  g_test_add ("/daemon/the-history-follows-the-daemon", Fixture, NULL, setup, test_the_history_follows_the_daemon, teardown);
  g_test_add ("/daemon/the-calls-reach-the-daemon", Fixture, NULL, setup, test_the_calls_reach_the_daemon, teardown);
  g_test_add ("/daemon/the-center-is-told-and-asked-for", Fixture, NULL, setup, test_the_center_is_told_and_asked_for, teardown);
  g_test_add ("/daemon/the-apps-with-grouping-off-follow-the-daemon", Fixture, NULL, setup, test_the_apps_with_grouping_off_follow_the_daemon, teardown);
  g_test_add ("/daemon/the-daemon-leaving-and-coming-back", Fixture, NULL, setup, test_the_daemon_leaving_and_coming_back, teardown);
  return g_test_run ();
}
