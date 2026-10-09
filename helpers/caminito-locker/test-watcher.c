/*
 * The session's status message, as gs-watcher.c (132) reads it from mate-session's
 * presence, against a stand-in presence on a bus of the test's own.
 *
 * The stand-in owns org.gnome.SessionManager and serves /org/gnome/SessionManager/Presence
 * with what mate-session has there for the status message: the status-text property and
 * the StatusTextChanged signal.
 */

#include <gio/gio.h>

#include "gs-watcher.h"

#define PRESENCE_NAME      "org.gnome.SessionManager"
#define PRESENCE_PATH      "/org/gnome/SessionManager/Presence"
#define PRESENCE_INTERFACE "org.gnome.SessionManager.Presence"

static const char presence_xml[] =
  "<node>"
  "  <interface name='" PRESENCE_INTERFACE "'>"
  "    <property name='status' type='u' access='readwrite'/>"
  "    <property name='status-text' type='s' access='readwrite'/>"
  "    <signal name='StatusChanged'><arg name='status' type='u'/></signal>"
  "    <signal name='StatusTextChanged'><arg name='status_text' type='s'/></signal>"
  "  </interface>"
  "</node>";

typedef struct
{
  GTestDBus       *bus;
  /* The stand-in's connection, served by its thread, as the watcher's first read blocks
     the test's. */
  GDBusConnection *connection;
  GMainContext    *context;
  GMainLoop       *loop;
  GThread         *thread;
  GMutex           lock;
  char            *status_text;
} Presence;

static GVariant *
get_property (GDBusConnection *connection G_GNUC_UNUSED,
              const char      *sender G_GNUC_UNUSED,
              const char      *path G_GNUC_UNUSED,
              const char      *interface G_GNUC_UNUSED,
              const char      *property,
              GError         **error G_GNUC_UNUSED,
              gpointer         data)
{
  Presence *presence = data;
  GVariant *value;

  if (g_strcmp0 (property, "status") == 0)
    return g_variant_new_uint32 (0);
  g_mutex_lock (&presence->lock);
  value = g_variant_new_string (presence->status_text);
  g_mutex_unlock (&presence->lock);
  return value;
}

static const GDBusInterfaceVTable presence_vtable = { NULL, get_property, NULL, { 0 } };

static void
start_bus (Presence *presence)
{
  presence->bus = g_test_dbus_new (G_TEST_DBUS_NONE);
  g_test_dbus_up (presence->bus);
}

static gpointer
serve_presence (gpointer data)
{
  Presence *presence = data;

  g_main_context_push_thread_default (presence->context);
  g_main_loop_run (presence->loop);
  g_main_context_pop_thread_default (presence->context);
  return NULL;
}

static void
own_presence (Presence *presence, const char *status_text)
{
  GDBusNodeInfo *info = g_dbus_node_info_new_for_xml (presence_xml, NULL);
  GError *error = NULL;
  GVariant *reply;

  g_mutex_init (&presence->lock);
  presence->status_text = g_strdup (status_text);
  presence->context = g_main_context_new ();
  presence->loop = g_main_loop_new (presence->context, FALSE);

  /* The connection and its object belong to the context they're made in. */
  g_main_context_push_thread_default (presence->context);
  presence->connection =
    g_dbus_connection_new_for_address_sync (g_test_dbus_get_bus_address (presence->bus),
                                            G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT
                                            | G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
                                            NULL,
                                            NULL,
                                            &error);
  g_assert_no_error (error);
  g_dbus_connection_register_object (presence->connection,
                                     PRESENCE_PATH,
                                     info->interfaces[0],
                                     &presence_vtable,
                                     presence,
                                     NULL,
                                     &error);
  g_assert_no_error (error);
  g_main_context_pop_thread_default (presence->context);
  g_dbus_node_info_unref (info);

  reply = g_dbus_connection_call_sync (presence->connection,
                                       "org.freedesktop.DBus",
                                       "/org/freedesktop/DBus",
                                       "org.freedesktop.DBus",
                                       "RequestName",
                                       g_variant_new ("(su)", PRESENCE_NAME, 0),
                                       G_VARIANT_TYPE ("(u)"),
                                       G_DBUS_CALL_FLAGS_NONE,
                                       -1,
                                       NULL,
                                       &error);
  g_assert_no_error (error);
  g_variant_unref (reply);

  presence->thread = g_thread_new ("presence", serve_presence, presence);
}

