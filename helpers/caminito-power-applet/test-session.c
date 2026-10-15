/*
 * mate-session as the Power applet uses it (265), against a mock of its inhibitor calls
 * (session-mock.py) on a private session bus: Stay awake's Inhibit and Uninhibit with the
 * same cookie, the list of what keeps the computer awake as applications add and remove
 * inhibitors, and mate-session stopping and starting again.
 *
 *   test-session <python> <session-mock.py>
 */

#include <gio/gio.h>

#include "pw-session.h"

#define SESSION_NAME "org.gnome.SessionManager"
#define SESSION_PATH "/org/gnome/SessionManager"
#define MOCK         "org.caminito.SessionMock"

static const char *python;
static const char *template;

typedef struct
{
  GTestDBus       *bus;
  GDBusConnection *connection;
  GSubprocess     *mock;
  PwSession       *session;
  guint            changes;
} Fixture;

static void
on_changed (PwSession *session G_GNUC_UNUSED, gpointer data)
{
  ((Fixture *) data)->changes++;
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
                                         "NameHasOwner", g_variant_new ("(s)", SESSION_NAME),
                                         G_VARIANT_TYPE ("(b)"), G_DBUS_CALL_FLAGS_NONE, -1,
                                         NULL, &error);
      g_assert_no_error (error);
      g_variant_get (has, "(b)", &owned);
      if (owned)
        return;
      g_usleep (100 * 1000);
    }
  g_error ("The mock never owned %s", SESSION_NAME);
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
  GVariant *reply = g_dbus_connection_call_sync (fixture->connection, SESSION_NAME, SESSION_PATH,
                                                 MOCK, method, parameters, NULL,
                                                 G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);

  g_assert_no_error (error);
  return reply;
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

