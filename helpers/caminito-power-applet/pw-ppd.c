/*
 * power-profiles-daemon as the Power popup reads it (264). See pw-ppd.h.
 *
 * Both bus names are watched, neither of them started: a computer with no daemon has no
 * profile section, and the applet doesn't start one. The proxy is made for the first name
 * that has an owner, again whenever that changes, so that a daemon that stops, starts or
 * moves from one name to the other is one more change.
 */

#include "pw-ppd.h"

#define PROPERTIES_INTERFACE "org.freedesktop.DBus.Properties"

typedef struct
{
  const char *name;
  const char *path;
  /* The interface is named as the bus name. */
} Daemon;

/* In the order of preference. */
static const Daemon daemons[] = {
  { "org.freedesktop.UPower.PowerProfiles", "/org/freedesktop/UPower/PowerProfiles" },
  { "net.hadess.PowerProfiles", "/net/hadess/PowerProfiles" },
};
#define DAEMON_COUNT G_N_ELEMENTS (daemons)

struct _PwPpd
{
  GCancellable *cancellable;
  guint         watches[DAEMON_COUNT];
  gboolean      owned[DAEMON_COUNT];
  /* The one the proxy is for, or -1. */
  int           current;
  GDBusProxy   *proxy;
  PwPpdChanged  changed;
  gpointer      data;
  guint         idle;
};

static void connect_proxy (PwPpd *ppd);

static gboolean
emit_changed (gpointer data)
{
  PwPpd *ppd = data;

  ppd->idle = 0;
  ppd->changed (ppd, ppd->data);
  return G_SOURCE_REMOVE;
}

static void
queue_changed (PwPpd *ppd)
{
  if (ppd->idle == 0)
    ppd->idle = g_idle_add (emit_changed, ppd);
}

static void
on_properties_changed (GDBusProxy *proxy G_GNUC_UNUSED,
                       GVariant   *changed G_GNUC_UNUSED,
                       GStrv       invalidated G_GNUC_UNUSED,
                       gpointer    data)
{
  queue_changed (data);
}

