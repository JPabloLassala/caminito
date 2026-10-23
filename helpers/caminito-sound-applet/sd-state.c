/*
 * What the Sound applet's icon and popup show (CMN-213, CMN-215, CMN-216). See sd-state.h.
 */

#include "sd-state.h"

#include <string.h>

static void devices_free (SdDevice **devices, guint *n_devices);
static gboolean devices_equal (const SdDevice *a, guint n_a, const SdDevice *b, guint n_b);
static void apps_free (SdApp **apps, guint *n_apps);
static gboolean apps_equal (const SdApp *a, guint n_a, const SdApp *b, guint n_b);

void
sd_card_clear (SdCard *card)
{
  for (guint i = 0; i < card->n_profiles; i++)
    {
      g_free (card->profiles[i].name);
      g_free (card->profiles[i].description);
    }
  g_clear_pointer (&card->profiles, g_free);
  card->n_profiles = 0;
  g_clear_pointer (&card->description, g_free);
  g_clear_pointer (&card->active, g_free);
}

void
sd_status_clear (SdStatus *status)
{
  for (guint i = 0; i < status->n_cards; i++)
    sd_card_clear (&status->cards[i]);
  g_clear_pointer (&status->cards, g_free);
  status->n_cards = 0;
  g_clear_pointer (&status->output_name, g_free);
  g_clear_pointer (&status->recording, g_strfreev);
  devices_free (&status->outputs, &status->n_outputs);
  devices_free (&status->inputs, &status->n_inputs);
  g_clear_pointer (&status->default_output, g_free);
  g_clear_pointer (&status->default_input, g_free);
  apps_free (&status->apps, &status->n_apps);
}

static gboolean
card_equal (const SdCard *a, const SdCard *b)
{
  if (a->index != b->index || a->n_profiles != b->n_profiles
      || g_strcmp0 (a->description, b->description) != 0
      || g_strcmp0 (a->active, b->active) != 0)
    return FALSE;
  for (guint i = 0; i < a->n_profiles; i++)
    if (g_strcmp0 (a->profiles[i].name, b->profiles[i].name) != 0
        || g_strcmp0 (a->profiles[i].description, b->profiles[i].description) != 0)
      return FALSE;
  return TRUE;
}

gboolean
sd_cards_equal (const SdCard *a, guint n_a, const SdCard *b, guint n_b)
{
  if (n_a != n_b)
    return FALSE;
  for (guint i = 0; i < n_a; i++)
    if (!card_equal (&a[i], &b[i]))
      return FALSE;
  return TRUE;
}

void
sd_card_copy (const SdCard *card, SdCard *copy)
{
  copy->index = card->index;
  copy->description = g_strdup (card->description);
  copy->active = g_strdup (card->active);
  copy->n_profiles = card->n_profiles;
  copy->profiles = g_new0 (SdProfile, card->n_profiles);
  for (guint i = 0; i < card->n_profiles; i++)
    {
      copy->profiles[i].name = g_strdup (card->profiles[i].name);
      copy->profiles[i].description = g_strdup (card->profiles[i].description);
    }
}

/* Either NULL, which is no names. */
static gboolean
strv_equal (char **a, char **b)
{
  if (a == NULL || b == NULL)
    return a == b;
  return g_strv_equal ((const char *const *) a, (const char *const *) b);
}

gboolean
sd_status_equal (const SdStatus *a, const SdStatus *b)
{
  return a->running == b->running && a->has_output == b->has_output
         && g_strcmp0 (a->output_name, b->output_name) == 0 && a->volume == b->volume
         && a->muted == b->muted && a->headphones == b->headphones
         && a->has_input == b->has_input && a->input_muted == b->input_muted
         && strv_equal (a->recording, b->recording)
         && sd_cards_equal (a->cards, a->n_cards, b->cards, b->n_cards)
         && devices_equal (a->outputs, a->n_outputs, b->outputs, b->n_outputs)
         && devices_equal (a->inputs, a->n_inputs, b->inputs, b->n_inputs)
         && g_strcmp0 (a->default_output, b->default_output) == 0
         && g_strcmp0 (a->default_input, b->default_input) == 0
         && apps_equal (a->apps, a->n_apps, b->apps, b->n_apps)
         && a->has_system_sounds == b->has_system_sounds
         && a->system_volume == b->system_volume;
}

/* As pactl rounds it. */
guint
sd_state_percent (pa_volume_t volume)
{
  return (guint) (((guint64) volume * 100 + PA_VOLUME_NORM / 2) / PA_VOLUME_NORM);
}

