/*
 * mate-session as the Power applet uses it (265). See pw-session.h.
 *
 * Stay awake is Inhibit with the suspend and idle flags, whose cookie the applet keeps,
 * and Uninhibit with it. mate-session drops an inhibitor when its owner leaves the bus, so
 * a panel that restarts has Stay awake off again, with nothing saved.
 *
 * The list is read again, whole, after each InhibitorAdded and InhibitorRemoved: an
 * inhibitor's path is all the signal has, and a few calls are cheaper than keeping a copy
 * that could drift from mate-session's.
 */

#include "pw-session.h"

#define SESSION_NAME        "org.gnome.SessionManager"
#define SESSION_PATH        "/org/gnome/SessionManager"
#define SESSION_INTERFACE   "org.gnome.SessionManager"
#define INHIBITOR_INTERFACE "org.gnome.SessionManager.Inhibitor"

struct _PwSession
{
  GCancellable     *cancellable;
  guint             watch;
  /* Set while mate-session has its name. */
  GDBusConnection  *connection;
  guint             added;
  guint             removed;
  /* Stay awake: what the user asked for, the cookie of the inhibitor mate-session gave
     (0 for none), and whether the Inhibit call is still out. */
  gboolean          wanted;
  guint32           cookie;
  gboolean          inhibiting;
  GPtrArray        *inhibitors;
  /* A read of the list is current until the next one starts. */
  guint             generation;
  guint             refresh;
  PwSessionChanged  changed;
  gpointer          data;
  guint             idle;
};

static gboolean
emit_changed (gpointer data)
{
  PwSession *session = data;

  session->idle = 0;
  session->changed (session, session->data);
  return G_SOURCE_REMOVE;
}

static void
queue_changed (PwSession *session)
{
  if (session->idle == 0)
    session->idle = g_idle_add (emit_changed, session);
}

static void
on_uninhibited (GObject *source, GAsyncResult *result, gpointer data G_GNUC_UNUSED)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = g_dbus_connection_call_finish (G_DBUS_CONNECTION (source), result,
                                                              &error);

  /* mate-session gone with the call out took the inhibitor with it. */
  if (reply == NULL && !g_error_matches (error, G_DBUS_ERROR, G_DBUS_ERROR_SERVICE_UNKNOWN)
      && !g_error_matches (error, G_DBUS_ERROR, G_DBUS_ERROR_NAME_HAS_NO_OWNER)
      && !g_error_matches (error, G_DBUS_ERROR, G_DBUS_ERROR_NO_REPLY)
      && !g_error_matches (error, G_DBUS_ERROR, G_DBUS_ERROR_DISCONNECTED)
      && !g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CLOSED))
    g_warning ("can't turn Stay awake off: %s", error->message);
}

static void
uninhibit (GDBusConnection *connection, guint32 cookie)
{
  /* No cancellable: the call goes out even as the applet is freed. */
  g_dbus_connection_call (connection, SESSION_NAME, SESSION_PATH, SESSION_INTERFACE,
                          "Uninhibit", g_variant_new ("(u)", cookie), NULL,
                          G_DBUS_CALL_FLAGS_NONE, -1, NULL, on_uninhibited, NULL);
}

/* The Inhibit call's data. The applet can be freed with the call out, so the session is
   only touched while the cancellable says that it isn't. The call itself isn't cancelled:
   mate-session would still make the inhibitor, and its cookie would be lost. */
typedef struct
{
  PwSession    *session;
  GCancellable *cancellable;
} Inhibiting;

static void
on_inhibited (GObject *source, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = g_dbus_connection_call_finish (G_DBUS_CONNECTION (source), result,
                                                              &error);
  Inhibiting *inhibiting = data;
  PwSession *session = inhibiting->session;
  gboolean gone = g_cancellable_is_cancelled (inhibiting->cancellable);
  guint32 cookie = 0;

  g_object_unref (inhibiting->cancellable);
  g_free (inhibiting);
  if (reply != NULL)
    g_variant_get (reply, "(u)", &cookie);
  if (gone)
    {
      /* The applet went with the call out: nothing may keep the computer awake. */
      if (cookie != 0)
        uninhibit (G_DBUS_CONNECTION (source), cookie);
      return;
    }
  session->inhibiting = FALSE;
  if (reply == NULL)
    {
      g_warning ("can't turn Stay awake on: %s", error->message);
      session->wanted = FALSE;
      queue_changed (session);
      return;
    }
  if (!session->wanted)
    {
      /* Turned off while the call was out. */
      uninhibit (G_DBUS_CONNECTION (source), cookie);
      return;
    }
  session->cookie = cookie;
}

