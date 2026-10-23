/*
 * The sound server through libpulse (CMN-213). See sd-pulse.h.
 *
 * Each event re-reads its facility's whole list, which keeps removals simple, and the status
 * is built again once no list is on its way. A list asked for again before the last one came
 * in replaces it when it comes: the server answers in order.
 */

#include "sd-pulse.h"

#include <pulse/glib-mainloop.h>
#include <pulse/pulseaudio.h>
#include <string.h>

/* The volume controls' own monitor streams, which aren't an application recording. */
static const char *const volume_controls[] = {
  "org.mate.VolumeControl", "org.gnome.VolumeControl", "org.PulseAudio.pavucontrol", NULL,
};

typedef enum
{
  LIST_SINKS,
  LIST_SOURCES,
  LIST_RECORDERS,
  LIST_CARDS,
  N_LISTS,
} ListKind;

/* A sink or a source. */
typedef struct
{
  char      *name;
  char      *description;
  pa_cvolume volume;
  gboolean   muted;
  gboolean   headphones;
  /* A source that is an output's monitor. */
  gboolean   monitor;
} Device;

/* A source output, which is a stream recording. */
typedef struct
{
  char    *app_id;
  char    *name;
  char    *binary;
  gboolean corked;
  /* The reader's own, or a volume control's. */
  gboolean ignored;
} Recorder;

typedef struct
{
  SdPulse    *pulse;
  pa_context *context;
  ListKind    kind;
  GPtrArray  *items;
} Request;

struct _SdPulse
{
  SdPulseChanged    changed;
  gpointer          data;
  pa_glib_mainloop *mainloop;
  pa_context       *context;
  guint             reconnect_id;
  /* The server's answer to its info has come since the context was ready. */
  gboolean          ready;
  char             *default_sink;
  char             *default_source;
  /* Of Device, Device, Recorder and SdCard, as the server last listed them. */
  GPtrArray        *lists[N_LISTS];
  /* The lists on their way, as Request. */
  GList            *requests;
  SdStatus          status;
};

static void connect_to_server (SdPulse *pulse);

static void
device_free (gpointer data)
{
  Device *device = data;

  g_free (device->name);
  g_free (device->description);
  g_free (device);
}

static void
recorder_free (gpointer data)
{
  Recorder *recorder = data;

  g_free (recorder->app_id);
  g_free (recorder->name);
  g_free (recorder->binary);
  g_free (recorder);
}

static void
card_free (gpointer data)
{
  SdStatus status = { .cards = data, .n_cards = 1 };

  /* Frees the SdCard's own memory too. */
  sd_status_clear (&status);
}

static GPtrArray *
list_new (ListKind kind)
{
  switch (kind)
    {
    case LIST_SINKS:
    case LIST_SOURCES:
      return g_ptr_array_new_with_free_func (device_free);
    case LIST_RECORDERS:
      return g_ptr_array_new_with_free_func (recorder_free);
    case LIST_CARDS:
    case N_LISTS:
      break;
    }
  return g_ptr_array_new_with_free_func (card_free);
}

static void
request_free (Request *request)
{
  g_ptr_array_unref (request->items);
  g_free (request);
}

static Device *
find_device (GPtrArray *devices, const char *name)
{
  if (name == NULL)
    return NULL;
  for (guint i = 0; i < devices->len; i++)
    {
      Device *device = g_ptr_array_index (devices, i);

      if (g_strcmp0 (device->name, name) == 0)
        return device;
    }
  return NULL;
}

