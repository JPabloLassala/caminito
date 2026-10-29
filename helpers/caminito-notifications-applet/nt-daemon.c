/*
 * caminito-notifications as the Notifications applet uses it (CMN-260). See nt-daemon.h.
 *
 * The history is listed again, whole, after each HistoryChanged: the signal carries
 * nothing, and the daemon keeps at most 200 entries, so a List is cheap and can't drift from
 * the daemon's own.
 */

#include "nt-daemon.h"

#define DAEMON_NAME      "org.mate.Caminito.Notifications"
#define DAEMON_PATH      "/org/mate/Caminito/Notifications"
#define DAEMON_INTERFACE "org.mate.Caminito.Notifications"

struct _NtDaemon
{
  GCancellable    *cancellable;
  guint            watch;
  /* Set while the daemon has its name. */
  GDBusConnection *connection;
  guint            history_changed;
  GPtrArray       *entries;
  /* A List is current until the next one starts, or the daemon goes. */
  guint            generation;
  guint            refresh;
  NtDaemonChanged  changed;
  gpointer         data;
  guint            idle;
};

static gboolean
emit_changed (gpointer data)
{
  NtDaemon *daemon = data;

  daemon->idle = 0;
  daemon->changed (daemon, daemon->data);
  return G_SOURCE_REMOVE;
}

static void
queue_changed (NtDaemon *daemon)
{
  if (daemon->idle == 0)
    daemon->idle = g_idle_add (emit_changed, daemon);
}

/* Whether error says that the daemon is gone, or went while the call was out: the name
   watch tells the applet. */
static gboolean
daemon_is_gone (const GError *error)
{
  return g_error_matches (error, G_DBUS_ERROR, G_DBUS_ERROR_SERVICE_UNKNOWN)
         || g_error_matches (error, G_DBUS_ERROR, G_DBUS_ERROR_NAME_HAS_NO_OWNER)
         || g_error_matches (error, G_DBUS_ERROR, G_DBUS_ERROR_NO_REPLY)
         || g_error_matches (error, G_DBUS_ERROR, G_DBUS_ERROR_DISCONNECTED)
         || g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CLOSED);
}

/* A List's data. The applet can be freed with the call out, so the daemon is only touched
   while the cancellable says that it isn't. */
typedef struct
{
  NtDaemon *daemon;
  guint     generation;
} Listing;

static void
on_listed (GObject *source, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = g_dbus_connection_call_finish (G_DBUS_CONNECTION (source), result,
                                                              &error);
  Listing *listing = data;
  NtDaemon *daemon = listing->daemon;
  guint generation = listing->generation;
  GPtrArray *entries;

  g_free (listing);
  if (reply == NULL)
    {
      if (!g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED) && !daemon_is_gone (error))
        g_warning ("can't list the notification history: %s", error->message);
      return;
    }
  if (generation != daemon->generation)
    return;
  /* The reply's type was checked by GDBus. */
  entries = nt_entries_from_list (reply);
  g_ptr_array_unref (daemon->entries);
  daemon->entries = entries;
  queue_changed (daemon);
}

static void
list (NtDaemon *daemon)
{
  Listing *listing = g_new0 (Listing, 1);

  listing->daemon = daemon;
  listing->generation = ++daemon->generation;
  g_dbus_connection_call (daemon->connection, DAEMON_NAME, DAEMON_PATH, DAEMON_INTERFACE, "List",
                          NULL, G_VARIANT_TYPE (NT_LIST_TYPE), G_DBUS_CALL_FLAGS_NONE, -1,
                          daemon->cancellable, on_listed, listing);
}

static gboolean
on_refresh (gpointer data)
{
  NtDaemon *daemon = data;

  daemon->refresh = 0;
  if (daemon->connection != NULL)
    list (daemon);
  return G_SOURCE_REMOVE;
}

static void
queue_refresh (NtDaemon *daemon)
{
  if (daemon->refresh == 0)
    daemon->refresh = g_idle_add (on_refresh, daemon);
}

static void
on_history_changed (GDBusConnection *connection G_GNUC_UNUSED,
                    const char      *sender G_GNUC_UNUSED,
                    const char      *path G_GNUC_UNUSED,
                    const char      *interface G_GNUC_UNUSED,
                    const char      *signal G_GNUC_UNUSED,
                    GVariant        *parameters G_GNUC_UNUSED,
                    gpointer         data)
{
  queue_refresh (data);
}

