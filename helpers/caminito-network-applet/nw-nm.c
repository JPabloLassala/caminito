/*
 * NetworkManager as the Network applet reads it (CMN-193). See nw-nm.h.
 *
 * NMClient follows NetworkManager's name and its objects. The status is built again from it
 * on any change to the client, to an active connection, to the device under the connection,
 * to its access point or its IPv4 configuration, or to the wired device; those objects are
 * watched again each time, since which ones matter changes with the status.
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
  /* The radios as they were when the Airplane mode tile turned them off, which turning it
     off again restores. Forgotten when airplane mode ends some other way. */
  gboolean      radios_saved;
  gboolean      saved_wireless;
  gboolean      saved_wwan;
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
is_vpn_connection (NMConnection *connection)
{
  return nm_connection_is_type (connection, NM_SETTING_VPN_SETTING_NAME)
         || nm_connection_is_type (connection, NM_SETTING_WIREGUARD_SETTING_NAME);
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

static guint64
timestamp_of (NMConnection *connection)
{
  NMSettingConnection *setting = nm_connection_get_setting_connection (connection);

  return setting != NULL ? nm_setting_connection_get_timestamp (setting) : 0;
}

/* Of connections, the one activated most recently that matches, the first on a tie; NULL
   with none. */
static NMRemoteConnection *
most_recent (const GPtrArray *connections, gboolean (*matches) (NMConnection *))
{
  NMRemoteConnection *found = NULL;

  for (guint i = 0; i < connections->len; i++)
    {
      NMConnection *connection = g_ptr_array_index (connections, i);

      if ((matches == NULL || matches (connection))
          && (found == NULL || timestamp_of (connection) > timestamp_of (NM_CONNECTION (found))))
        found = NM_REMOTE_CONNECTION (connection);
    }
  return found;
}

/* The wired device the Wired tile is for: the first ethernet device NetworkManager
   manages, one that's up before one with its cable in before the rest. Not a veth, which a
   container's network is. NULL with none. */
static NMDevice *
wired_device (NMClient *client)
{
  const GPtrArray *devices = nm_client_get_devices (client);
  NMDevice *found = NULL;

  for (guint i = 0; i < devices->len; i++)
    {
      NMDevice *device = g_ptr_array_index (devices, i);
      NMDeviceState state = nm_device_get_state (device);

      if (!NM_IS_DEVICE_ETHERNET (device) || NM_IS_DEVICE_VETH (device)
          || state <= NM_DEVICE_STATE_UNMANAGED)
        continue;
      if (state == NM_DEVICE_STATE_ACTIVATED)
        return device;
      if (found == NULL
          || (state > NM_DEVICE_STATE_UNAVAILABLE
              && nm_device_get_state (found) <= NM_DEVICE_STATE_UNAVAILABLE))
        found = device;
    }
  return found;
}

static NwWired
wired_of (NMDevice *device)
{
  NMDeviceState state;

  if (device == NULL)
    return NW_WIRED_ABSENT;
  state = nm_device_get_state (device);
  if (state <= NM_DEVICE_STATE_UNAVAILABLE)
    return NW_WIRED_UNPLUGGED;
  if (state == NM_DEVICE_STATE_ACTIVATED)
    return NW_WIRED_CONNECTED;
  if (state >= NM_DEVICE_STATE_PREPARE && state < NM_DEVICE_STATE_ACTIVATED)
    return NW_WIRED_CONNECTING;
  return NW_WIRED_DISCONNECTED;
}

/* What the VPN tile turns on and off: the VPN up, or else the one activated most
   recently. NULL with none set up. */
static NMConnection *
vpn_for_tile (NMClient *client)
{
  NMActiveConnection *vpn = find_active (client, NM_ACTIVE_CONNECTION_STATE_ACTIVATED, is_vpn);

  if (vpn != NULL && nm_active_connection_get_connection (vpn) != NULL)
    return NM_CONNECTION (nm_active_connection_get_connection (vpn));
  return (NMConnection *) most_recent (nm_client_get_connections (client), is_vpn_connection);
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
  NMActiveConnection *wifi;
  NMDevice *wired;
  NMConnection *last_vpn;
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

  status->wifi_enabled = nm_client_wireless_get_enabled (client);
  status->wifi_blocked = !nm_client_wireless_hardware_get_enabled (client);
  wifi = find_active (client, NM_ACTIVE_CONNECTION_STATE_ACTIVATED, is_wifi);
  if (wifi != NULL)
    status->wifi_network = g_strdup (nm_active_connection_get_id (wifi));
  wired = wired_device (client);
  status->wired = wired_of (wired);
  if (wired != NULL)
    watch (nm, wired);
  last_vpn = vpn_for_tile (client);
  if (last_vpn != NULL)
    status->last_vpn = g_strdup (nm_connection_get_id (last_vpn));

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
      const GPtrArray *devices = nm_active_connection_get_devices (base);
      NMIPConfig *ip4 = nm_active_connection_get_ip4_config (base);

      status->name = g_strdup (nm_active_connection_get_id (base));
      if (status->phase == NW_PHASE_CONNECTED && ip4 != NULL)
        {
          GPtrArray *addresses = nm_ip_config_get_addresses (ip4);

          watch (nm, ip4);
          if (addresses->len > 0)
            status->address = g_strdup (nm_ip_address_get_address (
              g_ptr_array_index (addresses, 0)));
        }
      if (devices->len > 0 && NM_IS_DEVICE_ETHERNET (g_ptr_array_index (devices, 0)))
        {
          NMDeviceEthernet *ethernet = g_ptr_array_index (devices, 0);

          if (ethernet != (gpointer) wired)
            watch (nm, ethernet);
          status->speed = nm_device_ethernet_get_speed (ethernet);
        }
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
  /* Airplane mode ended elsewhere: the radios saved are no longer what turning it off
     here should bring back. */
  if (nm->status.airplane && !status.airplane)
    nm->radios_saved = FALSE;
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

/* The end of a call that the status follows: only a failure is worth saying, and not the
   applet going away. what is the call, as "can't <what>". */
static void
warn_unless_cancelled (const GError *error, const char *what)
{
  if (error != NULL && !g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
    g_warning ("can't %s: %s", what, error->message);
}

static void
on_set_done (GObject *source, GAsyncResult *result, gpointer what)
{
  g_autoptr (GError) error = NULL;

  nm_client_dbus_set_property_finish (NM_CLIENT (source), result, &error);
  warn_unless_cancelled (error, what);
}

static void
on_activate_done (GObject *source, GAsyncResult *result, gpointer what)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (NMActiveConnection) active = nm_client_activate_connection_finish (
    NM_CLIENT (source), result, &error);

  warn_unless_cancelled (error, what);
}

static void
on_deactivate_done (GObject *source, GAsyncResult *result, gpointer what)
{
  g_autoptr (GError) error = NULL;

  nm_client_deactivate_connection_finish (NM_CLIENT (source), result, &error);
  warn_unless_cancelled (error, what);
}

static void
on_disconnect_done (GObject *source, GAsyncResult *result, gpointer what)
{
  g_autoptr (GError) error = NULL;

  nm_device_disconnect_finish (NM_DEVICE (source), result, &error);
  warn_unless_cancelled (error, what);
}

/* WirelessEnabled or WwanEnabled, which nm_client_wireless_set_enabled() would set without
   saying whether NetworkManager refused. */
static void
set_radio (NwNm *nm, const char *property, gboolean on, const char *what)
{
  nm_client_dbus_set_property (nm->client, NM_DBUS_PATH, NM_DBUS_INTERFACE, property,
                               g_variant_new_boolean (on), -1, nm->cancellable, on_set_done,
                               (gpointer) what);
}

static void
deactivate (NwNm *nm, NMActiveConnection *active, const char *what)
{
  nm_client_deactivate_connection_async (nm->client, active, nm->cancellable,
                                         on_deactivate_done, (gpointer) what);
}

/* As nmcli device disconnect: the device stays down until it's connected again, where
   deactivating its connection would let NetworkManager bring up another of its own. */
static void
disconnect_device (NwNm *nm, NMDevice *device, const char *what)
{
  nm_device_disconnect_async (device, nm->cancellable, on_disconnect_done, (gpointer) what);
}

static void
set_wired (NwNm *nm, gboolean on)
{
  NMDevice *device = wired_device (nm->client);

  if (device == NULL)
    return;
  if (on)
    {
      /* The connection the device had last. With none saved for it, NetworkManager picks
         the best one it has, as nmcli device connect does. */
      NMRemoteConnection *connection = most_recent (
        nm_device_get_available_connections (device), NULL);

      nm_client_activate_connection_async (nm->client, NM_CONNECTION (connection), device,
                                           NULL, nm->cancellable, on_activate_done,
                                           "connect the wired device");
      return;
    }
  disconnect_device (nm, device, "disconnect the wired device");
}

static void
set_vpn (NwNm *nm, gboolean on)
{
  NMConnection *vpn;
  NMActiveConnection *active;

  if (!on)
    {
      /* The one the tile shows. */
      active = find_active (nm->client, NM_ACTIVE_CONNECTION_STATE_ACTIVATED, is_vpn);
      if (active != NULL)
        deactivate (nm, active, "turn the VPN off");
      return;
    }
  vpn = vpn_for_tile (nm->client);
  if (vpn != NULL)
    nm_client_activate_connection_async (nm->client, vpn, NULL, NULL, nm->cancellable,
                                         on_activate_done, "turn the VPN on");
}

static void
set_airplane (NwNm *nm, gboolean on)
{
  gboolean wireless = TRUE;
  gboolean wwan = TRUE;

  if (on)
    {
      nm->radios_saved = TRUE;
      nm->saved_wireless = nm_client_wireless_get_enabled (nm->client);
      nm->saved_wwan = nm_client_wwan_get_enabled (nm->client);
      wireless = wwan = FALSE;
    }
  else if (nm->radios_saved)
    {
      wireless = nm->saved_wireless;
      wwan = nm->saved_wwan;
      nm->radios_saved = FALSE;
    }
  /* Without the radios as they were, every one comes back on. */
  set_radio (nm, "WirelessEnabled", wireless, "turn airplane mode on or off");
  set_radio (nm, "WwanEnabled", wwan, "turn airplane mode on or off");
}

void
nw_nm_set_tile (NwNm *nm, NwTile tile, gboolean on)
{
  if (!nm->status.running)
    return;
  switch (tile)
    {
    case NW_TILE_WIFI:
      set_radio (nm, "WirelessEnabled", on, "turn Wi-Fi on or off");
      break;
    case NW_TILE_WIRED:
      set_wired (nm, on);
      break;
    case NW_TILE_VPN:
      set_vpn (nm, on);
      break;
    case NW_TILE_AIRPLANE:
      set_airplane (nm, on);
      break;
    case NW_N_TILES:
      g_return_if_reached ();
    }
}

void
nw_nm_disconnect (NwNm *nm)
{
  NMActiveConnection *base;
  const GPtrArray *devices;

  if (!nm->status.running)
    return;
  base = base_connection (nm->client);
  if (base == NULL)
    return;
  devices = nm_active_connection_get_devices (base);
  if (devices->len > 0)
    disconnect_device (nm, g_ptr_array_index (devices, 0), "disconnect");
  else
    deactivate (nm, base, "disconnect");
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