static void
build_status (SdPulse *pulse, SdStatus *status)
{
  Device *sink = find_device (pulse->lists[LIST_SINKS], pulse->default_sink);
  Device *source = find_device (pulse->lists[LIST_SOURCES], pulse->default_source);
  GPtrArray *recorders = pulse->lists[LIST_RECORDERS];
  GPtrArray *cards = pulse->lists[LIST_CARDS];
  g_autoptr (GArray) streams = g_array_new (FALSE, FALSE, sizeof (SdStream));
  SdStatus cards_status;

  status->running = TRUE;
  if (sink != NULL)
    {
      status->has_output = TRUE;
      status->output_name = g_strdup (sink->description);
      status->volume = pa_cvolume_max (&sink->volume);
      status->muted = sink->muted;
      status->headphones = sink->headphones;
    }
  if (source != NULL && !source->monitor)
    {
      status->has_input = TRUE;
      status->input_muted = source->muted;
    }
  for (guint i = 0; i < recorders->len; i++)
    {
      Recorder *recorder = g_ptr_array_index (recorders, i);
      SdStream stream = { recorder->app_id, recorder->name, recorder->binary };

      if (!recorder->corked && !recorder->ignored)
        g_array_append_val (streams, stream);
    }
  status->recording = sd_state_recording_names ((SdStream *) streams->data, streams->len);
  /* The SdCards of the list, in a status of their own to copy them from. */
  cards_status = (SdStatus) { .n_cards = cards->len, .cards = g_new (SdCard, cards->len) };
  for (guint i = 0; i < cards->len; i++)
    cards_status.cards[i] = *(SdCard *) g_ptr_array_index (cards, i);
  sd_cards_copy (&cards_status, status);
  g_free (cards_status.cards);
}

static void
set_status (SdPulse *pulse, SdStatus *status)
{
  if (sd_status_equal (&pulse->status, status))
    {
      sd_status_clear (status);
      return;
    }
  sd_status_clear (&pulse->status);
  pulse->status = *status;
  pulse->changed (pulse, pulse->data);
}

/* Once every list has come in. */
static void
update (SdPulse *pulse)
{
  SdStatus status = { 0 };

  if (!pulse->ready || pulse->requests != NULL)
    return;
  build_status (pulse, &status);
  set_status (pulse, &status);
}

static void
request_done (Request *request, int eol)
{
  SdPulse *pulse = request->pulse;

  pulse->requests = g_list_remove (pulse->requests, request);
  if (eol > 0)
    {
      g_ptr_array_unref (pulse->lists[request->kind]);
      pulse->lists[request->kind] = g_ptr_array_ref (request->items);
    }
  else
    g_warning ("can't list the sound server's devices: %s",
               pa_strerror (pa_context_errno (request->context)));
  request_free (request);
  update (pulse);
}

static const char *
property (pa_proplist *properties, const char *key)
{
  const char *value = pa_proplist_gets (properties, key);

  return value != NULL && value[0] != '\0' ? value : NULL;
}

/* A headset, or a device on its Headphones port. */
static gboolean
is_headphones (pa_proplist *properties, const char *port_name, pa_device_port_type_t port_type)
{
  const char *form = property (properties, PA_PROP_DEVICE_FORM_FACTOR);

  if (g_strcmp0 (form, "headset") == 0 || g_strcmp0 (form, "headphone") == 0
      || g_strcmp0 (form, "hands-free") == 0)
    return TRUE;
  if (port_type == PA_DEVICE_PORT_TYPE_HEADPHONES || port_type == PA_DEVICE_PORT_TYPE_HEADSET)
    return TRUE;
  /* A server too old to give the port's type: ALSA's names it so. */
  return port_type == PA_DEVICE_PORT_TYPE_UNKNOWN && port_name != NULL
         && strstr (port_name, "headphones") != NULL;
}

static Device *
device_new (const char *name, const char *description, const pa_cvolume *volume, int mute)
{
  Device *device = g_new0 (Device, 1);

  device->name = g_strdup (name);
  device->description = g_strdup (description != NULL ? description : name);
  device->volume = *volume;
  device->muted = mute != 0;
  return device;
}

static void
on_sink (pa_context *context G_GNUC_UNUSED, const pa_sink_info *info, int eol, void *data)
{
  Request *request = data;
  Device *device;

  if (eol != 0)
    {
      request_done (request, eol);
      return;
    }
  device = device_new (info->name, info->description, &info->volume, info->mute);
  device->headphones = is_headphones (info->proplist,
                                      info->active_port != NULL ? info->active_port->name : NULL,
                                      info->active_port != NULL ? info->active_port->type
                                                                : PA_DEVICE_PORT_TYPE_UNKNOWN);
  g_ptr_array_add (request->items, device);
}

