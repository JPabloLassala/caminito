/*
 * What the Bluetooth applet shows (259). See bt-state.h.
 */

#include "bt-state.h"

static void
adapter_free (gpointer data)
{
  BtAdapter *adapter = data;

  g_free (adapter->path);
  g_free (adapter);
}

static void
device_free (gpointer data)
{
  BtDevice *device = data;

  g_free (device->path);
  g_free (device->adapter);
  g_free (device->alias);
  g_free (device);
}

BtSnapshot *
bt_snapshot_new (void)
{
  BtSnapshot *snapshot = g_new0 (BtSnapshot, 1);

  snapshot->adapters = g_ptr_array_new_with_free_func (adapter_free);
  snapshot->devices = g_ptr_array_new_with_free_func (device_free);
  return snapshot;
}

void
bt_snapshot_free (BtSnapshot *snapshot)
{
  g_ptr_array_unref (snapshot->adapters);
  g_ptr_array_unref (snapshot->devices);
  g_free (snapshot);
}

void
bt_snapshot_add_adapter (BtSnapshot *snapshot, const char *path, gboolean powered)
{
  BtAdapter *adapter = g_new0 (BtAdapter, 1);

  adapter->path = g_strdup (path);
  adapter->powered = powered;
  g_ptr_array_add (snapshot->adapters, adapter);
}

void
bt_snapshot_add_device (BtSnapshot *snapshot,
                        const char *path,
                        const char *adapter,
                        const char *alias,
                        gboolean    connected,
                        int         battery)
{
  BtDevice *device = g_new0 (BtDevice, 1);

  device->path = g_strdup (path);
  device->adapter = g_strdup (adapter);
  device->alias = g_strdup (alias);
  device->connected = connected;
  device->battery = battery;
  g_ptr_array_add (snapshot->devices, device);
}

static gboolean
adapter_powered (const BtSnapshot *snapshot, const char *path)
{
  for (guint i = 0; i < snapshot->adapters->len; i++)
    {
      const BtAdapter *adapter = g_ptr_array_index (snapshot->adapters, i);

      if (g_strcmp0 (adapter->path, path) == 0)
        return adapter->powered;
    }
  return FALSE;
}

static int
compare_paths (gconstpointer a, gconstpointer b)
{
  const BtDevice *const *first = a;
  const BtDevice *const *second = b;

  return g_strcmp0 ((*first)->path, (*second)->path);
}

GPtrArray *
bt_state_connected (const BtSnapshot *snapshot)
{
  GPtrArray *connected = g_ptr_array_new ();

  for (guint i = 0; i < snapshot->devices->len; i++)
    {
      BtDevice *device = g_ptr_array_index (snapshot->devices, i);

      if (device->connected && adapter_powered (snapshot, device->adapter))
        g_ptr_array_add (connected, device);
    }
  g_ptr_array_sort (connected, compare_paths);
  return connected;
}

BtIconState
bt_state_icon (const BtSnapshot *snapshot)
{
  g_autoptr (GPtrArray) connected = NULL;
  gboolean powered = FALSE;

  if (snapshot->adapters->len == 0)
    return BT_ICON_NONE;
  for (guint i = 0; i < snapshot->adapters->len; i++)
    powered |= ((const BtAdapter *) g_ptr_array_index (snapshot->adapters, i))->powered;
  if (!powered)
    return BT_ICON_OFF;
  connected = bt_state_connected (snapshot);
  return connected->len > 0 ? BT_ICON_CONNECTED : BT_ICON_ON;
}

char *
bt_state_tooltip (const BtSnapshot *snapshot)
{
  g_autoptr (GPtrArray) connected = NULL;
  GString *tooltip;

  switch (bt_state_icon (snapshot))
    {
    case BT_ICON_NONE:
      return NULL;
    case BT_ICON_OFF:
      return g_strdup ("Bluetooth is off");
    case BT_ICON_ON:
      return g_strdup ("Bluetooth is on\nNo devices connected");
    case BT_ICON_CONNECTED:
    default:
      break;
    }

  tooltip = g_string_new ("Bluetooth is on");
  connected = bt_state_connected (snapshot);
  for (guint i = 0; i < connected->len; i++)
    {
      const BtDevice *device = g_ptr_array_index (connected, i);

      g_string_append_printf (tooltip, "\n%s", device->alias);
      if (device->battery >= 0)
        g_string_append_printf (tooltip, ": %d%%", device->battery);
    }
  return g_string_free (tooltip, FALSE);
}

const char *const *
bt_state_icon_names (BtIconState state)
{
  /* MATE's themes have the plain names, and Adwaita the symbolic ones, with no name for
     connected. Breeze has neither, only its own three. GTK takes the first name the
     current theme has before looking in the themes it inherits, so Breeze's come last
     without losing to hicolor's or Adwaita's. */
  static const char *const off[] = {
    "bluetooth-disabled", "bluetooth-disabled-symbolic", "network-bluetooth-inactive", NULL,
  };
  static const char *const on[] = {
    "bluetooth-active", "bluetooth-active-symbolic", "network-bluetooth", NULL,
  };
  static const char *const connected[] = {
    "bluetooth-paired", "bluetooth-active-symbolic", "network-bluetooth-activated", NULL,
  };

  switch (state)
    {
    case BT_ICON_OFF:
      return off;
    case BT_ICON_ON:
      return on;
    case BT_ICON_CONNECTED:
      return connected;
    case BT_ICON_NONE:
    default:
      return NULL;
    }
}
