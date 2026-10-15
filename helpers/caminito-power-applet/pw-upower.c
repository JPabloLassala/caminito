/*
 * UPower as the Power applet reads it (262). See pw-upower.h.
 *
 * The proxy for the display device is made each time upowerd takes its name, and dropped
 * when it loses it, so that stopping and starting UPower is one more change and the
 * applet goes from no battery back to its state.
 */

#include "pw-upower.h"

#define UPOWER_NAME      "org.freedesktop.UPower"
#define DISPLAY_PATH     "/org/freedesktop/UPower/devices/DisplayDevice"
#define DEVICE_INTERFACE "org.freedesktop.UPower.Device"

struct _PwUpower
{
  GCancellable    *cancellable;
  guint            watch;
  GDBusProxy      *device;
  PwUpowerChanged  changed;
  gpointer         data;
  guint            idle;
};

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
  g_dbus_proxy_new (connection, G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START, NULL, UPOWER_NAME,
                    DISPLAY_PATH, DEVICE_INTERFACE, upower->cancellable, on_device, upower);
}

static void
on_name_vanished (GDBusConnection *connection G_GNUC_UNUSED,
                  const char      *name G_GNUC_UNUSED,
                  gpointer         data)
{
  PwUpower *upower = data;

  g_clear_object (&upower->device);
  queue_changed (upower);
}

PwUpower *
pw_upower_new (PwUpowerChanged changed, gpointer data)
{
  PwUpower *upower = g_new0 (PwUpower, 1);

  upower->cancellable = g_cancellable_new ();
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
  g_bus_unwatch_name (upower->watch);
  g_clear_handle_id (&upower->idle, g_source_remove);
  g_clear_object (&upower->device);
  g_object_unref (upower->cancellable);
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