void
pw_session_set_stay_awake (PwSession *session, gboolean on)
{
  if (on == session->wanted)
    return;
  if (on)
    {
      if (session->connection == NULL)
        {
          g_warning ("can't turn Stay awake on: mate-session isn't running");
          queue_changed (session);
          return;
        }
      session->wanted = TRUE;
      /* A call still out will bring the cookie. */
      if (!session->inhibiting && session->cookie == 0)
        {
          Inhibiting *inhibiting = g_new0 (Inhibiting, 1);

          inhibiting->session = session;
          inhibiting->cancellable = g_object_ref (session->cancellable);
          session->inhibiting = TRUE;
          g_dbus_connection_call (session->connection, SESSION_NAME, SESSION_PATH,
                                  SESSION_INTERFACE, "Inhibit",
                                  g_variant_new ("(susu)", PW_STAY_AWAKE_APP_ID, 0u,
                                                 PW_STAY_AWAKE_REASON,
                                                 (guint) (PW_INHIBIT_SUSPEND | PW_INHIBIT_IDLE)),
                                  G_VARIANT_TYPE ("(u)"), G_DBUS_CALL_FLAGS_NONE, -1,
                                  NULL, on_inhibited, inhibiting);
        }
      return;
    }
  session->wanted = FALSE;
  if (session->cookie != 0)
    {
      if (session->connection != NULL)
        uninhibit (session->connection, session->cookie);
      session->cookie = 0;
    }
}

gboolean
pw_session_stay_awake (PwSession *session)
{
  return session->wanted;
}

const GPtrArray *
pw_session_inhibitors (PwSession *session)
{
  return session->inhibitors;
}

/* One read of the list: the inhibitors' paths, then each one's app ID, reason and flags.
   It is dropped, with what it has read, if another read has started since, or the applet
   is gone. */
typedef struct
{
  PwSession    *session;
  GCancellable *cancellable;
  guint         generation;
  /* The Item of each inhibitor, in mate-session's order. */
  GPtrArray    *items;
  /* Calls still out. */
  guint         pending;
} Read;

typedef struct
{
  char     *path;
  char     *app_id;
  char     *reason;
  guint32   flags;
  /* Calls answered, of 3: an inhibitor that went in the meantime answers fewer. */
  guint     answered;
} Item;

static void
item_free (gpointer data)
{
  Item *item = data;

  g_free (item->path);
  g_free (item->app_id);
  g_free (item->reason);
  g_free (item);
}

static void
read_free (Read *read)
{
  g_ptr_array_unref (read->items);
  g_object_unref (read->cancellable);
  g_free (read);
}

static gboolean
read_is_current (Read *read)
{
  return !g_cancellable_is_cancelled (read->cancellable)
         && read->generation == read->session->generation;
}

static void
read_finished (Read *read)
{
  PwSession *session = read->session;
  GPtrArray *listed = g_ptr_array_new_with_free_func ((GDestroyNotify) pw_inhibitor_free);

  for (guint i = 0; i < read->items->len; i++)
    {
      Item *item = g_ptr_array_index (read->items, i);

      if (item->answered == 3 && pw_state_inhibitor_listed (item->flags))
        g_ptr_array_add (listed, pw_inhibitor_new (item->app_id, item->reason, item->flags));
    }
  g_ptr_array_unref (session->inhibitors);
  session->inhibitors = listed;
  queue_changed (session);
}

typedef struct
{
  Read *read;
  Item *item;
  /* 0 GetAppId, 1 GetReason, 2 GetFlags. */
  int   which;
} Call;

static void
on_field (GObject *source, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = g_dbus_connection_call_finish (G_DBUS_CONNECTION (source), result,
                                                              &error);
  Call *call = data;
  Read *read = call->read;
  Item *item = call->item;
  gboolean cancelled = reply == NULL && g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED);

  if (reply != NULL && read_is_current (read))
    {
      if (call->which == 2)
        g_variant_get (reply, "(u)", &item->flags);
      else
        {
          char *text = NULL;

          g_variant_get (reply, "(s)", &text);
          if (call->which == 0)
            {
              g_free (item->app_id);
              item->app_id = text;
            }
          else
            {
              g_free (item->reason);
              item->reason = text;
            }
        }
      item->answered++;
    }
  g_free (call);
  /* No error is reported: an inhibitor that was removed between the calls answers with
     one, and the next read doesn't have it. */
  if (--read->pending == 0)
    {
      if (!cancelled && read_is_current (read))
        read_finished (read);
      read_free (read);
    }
}

static void
read_item (Read *read, GDBusConnection *connection, Item *item)
{
  static const char *const methods[] = { "GetAppId", "GetReason", "GetFlags" };

  for (int which = 0; which < 3; which++)
    {
      Call *call = g_new0 (Call, 1);

      call->read = read;
      call->item = item;
      call->which = which;
      read->pending++;
      g_dbus_connection_call (connection, SESSION_NAME, item->path, INHIBITOR_INTERFACE,
                              methods[which], NULL,
                              G_VARIANT_TYPE (which == 2 ? "(u)" : "(s)"),
                              G_DBUS_CALL_FLAGS_NONE, -1, read->cancellable, on_field, call);
    }
}

