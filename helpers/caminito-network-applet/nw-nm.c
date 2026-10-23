/*
 * NetworkManager as the Network applet reads it (CMN-193). See nw-nm.h.
 *
 * NMClient follows NetworkManager's name and its objects. The status is built again from it
 * on any change to the client, to an active connection, to the Wi-Fi device under the
 * connection or to its access point; those objects are watched again each time, since
 * which ones matter changes with the status.
 */

#include "nw-nm.h"

#include <NetworkManager.h>

struct _NwNm
{
  NwNmChanged   changed;
  gpointer      data;
  GCancellable *cancellable;
  NMClient     *client;
  NwStatus      status;
  /* The objects whose "notify" rebuilds the status, besides the client. */
  GPtrArray    *watched;
};

static gboolean
is_vpn (NMActiveConnection *active)
{
  /* WireGuard is a device of its own to NetworkManager, not a VPN, but a VPN to the user. */
  return nm_active_connection_get_vpn (active)
         || g_strcmp0 (nm_active_connection_get_connection_type (active),
                       NM_SETTING_WIREGUARD_SETTING_NAME) == 0;
}

static gboolean
is_wifi (NMActiveConnection *active)
{
  return g_strcmp0 (nm_active_connection_get_connection_type (active),
                    NM_SETTING_WIRELESS_SETTING_NAME) == 0;
}

/* Ethernet or Wi-Fi: what a user connects to, unlike a bridge, a tun or the loopback that
   NetworkManager also lists as active. */
static gboolean
is_physical (NMActiveConnection *active)
{
  return is_wifi (active)
         || g_strcmp0 (nm_active_connection_get_connection_type (active),
                       NM_SETTING_WIRED_SETTING_NAME) == 0;
}

/* The first active connection in state that matches, NULL with none. */
static NMActiveConnection *
find_active (NMClient *client, NMActiveConnectionState state,
             gboolean (*matches) (NMActiveConnection *))
{
  const GPtrArray *actives = nm_client_get_active_connections (client);

  for (guint i = 0; i < actives->len; i++)
    {
      NMActiveConnection *active = g_ptr_array_index (actives, i);

      if (nm_active_connection_get_state (active) == state && matches (active))
        return active;
    }
  return NULL;
}

static gboolean
is_loopback (NMActiveConnection *active)
{
  return g_strcmp0 (nm_active_connection_get_connection_type (active),
                    NM_SETTING_LOOPBACK_SETTING_NAME) == 0;
}

/* What a VPN can go over: anything up but another VPN or the loopback. */
static gboolean
is_carrier (NMActiveConnection *active)
{
  return !is_vpn (active) && !is_loopback (active);
}

/* The connection the icon shows: the primary one, or, where a VPN is primary, the one it
   goes over. NULL with none up. */
static NMActiveConnection *
base_connection (NMClient *client)
{
  NMActiveConnection *primary = nm_client_get_primary_connection (client);
  const char *over_path;
  NMObject *over = NULL;
  NMActiveConnection *carrier;

  if (primary == NULL || !is_vpn (primary))
    return primary;
  /* A VPN's specific object is the active connection it goes over. WireGuard's is empty,
     which libnm gives as NULL. */
  over_path = nm_active_connection_get_specific_object_path (primary);
  if (over_path != NULL)
    over = nm_client_get_object_by_path (client, over_path);
  if (NM_IS_ACTIVE_CONNECTION (over))
    return NM_ACTIVE_CONNECTION (over);
  /* Ethernet or Wi-Fi first, over a bridge or a tun that may be up beside them. */
  carrier = find_active (client, NM_ACTIVE_CONNECTION_STATE_ACTIVATED, is_physical);
  if (carrier == NULL)
    carrier = find_active (client, NM_ACTIVE_CONNECTION_STATE_ACTIVATED, is_carrier);
  return carrier;
}

/* The Wi-Fi device under active, NULL where it isn't Wi-Fi. */
static NMDeviceWifi *
wifi_device (NMActiveConnection *active)
{
  const GPtrArray *devices = nm_active_connection_get_devices (active);

  for (guint i = 0; i < devices->len; i++)
    if (NM_IS_DEVICE_WIFI (g_ptr_array_index (devices, i)))
      return g_ptr_array_index (devices, i);
  return NULL;
}

static void on_notify (GObject *object, GParamSpec *pspec, gpointer data);

static void
watch (NwNm *nm, gpointer object)
{
  g_signal_connect (object, "notify", G_CALLBACK (on_notify), nm);
  g_ptr_array_add (nm->watched, g_object_ref (object));
}