static char *
add_inhibitor (Fixture *fixture, const char *app_id, const char *reason, guint flags)
{
  g_autoptr (GVariant) reply = mock_call (fixture, "AddInhibitor",
                                          g_variant_new ("(ssu)", app_id, reason, flags));
  char *path;

  g_variant_get (reply, "(o)", &path);
  return path;
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

/* The inhibitors as "app id (reason)", ";"-joined. */
static char *
listed (Fixture *fixture)
{
  const GPtrArray *inhibitors = pw_session_inhibitors (fixture->session);
  GString *joined = g_string_new (NULL);

  for (guint i = 0; i < inhibitors->len; i++)
    {
      const PwInhibitor *inhibitor = g_ptr_array_index (inhibitors, i);

      g_string_append_printf (joined, "%s%s (%s)", i > 0 ? ";" : "", inhibitor->app_id,
                              inhibitor->reason);
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
stay_awake_is_off (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return !pw_session_stay_awake (fixture->session);
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
  g_clear_pointer (&fixture->session, pw_session_free);
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

/* Starts the session and waits until it has read mate-session's list, which it tells the
   applet of: there is a connection to call over from then on. */
static void
start_session (Fixture *fixture)
{
  fixture->session = pw_session_new (on_changed, fixture);
  wait_until (fixture, has_changed, NULL);
}

static void
test_stay_awake_inhibits_and_uninhibits (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  start_session (fixture);
  g_assert_false (pw_session_stay_awake (fixture->session));

  pw_session_set_stay_awake (fixture->session, TRUE);
  g_assert_true (pw_session_stay_awake (fixture->session));
  /* Suspend (4) and idle (8): 12, with no window. */
  wait_until (fixture, log_is, "Inhibit \"caminito-power-applet\" 0 \"Stay awake is on\" 12 -> 1");
  /* Stay awake is in the list, as an inhibitor like the others. */
  wait_until (fixture, list_is, "caminito-power-applet (Stay awake is on)");

  pw_session_set_stay_awake (fixture->session, FALSE);
  g_assert_false (pw_session_stay_awake (fixture->session));
  /* The cookie it was given. */
  wait_until (fixture, log_is,
              "Inhibit \"caminito-power-applet\" 0 \"Stay awake is on\" 12 -> 1;Uninhibit 1");
  wait_until (fixture, list_is, "");

  /* And on again: a new inhibitor, a new cookie. */
  pw_session_set_stay_awake (fixture->session, TRUE);
  wait_until (fixture, log_is,
              "Inhibit \"caminito-power-applet\" 0 \"Stay awake is on\" 12 -> 1;Uninhibit 1;"
              "Inhibit \"caminito-power-applet\" 0 \"Stay awake is on\" 12 -> 2");
}

/* Turned off before mate-session has answered Inhibit: the cookie comes, and goes. */
static void
test_off_before_the_cookie_comes (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  start_session (fixture);

  pw_session_set_stay_awake (fixture->session, TRUE);
  pw_session_set_stay_awake (fixture->session, FALSE);
  wait_until (fixture, log_is,
              "Inhibit \"caminito-power-applet\" 0 \"Stay awake is on\" 12 -> 1;Uninhibit 1");
  wait_until (fixture, list_is, "");
  g_assert_false (pw_session_stay_awake (fixture->session));
}

/* Off and on again before it has answered: one inhibitor, kept. */
static void
test_on_again_before_the_cookie_comes (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  start_session (fixture);

  pw_session_set_stay_awake (fixture->session, TRUE);
  pw_session_set_stay_awake (fixture->session, FALSE);
  pw_session_set_stay_awake (fixture->session, TRUE);
  wait_until (fixture, list_is, "caminito-power-applet (Stay awake is on)");
  /* Turned off later, the cookie is the one that came. */
  pw_session_set_stay_awake (fixture->session, FALSE);
  wait_until (fixture, log_is,
              "Inhibit \"caminito-power-applet\" 0 \"Stay awake is on\" 12 -> 1;Uninhibit 1");
}

static void
test_applications_are_listed_as_they_come_and_go (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *video = NULL;
  g_autofree char *backup = NULL;
  g_autofree char *logout = NULL;
  g_autoptr (GVariant) ignored = NULL;

  start_mock (fixture);
  start_session (fixture);

  video = add_inhibitor (fixture, "firefox", "Playing video", 8);
  wait_until (fixture, list_is, "firefox (Playing video)");
  backup = add_inhibitor (fixture, "org.example.Backup", "Backing up", 4 | 1);
  wait_until (fixture, list_is, "firefox (Playing video);org.example.Backup (Backing up)");
  /* Only a logout or an automount isn't what keeps the computer awake. */
  logout = add_inhibitor (fixture, "pluma", "Unsaved changes", 1);
  add_inhibitor (fixture, "udisks", "Mounting", 16);
  /* The list is read again after each signal: give the last ones time to be. */
  pw_session_set_stay_awake (fixture->session, TRUE);
  wait_until (fixture, list_is,
              "firefox (Playing video);org.example.Backup (Backing up);"
              "caminito-power-applet (Stay awake is on)");

  ignored = mock_call (fixture, "RemoveInhibitor", g_variant_new ("(o)", video));
  wait_until (fixture, list_is,
              "org.example.Backup (Backing up);caminito-power-applet (Stay awake is on)");
}

static void
test_an_inhibitor_with_no_app_id_is_listed (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *path = NULL;

  start_mock (fixture);
  start_session (fixture);
  path = add_inhibitor (fixture, "", "Burning a disc", 4);
  wait_until (fixture, list_is, " (Burning a disc)");
}

/* The applet is loaded before mate-session runs, as the panel can be. */
static void
test_late_start (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *path = NULL;

  fixture->session = pw_session_new (on_changed, fixture);
  /* Nothing to turn on: the switch goes back off. */
  g_test_expect_message (NULL, G_LOG_LEVEL_WARNING,
                         "can't turn Stay awake on: mate-session isn't running");
  wait_until (fixture, has_changed, NULL);
  pw_session_set_stay_awake (fixture->session, TRUE);
  g_test_assert_expected_messages ();
  g_assert_false (pw_session_stay_awake (fixture->session));

  start_mock (fixture);
  path = add_inhibitor (fixture, "firefox", "Playing video", 8);
  wait_until (fixture, list_is, "firefox (Playing video)");
  pw_session_set_stay_awake (fixture->session, TRUE);
  wait_until (fixture, list_is, "firefox (Playing video);caminito-power-applet (Stay awake is on)");
}

static void
test_mate_session_restarting (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *path = NULL;

  start_mock (fixture);
  start_session (fixture);
  path = add_inhibitor (fixture, "firefox", "Playing video", 8);
  pw_session_set_stay_awake (fixture->session, TRUE);
  wait_until (fixture, list_is, "firefox (Playing video);caminito-power-applet (Stay awake is on)");

  /* mate-session gone: its inhibitors and Stay awake with it. */
  stop_mock (fixture);
  wait_until (fixture, list_is, "");
  wait_until (fixture, stay_awake_is_off, NULL);

  start_mock (fixture);
  /* Stay awake is off, and stays off, and the list is the new mate-session's. */
  g_free (path);
  path = add_inhibitor (fixture, "totem", "Playing a movie", 8);
  wait_until (fixture, list_is, "totem (Playing a movie)");
  g_assert_false (pw_session_stay_awake (fixture->session));
}

/* A panel that drops the applet keeps running: Stay awake can't outlive it. */
static void
test_freeing_gives_stay_awake_back (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  start_session (fixture);
  pw_session_set_stay_awake (fixture->session, TRUE);
  wait_until (fixture, list_is, "caminito-power-applet (Stay awake is on)");

  g_clear_pointer (&fixture->session, pw_session_free);
  wait_until (fixture, log_is,
              "Inhibit \"caminito-power-applet\" 0 \"Stay awake is on\" 12 -> 1;Uninhibit 1");
}

static void
test_free_early (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture);
  /* Freed with the name watch, the list's calls and an Inhibit under way: nothing may call
     into it afterwards. */
  start_session (fixture);
  pw_session_set_stay_awake (fixture->session, TRUE);
  g_clear_pointer (&fixture->session, pw_session_free);
  for (int i = 0; i < 50; i++)
    {
      g_main_context_iteration (NULL, FALSE);
      g_usleep (10 * 1000);
    }
  /* The Inhibit that was out was given back. */
  wait_until (fixture, log_is,
              "Inhibit \"caminito-power-applet\" 0 \"Stay awake is on\" 12 -> 1;Uninhibit 1");
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  if (argc != 3)
    g_error ("usage: test-session <python> <session-mock.py>");
  python = argv[1];
  template = argv[2];
  g_test_add ("/session/stay-awake-inhibits-with-12-and-uninhibits-with-the-same-cookie", Fixture, NULL, setup, test_stay_awake_inhibits_and_uninhibits, teardown);
  g_test_add ("/session/off-before-the-cookie-comes-gives-it-back", Fixture, NULL, setup, test_off_before_the_cookie_comes, teardown);
  g_test_add ("/session/on-again-before-the-cookie-comes-keeps-it", Fixture, NULL, setup, test_on_again_before_the_cookie_comes, teardown);
  g_test_add ("/session/applications-are-listed-as-they-come-and-go", Fixture, NULL, setup, test_applications_are_listed_as_they_come_and_go, teardown);
  g_test_add ("/session/an-inhibitor-with-no-app-id-is-listed", Fixture, NULL, setup, test_an_inhibitor_with_no_app_id_is_listed, teardown);
  g_test_add ("/session/mate-session-starting-after-the-applet-is-followed", Fixture, NULL, setup, test_late_start, teardown);
  g_test_add ("/session/mate-session-restarting-turns-stay-awake-off", Fixture, NULL, setup, test_mate_session_restarting, teardown);
  g_test_add ("/session/freeing-gives-stay-awake-back", Fixture, NULL, setup, test_freeing_gives_stay_awake_back, teardown);
  g_test_add ("/session/freeing-with-calls-out-calls-nothing", Fixture, NULL, setup, test_free_early, teardown);
  return g_test_run ();
}