static void
on_source (pa_context *context G_GNUC_UNUSED, const pa_source_info *info, int eol, void *data)
{
  Request *request = data;
  Device *device;

  if (eol != 0)
    {
      request_done (request, eol);
      return;
    }
  device = device_new (info->name, info->description, &info->volume, info->mute);
  device->monitor = info->monitor_of_sink != PA_INVALID_INDEX;
  g_ptr_array_add (request->items, device);
}

static void
on_recorder (pa_context *context, const pa_source_output_info *info, int eol, void *data)
{
  Request *request = data;
  Recorder *recorder;
  const char *app_id;

  if (eol != 0)
    {
      request_done (request, eol);
      return;
    }
  recorder = g_new0 (Recorder, 1);
  app_id = property (info->proplist, PA_PROP_APPLICATION_ID);
  /* PipeWire's for a Flatpak's client. */
  recorder->app_id = g_strdup (property (info->proplist, "pipewire.sec.app-id"));
  recorder->name = g_strdup (property (info->proplist, PA_PROP_APPLICATION_NAME));
  recorder->binary = g_strdup (property (info->proplist, PA_PROP_APPLICATION_PROCESS_BINARY));
  recorder->corked = info->corked != 0;
  recorder->ignored = info->client == pa_context_get_index (context)
                      || (app_id != NULL && g_strv_contains (volume_controls, app_id));
  g_ptr_array_add (request->items, recorder);
}

static void
on_card (pa_context *context G_GNUC_UNUSED, const pa_card_info *info, int eol, void *data)
{
  Request *request = data;
  SdCard *card;
  GArray *profiles;

  if (eol != 0)
    {
      request_done (request, eol);
      return;
    }
  card = g_new0 (SdCard, 1);
  card->index = info->index;
  card->description = g_strdup (property (info->proplist, PA_PROP_DEVICE_DESCRIPTION) ?: info->name);
  if (info->active_profile2 != NULL)
    card->active = g_strdup (info->active_profile2->name);
  profiles = g_array_new (FALSE, TRUE, sizeof (SdProfile));
  for (guint32 i = 0; i < info->n_profiles; i++)
    {
      const pa_card_profile_info2 *profile = info->profiles2[i];
      SdProfile copy;

      /* The active one even when it isn't, so that the menu has it checked. */
      if (!profile->available && info->active_profile2 != profile)
        continue;
      copy.name = g_strdup (profile->name);
      copy.description = g_strdup (profile->description);
      g_array_append_val (profiles, copy);
    }
  card->n_profiles = profiles->len;
  card->profiles = (SdProfile *) g_array_free (profiles, FALSE);
  g_ptr_array_add (request->items, card);
}

static void
request_list (SdPulse *pulse, ListKind kind)
{
  Request *request = g_new0 (Request, 1);
  pa_operation *operation = NULL;

  request->pulse = pulse;
  request->context = pulse->context;
  request->kind = kind;
  request->items = list_new (kind);
  switch (kind)
    {
    case LIST_SINKS:
      operation = pa_context_get_sink_info_list (pulse->context, on_sink, request);
      break;
    case LIST_SOURCES:
      operation = pa_context_get_source_info_list (pulse->context, on_source, request);
      break;
    case LIST_RECORDERS:
      operation = pa_context_get_source_output_info_list (pulse->context, on_recorder, request);
      break;
    case LIST_CARDS:
      operation = pa_context_get_card_info_list (pulse->context, on_card, request);
      break;
    case N_LISTS:
      break;
    }
  if (operation == NULL)
    {
      request_free (request);
      return;
    }
  pa_operation_unref (operation);
  pulse->requests = g_list_prepend (pulse->requests, request);
}

