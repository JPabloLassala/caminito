/*
 * BlueZ as the Bluetooth applet reads it (259). See bt-bluez.h.
 *
 * GDBusObjectManagerClient follows bluetoothd's name: when it goes, every object is
 * removed, and when it comes back, they're added again. So stopping and starting BlueZ is
 * one more burst of changes, and the applet goes from no adapter back to its state.
 */

#include "bt-bluez.h"

#define BLUEZ_NAME        "org.bluez"
#define ADAPTER_INTERFACE "org.bluez.Adapter1"
#define DEVICE_INTERFACE  "org.bluez.Device1"
#define BATTERY_INTERFACE "org.bluez.Battery1"

/* An unblocked adapter is bluetoothd's to power once it has seen the rfkill event, which
   it reads in its own time: until then Set fails with org.bluez.Error.Blocked. */
#define POWER_ON_TRIES    10
#define POWER_ON_INTERVAL_MS 300

struct _BtBluez
{
  GCancellable       *cancellable;
  GDBusObjectManager *manager;
  BtBluezChanged      changed;
  gpointer            data;
  guint               idle;
};

typedef struct
{
  GDBusProxy *adapter;
  gboolean    powered;
  int         tries_left;
} SetPowered;

static gboolean
emit_changed (gpointer data)
{
  BtBluez *bluez = data;

  bluez->idle = 0;
  bluez->changed (bluez, bluez->data);
  return G_SOURCE_REMOVE;
}

static void
queue_changed (BtBluez *bluez)
{
  if (bluez->idle == 0)
    bluez->idle = g_idle_add (emit_changed, bluez);
}

static void
on_object_changed (GDBusObjectManager *manager G_GNUC_UNUSED,
                   GDBusObject        *object G_GNUC_UNUSED,
                   gpointer            data)
{
  queue_changed (data);
}

static void
on_interface_changed (GDBusObjectManager *manager G_GNUC_UNUSED,
                      GDBusObject        *object G_GNUC_UNUSED,
                      GDBusInterface     *interface G_GNUC_UNUSED,
                      gpointer            data)
{
  queue_changed (data);
}

static void
on_properties_changed (GDBusObjectManagerClient *manager G_GNUC_UNUSED,
                       GDBusObjectProxy         *object G_GNUC_UNUSED,
                       GDBusProxy               *interface,
                       GVariant                 *changed G_GNUC_UNUSED,
                       const char *const        *invalidated G_GNUC_UNUSED,
                       gpointer                  data)
{
  const char *name = g_dbus_proxy_get_interface_name (interface);

  /* BlueZ changes a device's RSSI and the like all the time while it scans. */
  if (g_str_equal (name, ADAPTER_INTERFACE) || g_str_equal (name, DEVICE_INTERFACE)
      || g_str_equal (name, BATTERY_INTERFACE))
    queue_changed (data);
}

static void
on_name_owner (GObject *manager G_GNUC_UNUSED, GParamSpec *pspec G_GNUC_UNUSED, gpointer data)
{
  queue_changed (data);
}