const char *const *
sd_state_icon_names (const SdStatus *status)
{
  /* MATE's themes have the plain names, Adwaita the symbolic ones. With no sound server,
     the speaker with no waves, which the applet strikes through: MATE's audio-volume-off,
     and the others' lowest level. Not Breeze's audio-off, which is struck through already. */
  static const char *const no_server[] = {
    "audio-volume-off", "audio-volume-low-symbolic", NULL,
  };
  static const char *const muted[] = { "audio-volume-muted", "audio-volume-muted-symbolic", NULL };
  static const char *const headphones[] = {
    "audio-headphones", "audio-headphones-symbolic", NULL,
  };
  static const char *const levels[3][3] = {
    { "audio-volume-low", "audio-volume-low-symbolic", NULL },
    { "audio-volume-medium", "audio-volume-medium-symbolic", NULL },
    { "audio-volume-high", "audio-volume-high-symbolic", NULL },
  };
  guint percent;

  if (!status->running)
    return no_server;
  if (!status->has_output)
    return muted;
  if (status->headphones)
    return headphones;
  percent = sd_state_percent (status->volume);
  if (status->muted || percent == 0)
    return muted;
  if (percent <= 33)
    return levels[0];
  if (percent <= 66)
    return levels[1];
  return levels[2];
}

gboolean
sd_state_icon_struck (const SdStatus *status)
{
  if (!status->running)
    return TRUE;
  return status->has_output && status->headphones && status->muted;
}

char *
sd_state_tooltip (const SdStatus *status)
{
  GString *tooltip;

  if (!status->running)
    return g_strdup ("No sound server");
  if (!status->has_output)
    tooltip = g_string_new ("No sound device");
  else
    {
      tooltip = g_string_new (NULL);
      g_string_append_printf (tooltip, "%s: %u%%", status->output_name,
                              sd_state_percent (status->volume));
      if (status->muted)
        g_string_append (tooltip, " (muted)");
    }
  if (status->recording != NULL && status->recording[0] != NULL)
    {
      g_autofree char *names = g_strjoinv (", ", status->recording);

      g_string_append_printf (tooltip, "\nRecording: %s", names);
    }
  return g_string_free (tooltip, FALSE);
}

/* In the percent the tooltip shows, so that 45% steps to 50% and not to 49.99%. */
pa_volume_t
sd_state_scroll_volume (pa_volume_t volume, guint step, gboolean up, gboolean above_100)
{
  guint percent = sd_state_percent (volume);
  guint cap = above_100 ? 150 : 100;

  if (!up)
    percent = percent > step ? percent - step : 0;
  else if (percent >= cap)
    return volume;
  else
    percent = MIN (percent + step, cap);
  return (pa_volume_t) (((guint64) percent * PA_VOLUME_NORM + 50) / 100);
}

static const char *
nonempty (const char *value)
{
  return value != NULL && value[0] != '\0' ? value : NULL;
}

const char *
sd_state_stream_identity (const SdStream *stream)
{
  /* Not the binary first: several Electron programs run as electron. Not the process:
     Chromium plays from an audio-service process. */
  return nonempty (stream->app_id) ?: nonempty (stream->name) ?: nonempty (stream->binary);
}

/* The name a row shows. */
static const char *
stream_app_name (const SdStream *stream)
{
  return nonempty (stream->name) ?: nonempty (stream->binary) ?: sd_state_stream_identity (stream);
}

char **
sd_state_recording_names (const SdStream *streams, guint n_streams)
{
  g_autoptr (GHashTable) seen = g_hash_table_new (g_str_hash, g_str_equal);
  GPtrArray *names = g_ptr_array_new ();

  for (guint i = 0; i < n_streams; i++)
    {
      const char *identity = sd_state_stream_identity (&streams[i]);

      if (identity == NULL || !g_hash_table_add (seen, (char *) identity))
        continue;
      g_ptr_array_add (names, g_strdup (stream_app_name (&streams[i])));
    }
  if (names->len == 0)
    {
      g_ptr_array_free (names, TRUE);
      return NULL;
    }
  g_ptr_array_add (names, NULL);
  return (char **) g_ptr_array_free (names, FALSE);
}