/* What mate-session does when the user sets a new status message. */
static void
change_status_text (Presence *presence, const char *status_text)
{
  GError *error = NULL;

  g_mutex_lock (&presence->lock);
  g_free (presence->status_text);
  presence->status_text = g_strdup (status_text);
  g_mutex_unlock (&presence->lock);
  g_dbus_connection_emit_signal (presence->connection,
                                 NULL,
                                 PRESENCE_PATH,
                                 PRESENCE_INTERFACE,
                                 "StatusTextChanged",
                                 g_variant_new ("(s)", status_text),
                                 &error);
  g_assert_no_error (error);
}

static void
stop_bus (Presence *presence)
{
  if (presence->thread != NULL)
    {
      g_main_loop_quit (presence->loop);
      g_thread_join (presence->thread);
      g_dbus_connection_close_sync (presence->connection, NULL, NULL);
      g_clear_object (&presence->connection);
      g_main_loop_unref (presence->loop);
      g_main_context_unref (presence->context);
      g_mutex_clear (&presence->lock);
    }
  g_test_dbus_down (presence->bus);
  g_clear_object (&presence->bus);
  g_free (presence->status_text);
}

static char *
status_message (GSWatcher *watcher)
{
  char *text;

  g_object_get (watcher, "status-message", &text, NULL);
  return text;
}

static void
wait_for_status_message (GSWatcher *watcher, const char *expected)
{
  gint64 deadline = g_get_monotonic_time () + 5 * G_USEC_PER_SEC;

  for (;;)
    {
      char *text = status_message (watcher);
      gboolean seen = g_strcmp0 (text, expected) == 0;

      g_free (text);
      if (seen)
        return;
      g_assert_cmpint (g_get_monotonic_time (), <, deadline);
      g_main_context_iteration (NULL, TRUE);
    }
}

static void
test_status_text_at_start (void)
{
  Presence presence = { 0 };
  GSWatcher *watcher;

  start_bus (&presence);
  own_presence (&presence, "Back at 3");

  watcher = gs_watcher_new ();
  wait_for_status_message (watcher, "Back at 3");

  g_object_unref (watcher);
  stop_bus (&presence);
}

static void
notified (GObject *object G_GNUC_UNUSED, GParamSpec *pspec G_GNUC_UNUSED, gpointer data)
{
  int *count = data;

  (*count)++;
}

static void
test_status_text_changed (void)
{
  Presence presence = { 0 };
  GSWatcher *watcher;
  int notifications = 0;

  start_bus (&presence);
  own_presence (&presence, "");

  watcher = gs_watcher_new ();
  g_signal_connect (watcher, "notify::status-message", G_CALLBACK (notified), &notifications);
  change_status_text (&presence, "In a meeting");
  wait_for_status_message (watcher, "In a meeting");
  g_assert_cmpint (notifications, ==, 1);

  g_object_unref (watcher);
  stop_bus (&presence);
}

/* Without mate-session, as in a nested run, there's no message, as in mate-screensaver. */
static void
test_no_presence (void)
{
  Presence presence = { 0 };
  GSWatcher *watcher;
  char *text;

  start_bus (&presence);

  watcher = gs_watcher_new ();
  while (g_main_context_iteration (NULL, FALSE))
    ;
  text = status_message (watcher);
  g_assert_null (text);

  g_object_unref (watcher);
  stop_bus (&presence);
}

/* mate-session started after the locker, or again: its status text is read then. */
static void
test_presence_that_comes_later (void)
{
  Presence presence = { 0 };
  GSWatcher *watcher;

  start_bus (&presence);

  watcher = gs_watcher_new ();
  own_presence (&presence, "Back soon");
  wait_for_status_message (watcher, "Back soon");

  g_object_unref (watcher);
  stop_bus (&presence);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);

  g_test_add_func ("/watcher/status-text-at-start", test_status_text_at_start);
  g_test_add_func ("/watcher/status-text-changed", test_status_text_changed);
  g_test_add_func ("/watcher/no-presence", test_no_presence);
  g_test_add_func ("/watcher/presence-that-comes-later", test_presence_that_comes_later);

  return g_test_run ();
}