static void
on_manager (GObject *source G_GNUC_UNUSED, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  GDBusObjectManager *manager;
  BtBluez *bluez;

  manager = g_dbus_object_manager_client_new_for_bus_finish (result, &error);
  if (manager == NULL)
    {
      /* Cancelled means the applet is gone, and bluez with it. */
      if (!g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
        g_warning ("can't follow BlueZ: %s", error->message);
      return;
    }

  bluez = data;
  bluez->manager = manager;
  g_signal_connect (manager, "object-added", G_CALLBACK (on_object_changed), bluez);
  g_signal_connect (manager, "object-removed", G_CALLBACK (on_object_changed), bluez);
  g_signal_connect (manager, "interface-added", G_CALLBACK (on_interface_changed), bluez);
  g_signal_connect (manager, "interface-removed", G_CALLBACK (on_interface_changed), bluez);
  g_signal_connect (manager, "interface-proxy-properties-changed",
                    G_CALLBACK (on_properties_changed), bluez);
  g_signal_connect (manager, "notify::name-owner", G_CALLBACK (on_name_owner), bluez);
  queue_changed (bluez);
}

BtBluez *
bt_bluez_new (BtBluezChanged changed, gpointer data)
{
  BtBluez *bluez = g_new0 (BtBluez, 1);

  bluez->cancellable = g_cancellable_new ();
  bluez->changed = changed;
  bluez->data = data;
  /* Not auto-started: an applet on a computer without Bluetooth mustn't start it, and
     the client watches for the name all the same. */
  g_dbus_object_manager_client_new_for_bus (G_BUS_TYPE_SYSTEM,
                                            G_DBUS_OBJECT_MANAGER_CLIENT_FLAGS_DO_NOT_AUTO_START,
                                            BLUEZ_NAME,
                                            "/",
                                            NULL, NULL, NULL,
                                            bluez->cancellable,
                                            on_manager,
                                            bluez);
  return bluez;
}

void
bt_bluez_free (BtBluez *bluez)
{
  g_cancellable_cancel (bluez->cancellable);
  g_clear_object (&bluez->cancellable);
  if (bluez->manager != NULL)
    g_signal_handlers_disconnect_by_data (bluez->manager, bluez);
  g_clear_object (&bluez->manager);
  g_clear_handle_id (&bluez->idle, g_source_remove);
  g_free (bluez);
}

static gboolean
get_boolean (GDBusProxy *proxy, const char *name)
{
  g_autoptr (GVariant) value = g_dbus_proxy_get_cached_property (proxy, name);

  return value != NULL && g_variant_is_of_type (value, G_VARIANT_TYPE_BOOLEAN)
         && g_variant_get_boolean (value);
}

static char *
get_string (GDBusProxy *proxy, const char *name)
{
  g_autoptr (GVariant) value = g_dbus_proxy_get_cached_property (proxy, name);

  if (value == NULL
      || !(g_variant_is_of_type (value, G_VARIANT_TYPE_STRING)
           || g_variant_is_of_type (value, G_VARIANT_TYPE_OBJECT_PATH)))
    return NULL;
  return g_variant_dup_string (value, NULL);
}

static int
get_battery (GDBusObject *object)
{
  g_autoptr (GDBusInterface) battery = g_dbus_object_get_interface (object, BATTERY_INTERFACE);
  g_autoptr (GVariant) value = NULL;

  if (battery == NULL)
    return -1;
  value = g_dbus_proxy_get_cached_property (G_DBUS_PROXY (battery), "Percentage");
  if (value == NULL || !g_variant_is_of_type (value, G_VARIANT_TYPE_BYTE))
    return -1;
  return g_variant_get_byte (value);
}

BtSnapshot *
bt_bluez_snapshot (BtBluez *bluez)
{
  BtSnapshot *snapshot = bt_snapshot_new ();
  GList *objects;

  if (bluez->manager == NULL)
    return snapshot;

  objects = g_dbus_object_manager_get_objects (bluez->manager);
  for (GList *l = objects; l != NULL; l = l->next)
    {
      GDBusObject *object = l->data;
      const char *path = g_dbus_object_get_object_path (object);
      g_autoptr (GDBusInterface) adapter = g_dbus_object_get_interface (object, ADAPTER_INTERFACE);
      g_autoptr (GDBusInterface) device = g_dbus_object_get_interface (object, DEVICE_INTERFACE);

      if (adapter != NULL)
        bt_snapshot_add_adapter (snapshot, path, get_boolean (G_DBUS_PROXY (adapter), "Powered"));
      if (device != NULL)
        {
          GDBusProxy *proxy = G_DBUS_PROXY (device);
          g_autofree char *adapter_path = get_string (proxy, "Adapter");
          g_autofree char *alias = get_string (proxy, "Alias");

          bt_snapshot_add_device (snapshot, path, adapter_path,
                                  alias != NULL ? alias : path,
                                  get_boolean (proxy, "Connected"),
                                  get_battery (object));
        }
    }
  g_list_free_full (objects, g_object_unref);
  return snapshot;
}

static void set_powered (SetPowered *set);

static void
set_powered_free (SetPowered *set)
{
  g_object_unref (set->adapter);
  g_free (set);
}

static gboolean
retry_powered (gpointer data)
{
  set_powered (data);
  return G_SOURCE_REMOVE;
}

static void
on_set_powered (GObject *source, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = NULL;
  SetPowered *set = data;

  reply = g_dbus_connection_call_finish (G_DBUS_CONNECTION (source), result, &error);
  if (reply != NULL)
    {
      set_powered_free (set);
      return;
    }
  if (set->powered && --set->tries_left > 0)
    {
      g_timeout_add (POWER_ON_INTERVAL_MS, retry_powered, set);
      return;
    }
  g_warning ("can't power %s %s: %s", g_dbus_proxy_get_object_path (set->adapter),
             set->powered ? "on" : "off", error->message);
  set_powered_free (set);
}

/* Each call holds its adapter's proxy, so that a retry outlives the applet safely. */
static void
set_powered (SetPowered *set)
{
  g_dbus_connection_call (g_dbus_proxy_get_connection (set->adapter),
                          BLUEZ_NAME,
                          g_dbus_proxy_get_object_path (set->adapter),
                          "org.freedesktop.DBus.Properties",
                          "Set",
                          g_variant_new ("(ssv)", ADAPTER_INTERFACE, "Powered",
                                         g_variant_new_boolean (set->powered)),
                          NULL,
                          G_DBUS_CALL_FLAGS_NO_AUTO_START,
                          -1,
                          NULL,
                          on_set_powered,
                          set);
}

void
bt_bluez_set_powered (BtBluez *bluez, gboolean powered)
{
  GList *objects;

  if (bluez->manager == NULL)
    return;
  objects = g_dbus_object_manager_get_objects (bluez->manager);
  for (GList *l = objects; l != NULL; l = l->next)
    {
      GDBusInterface *adapter = g_dbus_object_get_interface (l->data, ADAPTER_INTERFACE);
      SetPowered *set;

      if (adapter == NULL)
        continue;
      set = g_new0 (SetPowered, 1);
      set->adapter = G_DBUS_PROXY (adapter);
      set->powered = powered;
      set->tries_left = POWER_ON_TRIES;
      set_powered (set);
    }
  g_list_free_full (objects, g_object_unref);
}
