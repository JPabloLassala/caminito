/*
 * mate-screensaver-command, against a stand-in org.mate.ScreenSaver on a bus of the test's
 * own, with the methods Caminito's shim has of it (caminito/src/dbus/screensaver.rs).
 *
 * The first argument is the path of the program.
 */

#include <locale.h>
#include <signal.h>
#include <sys/wait.h>

#include <gio/gio.h>

#define NAME      "org.mate.ScreenSaver"
#define PATH      "/org/mate/ScreenSaver"

static const char *program;

static const char saver_xml[] =
  "<node>"
  "  <interface name='org.mate.ScreenSaver'>"
  "    <method name='Lock'/>"
  "    <method name='SetActive'><arg name='value' type='b'/></method>"
  "    <method name='GetActive'><arg name='value' type='b' direction='out'/></method>"
  "    <method name='GetActiveTime'><arg name='seconds' type='u' direction='out'/></method>"
  "    <method name='GetInhibitors'><arg name='list' type='as' direction='out'/></method>"
  "    <method name='Inhibit'>"
  "      <arg name='application_name' type='s'/>"
  "      <arg name='reason' type='s'/>"
  "      <arg name='cookie' type='u' direction='out'/>"
  "    </method>"
  "  </interface>"
  "</node>";

typedef struct
{
  GTestDBus       *bus;
  GDBusConnection *connection;
  GMainContext    *context;
  GMainLoop       *loop;
  GThread         *thread;
  GMutex           lock;
  /* Each call, as its method and its arguments' text. */
  GPtrArray       *calls;
  gboolean         active;
  gboolean         inhibited;
} Saver;

static void
method_call (GDBusConnection       *connection G_GNUC_UNUSED,
             const char            *sender G_GNUC_UNUSED,
             const char            *path G_GNUC_UNUSED,
             const char            *interface G_GNUC_UNUSED,
             const char            *method,
             GVariant              *parameters,
             GDBusMethodInvocation *invocation,
             gpointer               data)
{
  Saver *saver = data;
  g_autofree char *args = g_variant_print (parameters, FALSE);
  GVariant *reply = NULL;

  g_mutex_lock (&saver->lock);
  g_ptr_array_add (saver->calls, g_strdup_printf ("%s %s", method, args));
  if (g_str_equal (method, "GetActive"))
    reply = g_variant_new ("(b)", saver->active);
  else if (g_str_equal (method, "GetActiveTime"))
    reply = g_variant_new ("(u)", 42);
  else if (g_str_equal (method, "Inhibit"))
    reply = g_variant_new ("(u)", 7);
  else if (g_str_equal (method, "GetInhibitors"))
    {
      const char *list[] = { "pluma: a reason", NULL };

      reply = g_variant_new ("(^as)", saver->inhibited ? list : &list[1]);
    }
  g_mutex_unlock (&saver->lock);
  g_dbus_method_invocation_return_value (invocation, reply);
}

static const GDBusInterfaceVTable saver_vtable = { method_call, NULL, NULL, { 0 } };

static gpointer
serve (gpointer data)
{
  Saver *saver = data;

  g_main_context_push_thread_default (saver->context);
  g_main_loop_run (saver->loop);
  g_main_context_pop_thread_default (saver->context);
  return NULL;
}

static void
start_bus (Saver *saver)
{
  saver->bus = g_test_dbus_new (G_TEST_DBUS_NONE);
  g_test_dbus_up (saver->bus);
}

static void
stop_bus (Saver *saver)
{
  g_test_dbus_down (saver->bus);
  g_object_unref (saver->bus);
}

static void
start_saver (Saver *saver)
{
  g_autoptr (GDBusNodeInfo) node = g_dbus_node_info_new_for_xml (saver_xml, NULL);
  g_autoptr (GVariant) owned = NULL;

  start_bus (saver);
  saver->calls = g_ptr_array_new_with_free_func (g_free);
  saver->context = g_main_context_new ();
  saver->loop = g_main_loop_new (saver->context, FALSE);
  /* The methods are dispatched in the context that's the default as they're registered. */
  g_main_context_push_thread_default (saver->context);
  saver->connection = g_dbus_connection_new_for_address_sync (
    g_test_dbus_get_bus_address (saver->bus),
    G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT | G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
    NULL, NULL, NULL);
  g_assert_nonnull (saver->connection);
  g_assert_cmpuint (g_dbus_connection_register_object (saver->connection, PATH, node->interfaces[0],
                                                       &saver_vtable, saver, NULL, NULL), >, 0);
  g_main_context_pop_thread_default (saver->context);
  owned = g_dbus_connection_call_sync (saver->connection, "org.freedesktop.DBus",
                                       "/org/freedesktop/DBus", "org.freedesktop.DBus",
                                       "RequestName", g_variant_new ("(su)", NAME, 0),
                                       NULL, G_DBUS_CALL_FLAGS_NONE, -1, NULL, NULL);
  g_assert_nonnull (owned);
  saver->thread = g_thread_new ("saver", serve, saver);
}

static void
stop_saver (Saver *saver)
{
  g_main_loop_quit (saver->loop);
  g_thread_join (saver->thread);
  g_dbus_connection_close_sync (saver->connection, NULL, NULL);
  g_object_unref (saver->connection);
  g_main_loop_unref (saver->loop);
  g_main_context_unref (saver->context);
  g_ptr_array_unref (saver->calls);
  stop_bus (saver);
}