static void
unsubscribe (NtDaemon *daemon)
{
  if (daemon->connection == NULL)
    return;
  g_dbus_connection_signal_unsubscribe (daemon->connection, daemon->history_changed);
  daemon->history_changed = 0;
  g_clear_object (&daemon->connection);
}

static void
on_name_appeared (GDBusConnection *connection,
                  const char      *name G_GNUC_UNUSED,
                  const char      *owner G_GNUC_UNUSED,
                  gpointer         data)
{
  NtDaemon *daemon = data;

  unsubscribe (daemon);
  daemon->connection = g_object_ref (connection);
  daemon->history_changed = g_dbus_connection_signal_subscribe (
    connection, DAEMON_NAME, DAEMON_INTERFACE, "HistoryChanged", DAEMON_PATH, NULL,
    G_DBUS_SIGNAL_FLAGS_NONE, on_history_changed, daemon, NULL);
  queue_refresh (daemon);
  /* Running, before the history is read. */
  queue_changed (daemon);
}

static void
on_name_vanished (GDBusConnection *connection G_GNUC_UNUSED,
                  const char      *name G_GNUC_UNUSED,
                  gpointer         data)
{
  NtDaemon *daemon = data;

  unsubscribe (daemon);
  /* A List under way is dropped. */
  daemon->generation++;
  g_clear_handle_id (&daemon->refresh, g_source_remove);
  g_ptr_array_set_size (daemon->entries, 0);
  queue_changed (daemon);
}

static void
on_called (GObject *source, GAsyncResult *result, gpointer data)
{
  g_autofree char *method = data;
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = g_dbus_connection_call_finish (G_DBUS_CONNECTION (source), result,
                                                              &error);

  /* An entry gone in the meantime, by a second click or its app, is refused: the next List
     hasn't it either. */
  if (reply == NULL && !daemon_is_gone (error)
      && !g_error_matches (error, G_DBUS_ERROR, G_DBUS_ERROR_INVALID_ARGS))
    g_warning ("can't call %s on the notification daemon: %s", method, error->message);
}

static void
call (NtDaemon *daemon, const char *method, GVariant *parameters)
{
  if (daemon->connection == NULL)
    {
      if (parameters != NULL)
        g_variant_unref (g_variant_ref_sink (parameters));
      return;
    }
  /* No cancellable: the call goes out even as the applet is freed. */
  g_dbus_connection_call (daemon->connection, DAEMON_NAME, DAEMON_PATH, DAEMON_INTERFACE, method,
                          parameters, NULL, G_DBUS_CALL_FLAGS_NONE, -1, NULL, on_called,
                          g_strdup (method));
}

void
nt_daemon_dismiss (NtDaemon *daemon, guint32 id)
{
  call (daemon, "Dismiss", g_variant_new ("(u)", id));
}

void
nt_daemon_clear_all (NtDaemon *daemon)
{
  call (daemon, "ClearAll", NULL);
}

void
nt_daemon_invoke_action (NtDaemon *daemon, guint32 id, const char *key)
{
  /* No activation token: bringing the app forward is CMN-266's. */
  call (daemon, "InvokeAction", g_variant_new ("(uss)", id, key, ""));
}

gboolean
nt_daemon_running (NtDaemon *daemon)
{
  return daemon->connection != NULL;
}

const GPtrArray *
nt_daemon_entries (NtDaemon *daemon)
{
  return daemon->entries;
}

NtDaemon *
nt_daemon_new (NtDaemonChanged changed, gpointer data)
{
  NtDaemon *daemon = g_new0 (NtDaemon, 1);

  daemon->cancellable = g_cancellable_new ();
  daemon->entries = g_ptr_array_new_with_free_func ((GDestroyNotify) nt_entry_free);
  daemon->changed = changed;
  daemon->data = data;
  daemon->watch = g_bus_watch_name (G_BUS_TYPE_SESSION, DAEMON_NAME,
                                    G_BUS_NAME_WATCHER_FLAGS_NONE, on_name_appeared,
                                    on_name_vanished, daemon, NULL);
  return daemon;
}

void
nt_daemon_free (NtDaemon *daemon)
{
  g_cancellable_cancel (daemon->cancellable);
  g_bus_unwatch_name (daemon->watch);
  unsubscribe (daemon);
  g_clear_handle_id (&daemon->refresh, g_source_remove);
  g_clear_handle_id (&daemon->idle, g_source_remove);
  g_ptr_array_unref (daemon->entries);
  g_object_unref (daemon->cancellable);
  g_free (daemon);
}