void
sd_device_clear (SdDevice *device)
{
  for (guint i = 0; i < device->n_ports; i++)
    {
      g_free (device->ports[i].name);
      g_free (device->ports[i].description);
    }
  g_clear_pointer (&device->ports, g_free);
  device->n_ports = 0;
  g_clear_pointer (&device->name, g_free);
  g_clear_pointer (&device->description, g_free);
  g_clear_pointer (&device->active_port, g_free);
  g_clear_pointer (&device->active_port_description, g_free);
}

void
sd_device_copy (const SdDevice *device, SdDevice *copy)
{
  *copy = *device;
  copy->name = g_strdup (device->name);
  copy->description = g_strdup (device->description);
  copy->active_port = g_strdup (device->active_port);
  copy->active_port_description = g_strdup (device->active_port_description);
  copy->ports = g_new0 (SdPort, device->n_ports);
  for (guint i = 0; i < device->n_ports; i++)
    {
      copy->ports[i].name = g_strdup (device->ports[i].name);
      copy->ports[i].description = g_strdup (device->ports[i].description);
    }
}

gboolean
sd_device_equal (const SdDevice *a, const SdDevice *b)
{
  if (g_strcmp0 (a->name, b->name) != 0 || g_strcmp0 (a->description, b->description) != 0
      || a->form != b->form || a->virtual_device != b->virtual_device
      || a->volume != b->volume || a->muted != b->muted || a->n_ports != b->n_ports
      || g_strcmp0 (a->active_port, b->active_port) != 0
      || g_strcmp0 (a->active_port_description, b->active_port_description) != 0)
    return FALSE;
  for (guint i = 0; i < a->n_ports; i++)
    if (g_strcmp0 (a->ports[i].name, b->ports[i].name) != 0
        || g_strcmp0 (a->ports[i].description, b->ports[i].description) != 0)
      return FALSE;
  return TRUE;
}

static gboolean
devices_equal (const SdDevice *a, guint n_a, const SdDevice *b, guint n_b)
{
  if (n_a != n_b)
    return FALSE;
  for (guint i = 0; i < n_a; i++)
    if (!sd_device_equal (&a[i], &b[i]))
      return FALSE;
  return TRUE;
}

static void
devices_free (SdDevice **devices, guint *n_devices)
{
  for (guint i = 0; i < *n_devices; i++)
    sd_device_clear (&(*devices)[i]);
  g_clear_pointer (devices, g_free);
  *n_devices = 0;
}

const SdDevice *
sd_status_devices (const SdStatus *status, SdSide side, guint *n_devices)
{
  *n_devices = side == SD_OUTPUT ? status->n_outputs : status->n_inputs;
  return side == SD_OUTPUT ? status->outputs : status->inputs;
}

const SdDevice *
sd_status_default_device (const SdStatus *status, SdSide side)
{
  const char *name = side == SD_OUTPUT ? status->default_output : status->default_input;
  guint n;
  const SdDevice *devices = sd_status_devices (status, side, &n);

  if (name == NULL)
    return NULL;
  for (guint i = 0; i < n; i++)
    if (g_strcmp0 (devices[i].name, name) == 0)
      return &devices[i];
  return NULL;
}

SdForm
sd_state_device_form (SdSide side, const char *form_factor, const char *port_name, int port_type)
{
  if (side == SD_INPUT)
    return SD_FORM_MICROPHONE;
  if (g_strcmp0 (form_factor, "headset") == 0 || g_strcmp0 (form_factor, "headphone") == 0
      || g_strcmp0 (form_factor, "hands-free") == 0 || port_type == PA_DEVICE_PORT_TYPE_HEADPHONES
      || port_type == PA_DEVICE_PORT_TYPE_HEADSET)
    return SD_FORM_HEADPHONES;
  if (g_strcmp0 (form_factor, "tv") == 0 || port_type == PA_DEVICE_PORT_TYPE_HDMI
      || port_type == PA_DEVICE_PORT_TYPE_TV)
    return SD_FORM_DISPLAY;
  /* A server too old to give the port's type: ALSA's names say it. */
  if (port_type == PA_DEVICE_PORT_TYPE_UNKNOWN && port_name != NULL)
    {
      if (strstr (port_name, "headphones") != NULL)
        return SD_FORM_HEADPHONES;
      if (strstr (port_name, "hdmi") != NULL || strstr (port_name, "displayport") != NULL)
        return SD_FORM_DISPLAY;
    }
  return SD_FORM_SPEAKER;
}

