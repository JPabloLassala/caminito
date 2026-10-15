/*
 * UPower as the Power applet reads it (262). See pw-upower.h.
 *
 * The proxy for the display device is made each time upowerd takes its name, and dropped
 * when it loses it, so that stopping and starting UPower is one more change and the
 * applet goes from no battery back to its state.
 *
 * The other devices (263) have a proxy each, from EnumerateDevices and then the manager's
 * DeviceAdded and DeviceRemoved, and are dropped together with the display device.
 */

#include "pw-upower.h"

#define UPOWER_NAME      "org.freedesktop.UPower"
#define UPOWER_PATH      "/org/freedesktop/UPower"
#define UPOWER_INTERFACE "org.freedesktop.UPower"
#define DISPLAY_PATH     "/org/freedesktop/UPower/devices/DisplayDevice"
#define DEVICE_INTERFACE "org.freedesktop.UPower.Device"

struct _PwUpower
{
  GCancellable    *cancellable;
  guint            watch;
  GDBusProxy      *device;
  /* UPower's manager object, for the devices that come and go. */
  GDBusProxy      *manager;
  /* Device path to its GDBusProxy, and the paths whose proxies are being made. */
  GHashTable      *devices;
  GHashTable      *pending;
  /* The calls of this appearance of upowerd: cancelled when it goes. */
  GCancellable    *appearance;
  PwUpowerChanged  changed;
  gpointer         data;
  guint            idle;
};

typedef struct
{
  PwUpower *upower;
  char     *path;
} DeviceRequest;

static void
device_request_free (DeviceRequest *request)
{
  g_free (request->path);
  g_free (request);
}

static gboolean
emit_changed (gpointer data)
{
  PwUpower *upower = data;

  upower->idle = 0;
  upower->changed (upower, upower->data);
  return G_SOURCE_REMOVE;
}