static void
unwatch_all (NwNm *nm)
{
  for (guint i = 0; i < nm->watched->len; i++)
    g_signal_handlers_disconnect_by_data (g_ptr_array_index (nm->watched, i), nm);
  g_ptr_array_set_size (nm->watched, 0);
}

/* Fills status from the client, and watches what it was read from. */
static void
read_status (NwNm *nm, NwStatus *status)
{
  NMClient *client = nm->client;
  const GPtrArray *actives;
  NMActiveConnection *base;
  NMActiveConnection *vpn;
  NMConnectivityState connectivity;

  *status = (NwStatus) { .strength = -1 };
  if (client == NULL || !nm_client_get_nm_running (client))
    return;
  status->running = TRUE;
  status->airplane = !nm_client_wireless_get_enabled (client)
                     && !nm_client_wwan_get_enabled (client);
  actives = nm_client_get_active_connections (client);
  for (guint i = 0; i < actives->len; i++)
    watch (nm, g_ptr_array_index (actives, i));

  base = base_connection (client);
  if (base == NULL)
    {
      /* Nothing up: whatever is coming up. */
      base = find_active (client, NM_ACTIVE_CONNECTION_STATE_ACTIVATING, is_physical);
      if (base != NULL)
        status->phase = NW_PHASE_CONNECTING;
    }
  else
    {
      status->phase = NW_PHASE_CONNECTED;
      connectivity = nm_client_get_connectivity (client);
      status->limited = connectivity == NM_CONNECTIVITY_PORTAL
                        || connectivity == NM_CONNECTIVITY_LIMITED;
    }
  if (base != NULL)
    {
      NMDeviceWifi *device = wifi_device (base);

      status->name = g_strdup (nm_active_connection_get_id (base));
      status->kind = is_wifi (base) || device != NULL ? NW_KIND_WIFI : NW_KIND_WIRED;
      if (device != NULL)
        {
          NMAccessPoint *ap = nm_device_wifi_get_active_access_point (device);

          watch (nm, device);
          if (ap != NULL)
            {
              watch (nm, ap);
              status->strength = nm_access_point_get_strength (ap);
            }
        }
    }

  vpn = find_active (client, NM_ACTIVE_CONNECTION_STATE_ACTIVATED, is_vpn);
  if (vpn != NULL)
    status->vpn = g_strdup (nm_active_connection_get_id (vpn));
}

static void
refresh (NwNm *nm)
{
  NwStatus status;

  unwatch_all (nm);
  read_status (nm, &status);
  if (nw_status_equal (&status, &nm->status))
    {
      nw_status_clear (&status);
      return;
    }
  nw_status_clear (&nm->status);
  nm->status = status;
  nm->changed (nm, nm->data);
}

static void
on_notify (GObject *object G_GNUC_UNUSED, GParamSpec *pspec G_GNUC_UNUSED, gpointer data)
{
  refresh (data);
}

static void
on_client_ready (GObject *source G_GNUC_UNUSED, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  NMClient *client = nm_client_new_finish (result, &error);
  NwNm *nm;

  if (client == NULL)
    {
      /* Freed before the answer, or no system bus at all: then it stays not running. */
      if (!g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
        g_warning ("can't read NetworkManager: %s", error->message);
      return;
    }
  nm = data;
  nm->client = client;
  /* NMClient follows NetworkManager's name: nm-running and the rest come as notify. */
  g_signal_connect (client, "notify", G_CALLBACK (on_notify), nm);
  refresh (nm);
}

NwNm *
nw_nm_new (NwNmChanged changed, gpointer data)
{
  NwNm *nm = g_new0 (NwNm, 1);

  nm->changed = changed;
  nm->data = data;
  nm->status.strength = -1;
  nm->watched = g_ptr_array_new_with_free_func (g_object_unref);
  nm->cancellable = g_cancellable_new ();
  nm_client_new_async (nm->cancellable, on_client_ready, nm);
  return nm;
}

const NwStatus *
nw_nm_status (NwNm *nm)
{
  return &nm->status;
}

void
nw_nm_free (NwNm *nm)
{
  g_cancellable_cancel (nm->cancellable);
  g_clear_object (&nm->cancellable);
  unwatch_all (nm);
  g_ptr_array_unref (nm->watched);
  if (nm->client != NULL)
    {
      g_signal_handlers_disconnect_by_data (nm->client, nm);
      g_clear_object (&nm->client);
    }
  nw_status_clear (&nm->status);
  g_free (nm);
}