SdPort *
sd_state_plugged_ports (const SdPortInfo *ports, guint n_ports, guint *n_plugged)
{
  GArray *plugged = g_array_new (FALSE, TRUE, sizeof (SdPort));

  for (guint i = 0; i < n_ports; i++)
    {
      SdPort port;

      if (ports[i].unplugged)
        continue;
      port.name = g_strdup (ports[i].name);
      port.description = g_strdup (ports[i].description ?: ports[i].name);
      g_array_append_val (plugged, port);
    }
  *n_plugged = plugged->len;
  if (plugged->len == 0)
    {
      g_array_free (plugged, TRUE);
      return NULL;
    }
  return (SdPort *) g_array_free (plugged, FALSE);
}

const char *
sd_state_device_detail (const SdDevice *device)
{
  if (device->virtual_device)
    return "Virtual device";
  return device->active_port_description;
}

const char *const *
sd_state_device_icon_names (const SdDevice *device)
{
  static const char *const names[][3] = {
    [SD_FORM_SPEAKER] = { "audio-speakers", "audio-speakers-symbolic", NULL },
    [SD_FORM_HEADPHONES] = { "audio-headphones", "audio-headphones-symbolic", NULL },
    [SD_FORM_DISPLAY] = { "video-display", "video-display-symbolic", NULL },
    [SD_FORM_MICROPHONE] = { "audio-input-microphone", "audio-input-microphone-symbolic", NULL },
  };

  return names[device->form];
}

/* 0 for muted or at 0, then 1 to 3 for the three steps of the level. */
static guint
level_of (const SdDevice *device)
{
  guint percent;

  if (device == NULL || device->muted)
    return 0;
  percent = sd_state_percent (device->volume);
  if (percent == 0)
    return 0;
  if (percent <= 33)
    return 1;
  if (percent <= 66)
    return 2;
  return 3;
}

const char *const *
sd_state_mute_icon_names (SdSide side, const SdDevice *device)
{
  static const char *const speaker[4][3] = {
    { "audio-volume-muted", "audio-volume-muted-symbolic", NULL },
    { "audio-volume-low", "audio-volume-low-symbolic", NULL },
    { "audio-volume-medium", "audio-volume-medium-symbolic", NULL },
    { "audio-volume-high", "audio-volume-high-symbolic", NULL },
  };
  static const char *const microphone[4][3] = {
    { "microphone-sensitivity-muted", "microphone-sensitivity-muted-symbolic", NULL },
    { "microphone-sensitivity-low", "microphone-sensitivity-low-symbolic", NULL },
    { "microphone-sensitivity-medium", "microphone-sensitivity-medium-symbolic", NULL },
    { "microphone-sensitivity-high", "microphone-sensitivity-high-symbolic", NULL },
  };

  if (side == SD_INPUT)
    return microphone[level_of (device)];
  if (device != NULL && device->form != SD_FORM_SPEAKER)
    return sd_state_device_icon_names (device);
  return speaker[level_of (device)];
}

gboolean
sd_state_mute_struck (SdSide side, const SdDevice *device)
{
  return side == SD_OUTPUT && device != NULL && device->form != SD_FORM_SPEAKER && device->muted;
}

pa_volume_t
sd_state_volume_of_percent (double percent)
{
  return (pa_volume_t) (MAX (percent, 0) * PA_VOLUME_NORM / 100 + 0.5);
}

int
sd_state_popup_room (const SdPlace *place)
{
  const SdRect *area = &place->workarea;
  int edge;

  /* It slides along a vertical panel, so it may take the work area's height. */
  if (place->sideways)
    return area->height;
  if (!place->opens_up)
    {
      if (place->panel_known)
        edge = place->panel.y + place->panel.height;
      /* GDK3 on Wayland has no work area, and no client learns where the panel's layer
         surface is: at the monitor's edge, as thick as its window. */
      else if (area->height == place->geometry.height)
        edge = place->geometry.y + place->panel_thickness;
      else
        edge = area->y;
      return MAX (0, area->y + area->height - edge);
    }
  if (place->panel_known)
    edge = place->panel.y;
  else if (area->height == place->geometry.height)
    edge = place->geometry.y + place->geometry.height - place->panel_thickness;
  else
    edge = area->y + area->height;
  return MAX (0, edge - area->y);
}

