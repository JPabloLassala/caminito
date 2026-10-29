/*
 * caminito-notifications as the Notifications applet uses it (CMN-260), against a mock of
 * its private API (notifications-mock.py) on a private session bus: the history read and
 * followed through HistoryChanged, the calls that dismiss, clear and act on its entries, the
 * daemon leaving the bus and coming back, and Do Not Disturb and the request for the
 * Notification center (CMN-263).
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
  guint            centers;
} Fixture;

static void
on_changed (NtDaemon *daemon G_GNUC_UNUSED, gpointer data)
{
  ((Fixture *) data)->changes++;
}

static void
on_open_center (NtDaemon *daemon G_GNUC_UNUSED, gpointer data)
{
  ((Fixture *) data)->centers++;
}

static NtDaemon *
new_daemon (Fixture *fixture)
{
  return nt_daemon_new (on_changed, on_open_center, fixture);
}

/* Calls the private API's method, as Caminito or another client would. */
static void
api_call (Fixture *fixture, const char *method, GVariant *parameters)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = g_dbus_connection_call_sync (
    fixture->connection, DAEMON_NAME, DAEMON_PATH, DAEMON_NAME, method, parameters, NULL,
    G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);

  g_assert_no_error (error);
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
connection_gone (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return fixture->connection == NULL;
}

/* Do Not Disturb as "on <end>" or "off", the end compared with expected's "on" alone when
   it's "on later". */
static gboolean
do_not_disturb_is (Fixture *fixture, gconstpointer expected)
{
  guint64 end = 0;
  gboolean on = nt_daemon_do_not_disturb (fixture->daemon, &end);
  g_autofree char *now = on ? g_strdup_printf ("on %" G_GUINT64_FORMAT, end) : g_strdup ("off");

  if (g_strcmp0 (expected, "on later") == 0)
    return on && end > (guint64) (g_get_real_time () / G_USEC_PER_SEC);
  return g_strcmp0 (now, expected) == 0;
}

static gboolean
center_requested (Fixture *fixture, gconstpointer expected)
{
  return fixture->centers == GPOINTER_TO_UINT (expected);
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
  fixture->daemon = new_daemon (fixture);
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
  fixture->daemon = new_daemon (fixture);
  wait_until (fixture, list_is, "2 Lunch?;1 Invoice");
  g_assert_true (nt_daemon_running (fixture->daemon));
}

static void
test_the_history_follows_the_daemon (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->daemon = new_daemon (fixture);
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
  fixture->daemon = new_daemon (fixture);
  wait_until (fixture, list_is, "3 Ana is calling;2 Lunch?;1 Invoice");

  nt_daemon_dismiss (fixture->daemon, 1);
  wait_until (fixture, list_is, "3 Ana is calling;2 Lunch?");
  nt_daemon_invoke_action (fixture->daemon, 2, "default");
  wait_until (fixture, list_is, "3 Ana is calling");
  /* A resident one stays. */
  nt_daemon_invoke_action (fixture->daemon, 3, "reply");
  wait_until (fixture, log_is, "Dismiss 1;InvokeAction 2 default \"\";InvokeAction 3 reply \"\"");
  g_assert_true (list_is (fixture, "3 Ana is calling"));
  nt_daemon_clear_all (fixture->daemon);
  wait_until (fixture, list_is, "");
  wait_until (fixture, log_is,
              "Dismiss 1;InvokeAction 2 default \"\";InvokeAction 3 reply \"\";ClearAll");
}

static void
test_the_daemon_leaving_and_coming_back (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  add_entry (fixture, 1, "Invoice", FALSE);
  fixture->daemon = new_daemon (fixture);
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
test_do_not_disturb_is_read_when_the_daemon_is_there (Fixture *fixture,
                                                      gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  api_call (fixture, "SetDoNotDisturb", g_variant_new ("(s)", "until-turned-off"));
  fixture->daemon = new_daemon (fixture);
  wait_until (fixture, do_not_disturb_is, "on 0");

  /* Off while the daemon isn't there. */
  stop_mock (fixture);
  wait_until (fixture, do_not_disturb_is, "off");
}

static void
test_do_not_disturb_is_set_and_followed (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->daemon = new_daemon (fixture);
  wait_until (fixture, is_running, GINT_TO_POINTER (TRUE));
  wait_until (fixture, do_not_disturb_is, "off");

  nt_daemon_set_do_not_disturb (fixture->daemon, "one-hour");
  wait_until (fixture, do_not_disturb_is, "on later");
  nt_daemon_toggle_do_not_disturb (fixture->daemon);
  wait_until (fixture, do_not_disturb_is, "off");
  nt_daemon_toggle_do_not_disturb (fixture->daemon);
  wait_until (fixture, do_not_disturb_is, "on 0");
  /* And as another client sets it. */
  api_call (fixture, "SetDoNotDisturb", g_variant_new ("(s)", "off"));
  wait_until (fixture, do_not_disturb_is, "off");
  g_assert_true (log_is (fixture, "SetDoNotDisturb one-hour;ToggleDoNotDisturb;"
                                  "ToggleDoNotDisturb;SetDoNotDisturb off"));
}

static void
test_open_center_reaches_the_applet (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  fixture->daemon = new_daemon (fixture);
  wait_until (fixture, is_running, GINT_TO_POINTER (TRUE));

  /* As Caminito calls it for Super+N. */
  api_call (fixture, "OpenCenter", NULL);
  wait_until (fixture, center_requested, GUINT_TO_POINTER (1));
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  if (argc < 3)
    g_error ("usage: test-daemon <python> <notifications-mock.py>");
  python = argv[1];
  template = argv[2];
  g_test_add ("/daemon/do-not-disturb-is-read-when-the-daemon-is-there", Fixture, NULL, setup, test_do_not_disturb_is_read_when_the_daemon_is_there, teardown);
  g_test_add ("/daemon/do-not-disturb-is-set-and-followed", Fixture, NULL, setup, test_do_not_disturb_is_set_and_followed, teardown);
  g_test_add ("/daemon/open-center-reaches-the-applet", Fixture, NULL, setup, test_open_center_reaches_the_applet, teardown);
  g_test_add ("/daemon/without-the-daemon-nothing-runs", Fixture, NULL, setup, test_without_the_daemon_nothing_runs, teardown);
  g_test_add ("/daemon/the-history-is-read-when-the-daemon-is-there", Fixture, NULL, setup, test_the_history_is_read_when_the_daemon_is_there, teardown);
  g_test_add ("/daemon/the-history-follows-the-daemon", Fixture, NULL, setup, test_the_history_follows_the_daemon, teardown);
  g_test_add ("/daemon/the-calls-reach-the-daemon", Fixture, NULL, setup, test_the_calls_reach_the_daemon, teardown);
  g_test_add ("/daemon/the-daemon-leaving-and-coming-back", Fixture, NULL, setup, test_the_daemon_leaving_and_coming_back, teardown);
  return g_test_run ();
}