static void
on_server_info (pa_context *context, const pa_server_info *info, void *data)
{
  SdPulse *pulse = data;

  if (context != pulse->context || info == NULL)
    return;
  g_free (pulse->default_sink);
  g_free (pulse->default_source);
  pulse->default_sink = g_strdup (info->default_sink_name);
  pulse->default_source = g_strdup (info->default_source_name);
  pulse->ready = TRUE;
  update (pulse);
}

static void
request_server_info (SdPulse *pulse)
{
  pa_operation *operation = pa_context_get_server_info (pulse->context, on_server_info, pulse);

  if (operation != NULL)
    pa_operation_unref (operation);
}

static void
on_event (pa_context *context, pa_subscription_event_type_t type, uint32_t index G_GNUC_UNUSED,
          void *data)
{
  SdPulse *pulse = data;

  if (context != pulse->context)
    return;
  switch (type & PA_SUBSCRIPTION_EVENT_FACILITY_MASK)
    {
    case PA_SUBSCRIPTION_EVENT_SINK:
      request_list (pulse, LIST_SINKS);
      break;
    case PA_SUBSCRIPTION_EVENT_SOURCE:
      request_list (pulse, LIST_SOURCES);
      break;
    case PA_SUBSCRIPTION_EVENT_SOURCE_OUTPUT:
      request_list (pulse, LIST_RECORDERS);
      break;
    case PA_SUBSCRIPTION_EVENT_CARD:
      request_list (pulse, LIST_CARDS);
      break;
    case PA_SUBSCRIPTION_EVENT_SERVER:
      request_server_info (pulse);
      break;
    default:
      /* Sink inputs: the popup's (CMN-216). */
      break;
    }
}

static void
forget_server (SdPulse *pulse)
{
  g_list_free_full (g_steal_pointer (&pulse->requests), (GDestroyNotify) request_free);
  for (guint i = 0; i < N_LISTS; i++)
    {
      g_clear_pointer (&pulse->lists[i], g_ptr_array_unref);
      pulse->lists[i] = list_new (i);
    }
  g_clear_pointer (&pulse->default_sink, g_free);
  g_clear_pointer (&pulse->default_source, g_free);
  pulse->ready = FALSE;
}

static void
disconnect (SdPulse *pulse)
{
  if (pulse->context == NULL)
    return;
  pa_context_set_state_callback (pulse->context, NULL, NULL);
  pa_context_set_subscribe_callback (pulse->context, NULL, NULL);
  pa_context_disconnect (pulse->context);
  /* Cancels the operations on their way, whose callbacks don't run: their Requests go
     next. */
  g_clear_pointer (&pulse->context, pa_context_unref);
  forget_server (pulse);
}

static gboolean
on_reconnect (gpointer data)
{
  SdPulse *pulse = data;

  pulse->reconnect_id = 0;
  connect_to_server (pulse);
  return G_SOURCE_REMOVE;
}

static void
on_state (pa_context *context, void *data)
{
  SdPulse *pulse = data;
  SdStatus none = { 0 };
  pa_operation *operation;

  switch (pa_context_get_state (context))
    {
    case PA_CONTEXT_READY:
      pa_context_set_subscribe_callback (context, on_event, pulse);
      operation = pa_context_subscribe (context,
                                        PA_SUBSCRIPTION_MASK_SINK | PA_SUBSCRIPTION_MASK_SOURCE
                                        | PA_SUBSCRIPTION_MASK_SINK_INPUT
                                        | PA_SUBSCRIPTION_MASK_SOURCE_OUTPUT
                                        | PA_SUBSCRIPTION_MASK_CARD
                                        | PA_SUBSCRIPTION_MASK_SERVER,
                                        NULL, NULL);
      if (operation != NULL)
        pa_operation_unref (operation);
      for (guint i = 0; i < N_LISTS; i++)
        request_list (pulse, i);
      request_server_info (pulse);
      break;
    case PA_CONTEXT_FAILED:
    case PA_CONTEXT_TERMINATED:
      /* Not from inside the context's own callback, which unreffing it would end. With
         PA_CONTEXT_NOFAIL the new one waits for the server to come back; the second keeps
         a context that fails at once, with no D-Bus to wait on, from spinning. */
      if (pulse->reconnect_id == 0)
        pulse->reconnect_id = g_timeout_add_seconds (1, on_reconnect, pulse);
      set_status (pulse, &none);
      break;
    default:
      break;
    }
}