static void
on_proxy (GObject *source G_GNUC_UNUSED, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  GDBusProxy *proxy = g_dbus_proxy_new_finish (result, &error);
  PwPpd *ppd;

  if (proxy == NULL)
    {
      /* Cancelled means the applet is gone, and the daemon with it. */
      if (!g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
        g_warning ("can't follow power-profiles-daemon: %s", error->message);
      return;
    }

  ppd = data;
  g_clear_object (&ppd->proxy);
  /* The daemon went again while the proxy was made: the watch has run, and the next
     appearance makes another. */
  if (g_dbus_proxy_get_name_owner (proxy) == NULL)
    {
      g_object_unref (proxy);
      return;
    }
  ppd->proxy = proxy;
  g_signal_connect (proxy, "g-properties-changed", G_CALLBACK (on_properties_changed), ppd);
  queue_changed (ppd);
}

/* The proxy for the first name that has an owner. */
static void
connect_proxy (PwPpd *ppd)
{
  int wanted = -1;

  for (guint i = 0; i < DAEMON_COUNT && wanted < 0; i++)
    if (ppd->owned[i])
      wanted = i;
  if (wanted == ppd->current)
    return;
  ppd->current = wanted;
  /* A proxy that's still being made for the old name is dropped by on_proxy's check on the
     owner, or replaces this one's successor: cancel it so that only the last stands. */
  g_cancellable_cancel (ppd->cancellable);
  g_object_unref (ppd->cancellable);
  ppd->cancellable = g_cancellable_new ();
  g_clear_object (&ppd->proxy);
  if (wanted >= 0)
    g_dbus_proxy_new_for_bus (G_BUS_TYPE_SYSTEM, G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START, NULL,
                              daemons[wanted].name, daemons[wanted].path, daemons[wanted].name,
                              ppd->cancellable, on_proxy, ppd);
  queue_changed (ppd);
}

typedef struct
{
  PwPpd *ppd;
  guint  index;
} Watch;

static void
on_name_appeared (GDBusConnection *connection G_GNUC_UNUSED,
                  const char      *name G_GNUC_UNUSED,
                  const char      *owner G_GNUC_UNUSED,
                  gpointer         data)
{
  Watch *watch = data;

  watch->ppd->owned[watch->index] = TRUE;
  /* A daemon that restarts under the same name is a new owner of the current proxy's name,
     which the proxy follows by itself; one that appears for a name the proxy isn't for
     changes it. */
  connect_proxy (watch->ppd);
  queue_changed (watch->ppd);
}

static void
on_name_vanished (GDBusConnection *connection G_GNUC_UNUSED,
                  const char      *name G_GNUC_UNUSED,
                  gpointer         data)
{
  Watch *watch = data;

  watch->ppd->owned[watch->index] = FALSE;
  connect_proxy (watch->ppd);
  queue_changed (watch->ppd);
}

PwPpd *
pw_ppd_new (PwPpdChanged changed, gpointer data)
{
  PwPpd *ppd = g_new0 (PwPpd, 1);

  ppd->cancellable = g_cancellable_new ();
  ppd->current = -1;
  ppd->changed = changed;
  ppd->data = data;
  for (guint i = 0; i < DAEMON_COUNT; i++)
    {
      Watch *watch = g_new0 (Watch, 1);

      watch->ppd = ppd;
      watch->index = i;
      ppd->watches[i] = g_bus_watch_name (G_BUS_TYPE_SYSTEM, daemons[i].name,
                                          G_BUS_NAME_WATCHER_FLAGS_NONE, on_name_appeared,
                                          on_name_vanished, watch, g_free);
    }
  return ppd;
}

void
pw_ppd_free (PwPpd *ppd)
{
  g_cancellable_cancel (ppd->cancellable);
  for (guint i = 0; i < DAEMON_COUNT; i++)
    g_bus_unwatch_name (ppd->watches[i]);
  g_clear_handle_id (&ppd->idle, g_source_remove);
  g_clear_object (&ppd->proxy);
  g_object_unref (ppd->cancellable);
  g_free (ppd);
}

PwProfiles *
pw_ppd_profiles (PwPpd *ppd)
{
  g_autoptr (GVariant) listed = NULL;
  g_autoptr (GVariant) active = NULL;
  g_autoptr (GVariant) degraded = NULL;
  g_autoptr (GVariant) holds = NULL;

  if (ppd->proxy == NULL)
    return NULL;
  listed = g_dbus_proxy_get_cached_property (ppd->proxy, "Profiles");
  active = g_dbus_proxy_get_cached_property (ppd->proxy, "ActiveProfile");
  degraded = g_dbus_proxy_get_cached_property (ppd->proxy, "PerformanceDegraded");
  holds = g_dbus_proxy_get_cached_property (ppd->proxy, "ActiveProfileHolds");
  return pw_profiles_new (listed, active, degraded, holds);
}

static void
on_set (GObject *source, GAsyncResult *result, gpointer data G_GNUC_UNUSED)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = g_dbus_connection_call_finish (G_DBUS_CONNECTION (source), result,
                                                              &error);

  if (reply == NULL)
    g_warning ("can't set the power profile: %s", error->message);
}

void
pw_ppd_set_active (PwPpd *ppd, PwProfile profile)
{
  if (ppd->proxy == NULL || ppd->current < 0)
    return;
  /* The proxy's own properties are read-only copies: the daemon's are set through the
     properties interface, which the daemon answers with a change. */
  g_dbus_connection_call (g_dbus_proxy_get_connection (ppd->proxy),
                          daemons[ppd->current].name, daemons[ppd->current].path,
                          PROPERTIES_INTERFACE, "Set",
                          g_variant_new ("(ssv)", daemons[ppd->current].name, "ActiveProfile",
                                         g_variant_new_string (pw_profile_id (profile))),
                          NULL, G_DBUS_CALL_FLAGS_NONE, -1, NULL, on_set, NULL);
}