SdApp *
sd_state_apps (const SdStream *streams, guint n_streams, SdSide side, const char *default_device,
               guint *n_apps)
{
  /* Each application's index in apps, by its identity. */
  g_autoptr (GHashTable) seen = g_hash_table_new (g_str_hash, g_str_equal);
  g_autoptr (GPtrArray) members = g_ptr_array_new_with_free_func ((GDestroyNotify) g_array_unref);
  GArray *apps = g_array_new (FALSE, TRUE, sizeof (SdApp));

  for (guint i = 0; i < n_streams; i++)
    {
      const SdStream *stream = &streams[i];
      const char *identity = sd_state_stream_identity (stream);
      gpointer position;
      SdApp *app;
      SdStreamVolume member;

      if (identity == NULL)
        continue;
      if (!g_hash_table_lookup_extended (seen, identity, NULL, &position))
        {
          SdApp added = { 0 };

          added.id = g_strdup (identity);
          added.name = g_strdup (stream_app_name (stream));
          added.icon = g_strdup (nonempty (stream->icon));
          added.side = side;
          position = GUINT_TO_POINTER (apps->len);
          g_array_append_val (apps, added);
          g_ptr_array_add (members, g_array_new (FALSE, TRUE, sizeof (SdStreamVolume)));
          g_hash_table_insert (seen, (char *) identity, position);
        }
      app = &g_array_index (apps, SdApp, GPOINTER_TO_UINT (position));
      member = (SdStreamVolume) { stream->index, g_strdup (stream->title), stream->volume };
      g_array_append_val (g_ptr_array_index (members, GPOINTER_TO_UINT (position)), member);
      app->volume = MAX (app->volume, stream->volume);
      if (app->device == NULL && g_strcmp0 (stream->device, default_device) != 0)
        {
          app->device = g_strdup (stream->device);
          app->device_description = g_strdup (stream->device_description ?: stream->device);
        }
    }
  for (guint i = 0; i < apps->len; i++)
    {
      SdApp *app = &g_array_index (apps, SdApp, i);
      GArray *of_app = g_ptr_array_index (members, i);

      app->n_streams = of_app->len;
      app->streams = (SdStreamVolume *) g_array_free (g_array_ref (of_app), FALSE);
    }
  *n_apps = apps->len;
  if (apps->len == 0)
    {
      g_array_free (apps, TRUE);
      return NULL;
    }
  return (SdApp *) g_array_free (apps, FALSE);
}

pa_volume_t
sd_state_scaled_volume (pa_volume_t stream, pa_volume_t loudest, pa_volume_t volume)
{
  if (loudest == 0)
    return volume;
  return (pa_volume_t) (((guint64) stream * volume + loudest / 2) / loudest);
}

void
sd_app_clear (SdApp *app)
{
  for (guint i = 0; i < app->n_streams; i++)
    g_free (app->streams[i].title);
  g_clear_pointer (&app->streams, g_free);
  app->n_streams = 0;
  g_clear_pointer (&app->id, g_free);
  g_clear_pointer (&app->name, g_free);
  g_clear_pointer (&app->icon, g_free);
  g_clear_pointer (&app->device, g_free);
  g_clear_pointer (&app->device_description, g_free);
}

gboolean
sd_app_equal (const SdApp *a, const SdApp *b)
{
  if (g_strcmp0 (a->id, b->id) != 0 || g_strcmp0 (a->name, b->name) != 0
      || g_strcmp0 (a->icon, b->icon) != 0 || a->side != b->side
      || g_strcmp0 (a->device, b->device) != 0
      || g_strcmp0 (a->device_description, b->device_description) != 0 || a->volume != b->volume
      || a->n_streams != b->n_streams)
    return FALSE;
  for (guint i = 0; i < a->n_streams; i++)
    if (a->streams[i].index != b->streams[i].index
        || g_strcmp0 (a->streams[i].title, b->streams[i].title) != 0
        || a->streams[i].volume != b->streams[i].volume)
      return FALSE;
  return TRUE;
}

static gboolean
apps_equal (const SdApp *a, guint n_a, const SdApp *b, guint n_b)
{
  if (n_a != n_b)
    return FALSE;
  for (guint i = 0; i < n_a; i++)
    if (!sd_app_equal (&a[i], &b[i]))
      return FALSE;
  return TRUE;
}

static void
apps_free (SdApp **apps, guint *n_apps)
{
  for (guint i = 0; i < *n_apps; i++)
    sd_app_clear (&(*apps)[i]);
  g_clear_pointer (apps, g_free);
  *n_apps = 0;
}

const SdApp *
sd_status_app (const SdStatus *status, SdSide side, const char *id)
{
  for (guint i = 0; i < status->n_apps; i++)
    if (status->apps[i].side == side && g_strcmp0 (status->apps[i].id, id) == 0)
      return &status->apps[i];
  return NULL;
}