static void
connect_to_server (SdPulse *pulse)
{
  pa_proplist *properties = pa_proplist_new ();

  disconnect (pulse);
  pa_proplist_sets (properties, PA_PROP_APPLICATION_NAME, "Sound applet");
  pa_proplist_sets (properties, PA_PROP_APPLICATION_ID, "org.mate.panel.CaminitoSoundApplet");
  pa_proplist_sets (properties, PA_PROP_APPLICATION_ICON_NAME, "audio-volume-high");
  pulse->context = pa_context_new_with_proplist (pa_glib_mainloop_get_api (pulse->mainloop),
                                                 NULL, properties);
  pa_proplist_free (properties);
  pa_context_set_state_callback (pulse->context, on_state, pulse);
  /* NOFAIL: with no server yet, it waits for one rather than failing. No autospawn: a
     panel applet doesn't start PulseAudio. */
  if (pa_context_connect (pulse->context, NULL, PA_CONTEXT_NOFAIL | PA_CONTEXT_NOAUTOSPAWN,
                          NULL)
      < 0)
    {
      g_warning ("can't connect to the sound server: %s",
                 pa_strerror (pa_context_errno (pulse->context)));
      if (pulse->reconnect_id == 0)
        pulse->reconnect_id = g_timeout_add_seconds (5, on_reconnect, pulse);
    }
}

SdPulse *
sd_pulse_new (SdPulseChanged changed, gpointer data)
{
  SdPulse *pulse = g_new0 (SdPulse, 1);

  pulse->changed = changed;
  pulse->data = data;
  pulse->mainloop = pa_glib_mainloop_new (NULL);
  forget_server (pulse);
  connect_to_server (pulse);
  return pulse;
}

const SdStatus *
sd_pulse_status (SdPulse *pulse)
{
  return &pulse->status;
}

static void
done (pa_operation *operation)
{
  if (operation != NULL)
    pa_operation_unref (operation);
}

static gboolean
connected (SdPulse *pulse)
{
  return pulse->context != NULL && pulse->ready;
}

void
sd_pulse_set_volume (SdPulse *pulse, pa_volume_t volume)
{
  Device *sink;
  pa_cvolume channels;

  if (!connected (pulse))
    return;
  sink = find_device (pulse->lists[LIST_SINKS], pulse->default_sink);
  if (sink == NULL)
    return;
  channels = sink->volume;
  /* Keeps the balance. */
  pa_cvolume_scale (&channels, volume);
  done (pa_context_set_sink_volume_by_name (pulse->context, sink->name, &channels, NULL, NULL));
}

void
sd_pulse_set_muted (SdPulse *pulse, gboolean muted)
{
  if (!connected (pulse) || pulse->default_sink == NULL)
    return;
  done (pa_context_set_sink_mute_by_name (pulse->context, pulse->default_sink, muted, NULL,
                                          NULL));
}

void
sd_pulse_set_input_muted (SdPulse *pulse, gboolean muted)
{
  if (!connected (pulse) || pulse->default_source == NULL)
    return;
  done (pa_context_set_source_mute_by_name (pulse->context, pulse->default_source, muted, NULL,
                                            NULL));
}

void
sd_pulse_set_profile (SdPulse *pulse, guint32 card, const char *profile)
{
  if (!connected (pulse))
    return;
  done (pa_context_set_card_profile_by_index (pulse->context, card, profile, NULL, NULL));
}

void
sd_pulse_free (SdPulse *pulse)
{
  g_clear_handle_id (&pulse->reconnect_id, g_source_remove);
  disconnect (pulse);
  for (guint i = 0; i < N_LISTS; i++)
    g_clear_pointer (&pulse->lists[i], g_ptr_array_unref);
  pa_glib_mainloop_free (pulse->mainloop);
  sd_status_clear (&pulse->status);
  g_free (pulse);
}