static gboolean
called (Saver *saver, const char *call)
{
  gboolean found;

  g_mutex_lock (&saver->lock);
  found = g_ptr_array_find_with_equal_func (saver->calls, call, g_str_equal, NULL);
  g_mutex_unlock (&saver->lock);
  return found;
}

/* Runs the program with `option`, and returns what it printed. */
static char *
run (const char *option, gboolean expect_success)
{
  const char *argv[] = { program, option, NULL };
  char *out = NULL;
  int status = 0;
  g_autoptr (GError) error = NULL;

  g_assert_true (g_spawn_sync (NULL, (char **) argv, NULL, G_SPAWN_DEFAULT, NULL, NULL,
                               &out, NULL, &status, &error));
  g_assert_no_error (error);
  g_assert_cmpint (g_spawn_check_wait_status (status, NULL), ==, expect_success);
  return out;
}

static void
test_lock_asks_for_the_lock (void)
{
  Saver saver = { 0 };
  g_autofree char *out = NULL;

  start_saver (&saver);
  out = run ("--lock", TRUE);
  g_assert_true (called (&saver, "Lock ()"));
  stop_saver (&saver);
}

static void
test_query_says_the_state_and_the_inhibitors (void)
{
  Saver saver = { 0 };
  g_autofree char *inactive = NULL;
  g_autofree char *inhibited = NULL;

  start_saver (&saver);
  inactive = run ("--query", TRUE);
  g_assert_cmpstr (inactive, ==, "The screensaver is inactive\nThe screensaver is not inhibited\n");

  saver.active = TRUE;
  saver.inhibited = TRUE;
  inhibited = run ("--query", TRUE);
  g_assert_cmpstr (inhibited, ==,
                   "The screensaver is active\n"
                   "The screensaver is being inhibited by:\n"
                   "\tpluma: a reason\n");
  stop_saver (&saver);
}

static void
test_time_says_how_long_it_has_been_active (void)
{
  Saver saver = { 0 };
  g_autofree char *inactive = NULL;
  g_autofree char *active = NULL;

  start_saver (&saver);
  inactive = run ("--time", TRUE);
  g_assert_cmpstr (inactive, ==, "The screensaver is not currently active.\n");
  saver.active = TRUE;
  active = run ("--time", TRUE);
  g_assert_cmpstr (active, ==, "The screensaver has been active for 42 seconds.\n");
  stop_saver (&saver);
}

static void
test_activate_and_deactivate_set_the_blank (void)
{
  Saver saver = { 0 };
  g_autofree char *on = NULL;
  g_autofree char *off = NULL;

  start_saver (&saver);
  on = run ("--activate", TRUE);
  g_assert_true (called (&saver, "SetActive (true,)"));
  off = run ("--deactivate", TRUE);
  g_assert_true (called (&saver, "SetActive (false,)"));
  stop_saver (&saver);
}

/* It holds its inhibitor until it's ended, as the bus then ends the hold. */
static void
test_inhibit_blocks_while_it_holds (void)
{
  Saver saver = { 0 };
  const char *argv[] = { program, "--inhibit", "-n", "pluma", "-r", "a reason", NULL };
  GPid pid;
  int status;
  g_autoptr (GError) error = NULL;

  start_saver (&saver);
  g_assert_true (g_spawn_async (NULL, (char **) argv, NULL, G_SPAWN_DO_NOT_REAP_CHILD, NULL, NULL,
                                &pid, &error));
  g_assert_no_error (error);
  for (int i = 0; i < 500 && !called (&saver, "Inhibit ('pluma', 'a reason')"); i++)
    g_usleep (10000);
  g_assert_true (called (&saver, "Inhibit ('pluma', 'a reason')"));
  g_usleep (100000);
  g_assert_cmpint (waitpid (pid, &status, WNOHANG), ==, 0);
  kill (pid, SIGTERM);
  g_assert_cmpint (waitpid (pid, &status, 0), ==, pid);
  stop_saver (&saver);
}

/* It never starts a screensaver: the session's is Caminito's. */
static void
test_without_a_screensaver_it_fails (void)
{
  Saver saver = { 0 };
  g_autofree char *out = NULL;

  start_bus (&saver);
  out = run ("--lock", FALSE);
  stop_bus (&saver);
}

int
main (int argc, char **argv)
{
  setlocale (LC_ALL, "C");
  g_test_init (&argc, &argv, NULL);
  g_assert_cmpint (argc, ==, 2);
  program = argv[1];

  g_test_add_func ("/command/lock-asks-for-the-lock", test_lock_asks_for_the_lock);
  g_test_add_func ("/command/query-says-the-state-and-the-inhibitors",
                   test_query_says_the_state_and_the_inhibitors);
  g_test_add_func ("/command/time-says-how-long-it-has-been-active",
                   test_time_says_how_long_it_has_been_active);
  g_test_add_func ("/command/activate-and-deactivate-set-the-blank",
                   test_activate_and_deactivate_set_the_blank);
  g_test_add_func ("/command/inhibit-blocks-while-it-holds", test_inhibit_blocks_while_it_holds);
  g_test_add_func ("/command/without-a-screensaver-it-fails", test_without_a_screensaver_it_fails);

  return g_test_run ();
}