static void
queue_changed (PwUpower *upower)
{
  if (upower->idle == 0)
    upower->idle = g_idle_add (emit_changed, upower);
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
on_other_device (GObject *source G_GNUC_UNUSED, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  DeviceRequest *request = data;
  GDBusProxy *proxy = g_dbus_proxy_new_finish (result, &error);
  PwUpower *upower;

  if (proxy == NULL)
    {
      /* Cancelled: upowerd went, or the applet did, and request->upower with it. */
      if (!g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
        g_warning ("can't follow the UPower device %s: %s", request->path, error->message);
      device_request_free (request);
      return;
    }
  upower = request->upower;
  /* DeviceRemoved came while the proxy was made, or upowerd went again. */
  if (!g_hash_table_remove (upower->pending, request->path)
      || g_dbus_proxy_get_name_owner (proxy) == NULL)
    {
      g_object_unref (proxy);
      device_request_free (request);
      return;
    }
  g_signal_connect (proxy, "g-properties-changed", G_CALLBACK (on_properties_changed), upower);
  g_hash_table_replace (upower->devices, g_strdup (request->path), proxy);
  queue_changed (upower);
  device_request_free (request);
}

static void
add_device (PwUpower *upower, const char *path)
{
  DeviceRequest *request;

  /* The display device has a proxy of its own. */
  if (g_str_equal (path, DISPLAY_PATH) || g_hash_table_contains (upower->devices, path)
      || g_hash_table_contains (upower->pending, path))
    return;
  g_hash_table_add (upower->pending, g_strdup (path));
  request = g_new0 (DeviceRequest, 1);
  request->upower = upower;
  request->path = g_strdup (path);
  g_dbus_proxy_new (g_dbus_proxy_get_connection (upower->manager),
                    G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START, NULL, UPOWER_NAME, path,
                    DEVICE_INTERFACE, upower->appearance, on_other_device, request);
}

static void
on_manager_signal (GDBusProxy *proxy G_GNUC_UNUSED,
                   const char *sender G_GNUC_UNUSED,
                   const char *signal,
                   GVariant   *parameters,
                   gpointer    data)
{
  PwUpower *upower = data;
  const char *path;

  if (!g_variant_is_of_type (parameters, G_VARIANT_TYPE ("(o)")))
    return;
  g_variant_get (parameters, "(&o)", &path);
  if (g_str_equal (signal, "DeviceAdded"))
    add_device (upower, path);
  else if (g_str_equal (signal, "DeviceRemoved"))
    {
      g_hash_table_remove (upower->pending, path);
      if (g_hash_table_remove (upower->devices, path))
        queue_changed (upower);
    }
}

static void
on_enumerated (GObject *source, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = g_dbus_proxy_call_finish (G_DBUS_PROXY (source), result, &error);
  g_autoptr (GVariant) paths = NULL;
  GVariantIter iter;
  const char *path;

  if (reply == NULL)
    {
      if (!g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
        g_warning ("can't list UPower's devices: %s", error->message);
      return;
    }
  paths = g_variant_get_child_value (reply, 0);
  g_variant_iter_init (&iter, paths);
  while (g_variant_iter_next (&iter, "&o", &path))
    add_device (data, path);
}

static void
on_manager (GObject *source G_GNUC_UNUSED, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  GDBusProxy *manager = g_dbus_proxy_new_finish (result, &error);
  PwUpower *upower;

  if (manager == NULL)
    {
      if (!g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
        g_warning ("can't follow UPower's devices: %s", error->message);
      return;
    }
  upower = data;
  g_clear_object (&upower->manager);
  if (g_dbus_proxy_get_name_owner (manager) == NULL)
    {
      g_object_unref (manager);
      return;
    }
  upower->manager = manager;
  /* DeviceAdded and DeviceRemoved, before the list: a device that comes in between is in
     one of them at least, and a path twice is only one proxy. */
  g_signal_connect (manager, "g-signal", G_CALLBACK (on_manager_signal), upower);
  g_dbus_proxy_call (manager, "EnumerateDevices", NULL, G_DBUS_CALL_FLAGS_NONE, -1,
                     upower->appearance, on_enumerated, upower);
}

static void
forget_devices (PwUpower *upower)
{
  g_cancellable_cancel (upower->appearance);
  g_object_unref (upower->appearance);
  upower->appearance = g_cancellable_new ();
  g_clear_object (&upower->manager);
  g_hash_table_remove_all (upower->devices);
  g_hash_table_remove_all (upower->pending);
}

static void
on_device (GObject *source G_GNUC_UNUSED, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  GDBusProxy *device = g_dbus_proxy_new_finish (result, &error);
  PwUpower *upower;

  if (device == NULL)
    {
      /* Cancelled means the applet is gone, and upower with it. */
      if (!g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
        g_warning ("can't follow UPower: %s", error->message);
      return;
    }

  upower = data;
  g_clear_object (&upower->device);
  /* upowerd went again while the proxy was made: on_name_vanished has run, and the next
     appearance makes another. */
  if (g_dbus_proxy_get_name_owner (device) == NULL)
    {
      g_object_unref (device);
      return;
    }
  upower->device = device;
  g_signal_connect (device, "g-properties-changed", G_CALLBACK (on_properties_changed), upower);
  queue_changed (upower);
}

static void
on_name_appeared (GDBusConnection *connection,
                  const char      *name G_GNUC_UNUSED,
                  const char      *owner G_GNUC_UNUSED,
                  gpointer         data)
{
  PwUpower *upower = data;

  g_clear_object (&upower->device);
  forget_devices (upower);
  g_dbus_proxy_new (connection, G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START, NULL, UPOWER_NAME,
                    DISPLAY_PATH, DEVICE_INTERFACE, upower->cancellable, on_device, upower);
  g_dbus_proxy_new (connection, G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START, NULL, UPOWER_NAME,
                    UPOWER_PATH, UPOWER_INTERFACE, upower->appearance, on_manager, upower);
}

static void
on_name_vanished (GDBusConnection *connection G_GNUC_UNUSED,
                  const char      *name G_GNUC_UNUSED,
                  gpointer         data)
{
  PwUpower *upower = data;

  g_clear_object (&upower->device);
  forget_devices (upower);
  queue_changed (upower);
}

PwUpower *
pw_upower_new (PwUpowerChanged changed, gpointer data)
{
  PwUpower *upower = g_new0 (PwUpower, 1);

  upower->cancellable = g_cancellable_new ();
  upower->appearance = g_cancellable_new ();
  upower->devices = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, g_object_unref);
  upower->pending = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  upower->changed = changed;
  upower->data = data;
  upower->watch = g_bus_watch_name (G_BUS_TYPE_SYSTEM, UPOWER_NAME, G_BUS_NAME_WATCHER_FLAGS_NONE,
                                    on_name_appeared, on_name_vanished, upower, NULL);
  return upower;
}

void
pw_upower_free (PwUpower *upower)
{
  g_cancellable_cancel (upower->cancellable);
  g_cancellable_cancel (upower->appearance);
  g_bus_unwatch_name (upower->watch);
  g_clear_handle_id (&upower->idle, g_source_remove);
  g_clear_object (&upower->device);
  g_clear_object (&upower->manager);
  g_hash_table_unref (upower->devices);
  g_hash_table_unref (upower->pending);
  g_object_unref (upower->cancellable);
  g_object_unref (upower->appearance);
  g_free (upower);
}

void
pw_upower_display (PwUpower *upower, PwDisplay *display)
{
  g_autoptr (GVariant) present = NULL;
  g_autoptr (GVariant) state = NULL;
  g_autoptr (GVariant) percentage = NULL;
  g_autoptr (GVariant) to_empty = NULL;
  g_autoptr (GVariant) to_full = NULL;

  *display = (PwDisplay) { 0 };
  if (upower->device == NULL)
    return;
  present = g_dbus_proxy_get_cached_property (upower->device, "IsPresent");
  state = g_dbus_proxy_get_cached_property (upower->device, "State");
  percentage = g_dbus_proxy_get_cached_property (upower->device, "Percentage");
  to_empty = g_dbus_proxy_get_cached_property (upower->device, "TimeToEmpty");
  to_full = g_dbus_proxy_get_cached_property (upower->device, "TimeToFull");
  if (present == NULL || !g_variant_is_of_type (present, G_VARIANT_TYPE_BOOLEAN))
    return;
  display->present = g_variant_get_boolean (present);
  if (state != NULL && g_variant_is_of_type (state, G_VARIANT_TYPE_UINT32))
    display->state = g_variant_get_uint32 (state);
  if (percentage != NULL && g_variant_is_of_type (percentage, G_VARIANT_TYPE_DOUBLE))
    display->percentage = g_variant_get_double (percentage);
  if (to_empty != NULL && g_variant_is_of_type (to_empty, G_VARIANT_TYPE_INT64))
    display->time_to_empty = g_variant_get_int64 (to_empty);
  if (to_full != NULL && g_variant_is_of_type (to_full, G_VARIANT_TYPE_INT64))
    display->time_to_full = g_variant_get_int64 (to_full);
}

static gboolean
bool_property (GDBusProxy *proxy, const char *name, gboolean fallback)
{
  g_autoptr (GVariant) value = g_dbus_proxy_get_cached_property (proxy, name);

  if (value != NULL && g_variant_is_of_type (value, G_VARIANT_TYPE_BOOLEAN))
    return g_variant_get_boolean (value);
  return fallback;
}

static guint
uint_property (GDBusProxy *proxy, const char *name, guint fallback)
{
  g_autoptr (GVariant) value = g_dbus_proxy_get_cached_property (proxy, name);

  if (value != NULL && g_variant_is_of_type (value, G_VARIANT_TYPE_UINT32))
    return g_variant_get_uint32 (value);
  return fallback;
}

static double
double_property (GDBusProxy *proxy, const char *name)
{
  g_autoptr (GVariant) value = g_dbus_proxy_get_cached_property (proxy, name);

  if (value != NULL && g_variant_is_of_type (value, G_VARIANT_TYPE_DOUBLE))
    return g_variant_get_double (value);
  return 0;
}

static PwDevice *
device_from (const char *path, GDBusProxy *proxy)
{
  PwDevice *device = pw_device_new ();
  g_autoptr (GVariant) model = g_dbus_proxy_get_cached_property (proxy, "Model");
  guint level;

  device->path = g_strdup (path);
  device->type = uint_property (proxy, "Type", PW_TYPE_UNKNOWN);
  device->power_supply = bool_property (proxy, "PowerSupply", FALSE);
  device->present = bool_property (proxy, "IsPresent", FALSE);
  device->state = uint_property (proxy, "State", PW_STATE_UNKNOWN);
  device->percentage = double_property (proxy, "Percentage");
  /* BatteryLevel is "none" (1) where Percentage is the measure, and the level's name
     where it isn't; a UPower that has no such property has only percentages. */
  level = uint_property (proxy, "BatteryLevel", 1);
  device->has_percentage = level == 1;
  device->capacity = double_property (proxy, "Capacity");
  device->threshold_supported = bool_property (proxy, "ChargeThresholdSupported", FALSE);
  device->threshold_enabled = bool_property (proxy, "ChargeThresholdEnabled", FALSE);
  device->threshold_end = uint_property (proxy, "ChargeEndThreshold", 0);
  if (model != NULL && g_variant_is_of_type (model, G_VARIANT_TYPE_STRING))
    device->model = g_variant_dup_string (model, NULL);
  return device;
}

GPtrArray *
pw_upower_devices (PwUpower *upower)
{
  GPtrArray *devices = g_ptr_array_new_with_free_func ((GDestroyNotify) pw_device_free);
  GHashTableIter iter;
  gpointer path, proxy;

  g_hash_table_iter_init (&iter, upower->devices);
  while (g_hash_table_iter_next (&iter, &path, &proxy))
    g_ptr_array_add (devices, device_from (path, proxy));
  return devices;
}

typedef struct
{
  /* The applet's: cancelled once PwUpower and what done was given are gone. */
  GCancellable *cancellable;
  char         *path;
  PwUpowerDone  done;
  gpointer      data;
} ThresholdCall;

static void
threshold_call_free (ThresholdCall *call)
{
  g_object_unref (call->cancellable);
  g_free (call->path);
  g_free (call);
}

static void
on_threshold (GObject *source, GAsyncResult *result, gpointer data)
{
  g_autoptr (GError) error = NULL;
  ThresholdCall *call = data;
  g_autoptr (GVariant) reply = g_dbus_proxy_call_finish (G_DBUS_PROXY (source), result, &error);

  /* The applet is gone, and call->done's data with it. */
  if (reply == NULL && g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
    {
      threshold_call_free (call);
      return;
    }
  if (error != NULL)
    g_dbus_error_strip_remote_error (error);
  call->done (call->path, error != NULL ? error->message : NULL, call->data);
  threshold_call_free (call);
}

static gboolean
on_threshold_missing (gpointer data)
{
  ThresholdCall *call = data;

  if (!g_cancellable_is_cancelled (call->cancellable))
    call->done (call->path, "The battery is gone", call->data);
  threshold_call_free (call);
  return G_SOURCE_REMOVE;
}

void
pw_upower_enable_charge_threshold (PwUpower    *upower,
                                   const char  *path,
                                   gboolean     enable,
                                   PwUpowerDone done,
                                   gpointer     data)
{
  ThresholdCall *call = g_new0 (ThresholdCall, 1);
  GDBusProxy *proxy = g_hash_table_lookup (upower->devices, path);

  call->cancellable = g_object_ref (upower->cancellable);
  call->path = g_strdup (path);
  call->done = done;
  call->data = data;
  if (proxy == NULL)
    {
      g_idle_add (on_threshold_missing, call);
      return;
    }
  /* Polkit may ask the user, which takes as long as the user does. */
  g_dbus_proxy_call (proxy, "EnableChargeThreshold", g_variant_new ("(b)", enable),
                     G_DBUS_CALL_FLAGS_ALLOW_INTERACTIVE_AUTHORIZATION, G_MAXINT,
                     upower->cancellable, on_threshold, call);
}