static void
on_inhibitors (GObject *source, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = g_dbus_connection_call_finish (G_DBUS_CONNECTION (source), result,
                                                              &error);
  Read *read = data;
  g_autoptr (GVariantIter) paths = NULL;
  const char *path;

  if (reply == NULL)
    {
      /* Cancelled, or mate-session went: its vanishing empties the list. */
      if (!g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
        g_warning ("can't list what keeps the computer awake: %s", error->message);
      read_free (read);
      return;
    }
  if (!read_is_current (read))
    {
      read_free (read);
      return;
    }
  g_variant_get (reply, "(ao)", &paths);
  /* Held until every path has its calls out, so that a read that is done at once isn't
     finished early. */
  read->pending = 1;
  while (g_variant_iter_next (paths, "&o", &path))
    {
      Item *item = g_new0 (Item, 1);

      item->path = g_strdup (path);
      g_ptr_array_add (read->items, item);
      read_item (read, G_DBUS_CONNECTION (source), item);
    }
  if (--read->pending == 0)
    {
      read_finished (read);
      read_free (read);
    }
}

static void
read_inhibitors (PwSession *session)
{
  Read *read = g_new0 (Read, 1);

  read->session = session;
  read->cancellable = g_object_ref (session->cancellable);
  read->generation = ++session->generation;
  read->items = g_ptr_array_new_with_free_func (item_free);
  g_dbus_connection_call (session->connection, SESSION_NAME, SESSION_PATH, SESSION_INTERFACE,
                          "GetInhibitors", NULL, G_VARIANT_TYPE ("(ao)"),
                          G_DBUS_CALL_FLAGS_NONE, -1, session->cancellable, on_inhibitors, read);
}

static gboolean
on_refresh (gpointer data)
{
  PwSession *session = data;

  session->refresh = 0;
  if (session->connection != NULL)
    read_inhibitors (session);
  return G_SOURCE_REMOVE;
}

static void
queue_refresh (PwSession *session)
{
  if (session->refresh == 0)
    session->refresh = g_idle_add (on_refresh, session);
}

static void
on_inhibitor_signal (GDBusConnection *connection G_GNUC_UNUSED,
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
unsubscribe (PwSession *session)
{
  if (session->connection == NULL)
    return;
  g_dbus_connection_signal_unsubscribe (session->connection, session->added);
  g_dbus_connection_signal_unsubscribe (session->connection, session->removed);
  session->added = session->removed = 0;
  g_clear_object (&session->connection);
}

static void
on_name_appeared (GDBusConnection *connection,
                  const char      *name G_GNUC_UNUSED,
                  const char      *owner G_GNUC_UNUSED,
                  gpointer         data)
{
  PwSession *session = data;

  unsubscribe (session);
  session->connection = g_object_ref (connection);
  session->added = g_dbus_connection_signal_subscribe (
    connection, SESSION_NAME, SESSION_INTERFACE, "InhibitorAdded", SESSION_PATH, NULL,
    G_DBUS_SIGNAL_FLAGS_NONE, on_inhibitor_signal, session, NULL);
  session->removed = g_dbus_connection_signal_subscribe (
    connection, SESSION_NAME, SESSION_INTERFACE, "InhibitorRemoved", SESSION_PATH, NULL,
    G_DBUS_SIGNAL_FLAGS_NONE, on_inhibitor_signal, session, NULL);
  queue_refresh (session);
}

static void
on_name_vanished (GDBusConnection *connection G_GNUC_UNUSED,
                  const char      *name G_GNUC_UNUSED,
                  gpointer         data)
{
  PwSession *session = data;

  unsubscribe (session);
  /* A read under way is dropped, and so is the inhibitor: mate-session's went with it. */
  session->generation++;
  g_clear_handle_id (&session->refresh, g_source_remove);
  g_ptr_array_set_size (session->inhibitors, 0);
  session->wanted = FALSE;
  session->cookie = 0;
  session->inhibiting = FALSE;
  queue_changed (session);
}

PwSession *
pw_session_new (PwSessionChanged changed, gpointer data)
{
  PwSession *session = g_new0 (PwSession, 1);

  session->cancellable = g_cancellable_new ();
  session->inhibitors = g_ptr_array_new_with_free_func ((GDestroyNotify) pw_inhibitor_free);
  session->changed = changed;
  session->data = data;
  session->watch = g_bus_watch_name (G_BUS_TYPE_SESSION, SESSION_NAME,
                                     G_BUS_NAME_WATCHER_FLAGS_NONE, on_name_appeared,
                                     on_name_vanished, session, NULL);
  return session;
}

void
pw_session_free (PwSession *session)
{
  /* A panel that drops the applet keeps running: Stay awake can't outlive it. */
  if (session->cookie != 0 && session->connection != NULL)
    uninhibit (session->connection, session->cookie);
  g_cancellable_cancel (session->cancellable);
  g_bus_unwatch_name (session->watch);
  unsubscribe (session);
  g_clear_handle_id (&session->refresh, g_source_remove);
  g_clear_handle_id (&session->idle, g_source_remove);
  g_ptr_array_unref (session->inhibitors);
  g_object_unref (session->cancellable);
  g_free (session);
}
