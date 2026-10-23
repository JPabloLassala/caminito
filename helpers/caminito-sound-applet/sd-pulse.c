/*
 * The sound server through libpulse (CMN-213, CMN-215, CMN-216, CMN-217). See sd-pulse.h.
 *
 * Each event re-reads its facility's whole list, which keeps removals simple, and the status
 * is built again once no list is on its way. A list asked for again before the last one came
 * in replaces it when it comes: the server answers in order.
 *
 * Pins are applied twice (ADR 0045). A new stream's own info is asked for from its `new`
 * event, ahead of its facility's list, and the stream is moved from that answer: the move
 * lands before WirePlumber links the stream, about 4 ms after the event, so it never plays on
 * the default. And each time the status is built, any pinned application's stream that isn't
 * on its device while the device is there is moved: a device that comes back, a pin made or
 * changed, a stream that was there before the reader.
 */

#include "sd-pulse.h"

#include <gio/gio.h>
#include <pulse/ext-stream-restore.h>
#include <pulse/glib-mainloop.h>
#include <pulse/pulseaudio.h>
#include <stdio.h>
#include <string.h>

/* The volume controls' own monitor streams, which aren't an application recording. */
static const char *const volume_controls[] = {
  "org.mate.VolumeControl", "org.gnome.VolumeControl", "org.PulseAudio.pavucontrol", NULL,
};

/* System sounds' entry in the stream-restore extension, as pavucontrol names it. */
#define EVENT_ROLE "sink-input-by-media-role:event"

typedef enum
{
  LIST_SINKS,
  LIST_SOURCES,
  LIST_PLAYING,
  LIST_RECORDING,
  LIST_CARDS,
  LIST_SYSTEM_SOUNDS,
  N_LISTS,
} ListKind;

/* A sink or a source. */
typedef struct
{
  SdDevice   device;
  guint32    index;
  /* Every channel's, which a volume set keeps the balance of. */
  pa_cvolume volume;
  /* A source that is an output's monitor. */
  gboolean   monitor;
  /* A sink's monitor, which its streams' peaks are heard on. */
  guint32    monitor_source;
} Device;

/* A Stream: one of the server's sink inputs or source outputs. */
typedef struct
{
  char      *app_id;
  char      *name;
  char      *binary;
  char      *icon;
  char      *title;
  guint32    index;
  /* The sink's or the source's index. */
  guint32    device;
  /* Every channel's, none where it has no volume. */
  pa_cvolume volume;
  gboolean   corked;
  /* The reader's own, or a volume control's. */
  gboolean   ignored;
  /* An event sound's, which System sounds' row has rather than an application's. */
  gboolean   event;
} Stream;

/* System sounds' entry, where the server has one. */
typedef struct
{
  pa_cvolume volume;
  char      *device;
  gboolean   mute;
} SystemSounds;

/* For the warnings. */
static const char *const list_names[N_LISTS] = {
  "outputs", "inputs", "playing streams", "recording streams", "cards", "system sounds' volume",
};

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
  /* Of Device, Device, Stream, Stream, SdCard and SystemSounds, as the server last listed
     them, and whether it has listed each since the context was ready. */
  GPtrArray        *lists[N_LISTS];
  gboolean          listed[N_LISTS];
  /* The lists on their way, as Request. */
  GList            *requests;
  SdStatus          status;
  /* Who hears the Default input, and the peak stream on it, while someone does. */
  SdPulseLevel      level;
  gpointer          level_data;
  pa_stream        *level_stream;
  char             *level_source;
  /* Who hears the applications, and the peak streams on their streams while someone does,
     as AppMonitor by "<side>:<stream>:<device>": a stream moved gets a new one. One that
     couldn't be opened has no stream, and isn't tried again. */
  SdPulseAppLevel   app_level;
  gpointer          app_level_data;
  GHashTable       *monitors;
  /* Each stream's share of its application's loudest when the application's slider went to
     0, as a double by "<side>:<stream>", which silent streams can't tell. */
  GHashTable       *shares;
  /* The pins, as SdPin (CMN-217), and the moves to a pinned device on their way, as the
     device's index plus one by "<side>:<stream>": a move isn't asked for twice, and one the
     server refuses isn't asked for again. */
  GArray           *pins;
  GHashTable       *moving;
  /* WirePlumber's target restore was turned off on this connection. */
  gboolean          restore_target_off;
};

/* A peak stream on one of an application's streams. */
typedef struct
{
  SdPulse   *pulse;
  SdSide     side;
  char      *app;
  pa_stream *stream;
  /* Its last peak. */
  double     level;
} AppMonitor;

/* As pavucontrol's peak meters, which pipewire-pulse delivers about 18 of (CMN-210). */
#define PEAKS_PER_SECOND 25

static void connect_to_server (SdPulse *pulse);

/* An operation whose end the reader doesn't wait for: the server's events tell it what
   changed. NULL where the context couldn't send it. */
static void
drop_operation (pa_operation *operation)
{
  if (operation != NULL)
    pa_operation_unref (operation);
}

static void
device_free (gpointer data)
{
  Device *device = data;

  sd_device_clear (&device->device);
  g_free (device);
}

static void
stream_free (gpointer data)
{
  Stream *stream = data;

  g_free (stream->app_id);
  g_free (stream->name);
  g_free (stream->binary);
  g_free (stream->icon);
  g_free (stream->title);
  g_free (stream);
}

static void
system_sounds_free (gpointer data)
{
  SystemSounds *entry = data;

  g_free (entry->device);
  g_free (entry);
}

static void
card_free (gpointer data)
{
  sd_card_clear (data);
  g_free (data);
}

static GPtrArray *
list_new (ListKind kind)
{
  switch (kind)
    {
    case LIST_SINKS:
    case LIST_SOURCES:
      return g_ptr_array_new_with_free_func (device_free);
    case LIST_PLAYING:
    case LIST_RECORDING:
      return g_ptr_array_new_with_free_func (stream_free);
    case LIST_SYSTEM_SOUNDS:
      return g_ptr_array_new_with_free_func (system_sounds_free);
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

      if (g_strcmp0 (device->device.name, name) == 0)
        return device;
    }
  return NULL;
}

static Device *
find_device_by_index (GPtrArray *devices, guint32 index)
{
  for (guint i = 0; i < devices->len; i++)
    {
      Device *device = g_ptr_array_index (devices, i);

      if (device->index == index)
        return device;
    }
  return NULL;
}

static Stream *
find_stream (GPtrArray *streams, guint32 index)
{
  for (guint i = 0; i < streams->len; i++)
    {
      Stream *stream = g_ptr_array_index (streams, i);

      if (stream->index == index)
        return stream;
    }
  return NULL;
}

static GPtrArray *
devices_of (SdPulse *pulse, SdSide side)
{
  return pulse->lists[side == SD_OUTPUT ? LIST_SINKS : LIST_SOURCES];
}

static GPtrArray *
streams_of (SdPulse *pulse, SdSide side)
{
  return pulse->lists[side == SD_OUTPUT ? LIST_PLAYING : LIST_RECORDING];
}

/* A stream of a Sound application now: playing or recording, and not the reader's own, a
   volume control's or an event sound. */
static gboolean
stream_listed (const Stream *stream)
{
  return !stream->corked && !stream->ignored && !stream->event;
}

/* The stream as sd-state takes it. */
static SdStream
stream_named (const Stream *stream, GPtrArray *devices)
{
  Device *device = find_device_by_index (devices, stream->device);

  return (SdStream) {
    .app_id = stream->app_id,
    .name = stream->name,
    .binary = stream->binary,
    .icon = stream->icon,
    .title = stream->title,
    .index = stream->index,
    .device = device != NULL ? device->device.name : NULL,
    .device_description = device != NULL ? device->device.description : NULL,
    .volume = pa_cvolume_max (&stream->volume),
    .corked = stream->corked,
  };
}

/* The pin of a stream's application, NULL where it isn't pinned or isn't an application's. */
static const SdPin *
pin_of (SdPulse *pulse, SdSide side, const Stream *stream)
{
  SdStream named = { .app_id = stream->app_id, .name = stream->name, .binary = stream->binary };
  const char *identity = sd_state_stream_identity (&named);
  int found;

  if (identity == NULL || stream->ignored || stream->event)
    return NULL;
  found = sd_pins_find (pulse->pins, side, identity);
  return found >= 0 ? &g_array_index (pulse->pins, SdPin, found) : NULL;
}

/* Moves a pinned application's stream to its device, where the device is on the server and
   the stream isn't on it yet. Paused, it's moved too, so that it plays there. */
static void
apply_pin (SdPulse *pulse, SdSide side, const Stream *stream)
{
  const SdPin *pin = pin_of (pulse, side, stream);
  Device *device;
  g_autofree char *key = NULL;

  if (pin == NULL)
    return;
  device = find_device (devices_of (pulse, side), pin->device);
  if (device == NULL || device->index == stream->device)
    return;
  key = g_strdup_printf ("%d:%u", side, stream->index);
  if (GPOINTER_TO_UINT (g_hash_table_lookup (pulse->moving, key)) == device->index + 1)
    return;
  if (side == SD_OUTPUT)
    drop_operation (pa_context_move_sink_input_by_index (pulse->context, stream->index,
                                                         device->index, NULL, NULL));
  else
    drop_operation (pa_context_move_source_output_by_index (pulse->context, stream->index,
                                                            device->index, NULL, NULL));
  g_hash_table_insert (pulse->moving, g_steal_pointer (&key), GUINT_TO_POINTER (device->index + 1));
}

/* Every pinned application's streams onto their devices, and the moves that are done or
   whose stream is gone forgotten. */
static void
apply_pins (SdPulse *pulse)
{
  GHashTableIter iter;
  gpointer key, value;

  g_hash_table_iter_init (&iter, pulse->moving);
  while (g_hash_table_iter_next (&iter, &key, &value))
    {
      int side;
      guint32 index;
      Stream *stream = NULL;

      if (sscanf (key, "%d:%u", &side, &index) == 2)
        stream = find_stream (streams_of (pulse, side), index);
      if (stream == NULL || stream->device + 1 == GPOINTER_TO_UINT (value))
        g_hash_table_iter_remove (&iter);
    }
  for (SdSide side = SD_OUTPUT; pulse->pins->len > 0 && side <= SD_INPUT; side++)
    {
      GPtrArray *streams = streams_of (pulse, side);

      for (guint i = 0; i < streams->len; i++)
        apply_pin (pulse, side, g_ptr_array_index (streams, i));
    }
}

/* The status's Pinned applications, in the pins' order, from every stream of theirs, paused
   ones too, and every device, monitors too: an application may record one. */
static void
add_pinned (SdPulse *pulse, SdStatus *status)
{
  status->n_pinned = pulse->pins->len;
  status->pinned = g_new0 (SdPinnedApp, pulse->pins->len);
  for (guint i = 0; i < pulse->pins->len; i++)
    {
      const SdPin *pin = &g_array_index (pulse->pins, SdPin, i);
      GPtrArray *streams = streams_of (pulse, pin->side);
      GPtrArray *devices = devices_of (pulse, pin->side);
      g_autoptr (GArray) named = g_array_new (FALSE, FALSE, sizeof (SdStream));
      g_autoptr (GArray) known = g_array_new (FALSE, FALSE, sizeof (SdDevice));

      for (guint j = 0; j < streams->len; j++)
        {
          Stream *stream = g_ptr_array_index (streams, j);
          SdStream copy = stream_named (stream, devices);

          if (!stream->ignored && !stream->event)
            g_array_append_val (named, copy);
        }
      for (guint j = 0; j < devices->len; j++)
        g_array_append_val (known, ((Device *) g_ptr_array_index (devices, j))->device);
      sd_state_pinned_app (pin, (SdStream *) named->data, named->len, (SdDevice *) known->data,
                           known->len, &status->pinned[i]);
    }
}

/* The side's Sound applications, onto apps. */
static void
add_apps (SdPulse *pulse, GArray *apps, SdSide side)
{
  GPtrArray *streams = streams_of (pulse, side);
  GPtrArray *devices = devices_of (pulse, side);
  g_autoptr (GArray) named = g_array_new (FALSE, FALSE, sizeof (SdStream));
  SdApp *found;
  guint n;

  for (guint i = 0; i < streams->len; i++)
    {
      Stream *stream = g_ptr_array_index (streams, i);
      SdStream copy;

      if (!stream_listed (stream) || pin_of (pulse, side, stream) != NULL)
        continue;
      copy = stream_named (stream, devices);
      g_array_append_val (named, copy);
    }
  found = sd_state_apps ((SdStream *) named->data, named->len, side,
                         side == SD_OUTPUT ? pulse->default_sink : pulse->default_source, &n);
  g_array_append_vals (apps, found, n);
  g_free (found);
}

/* The popup's list: every device, but the outputs' monitors. */
static SdDevice *
copy_devices (GPtrArray *devices, guint *n_copied)
{
  SdDevice *copies = g_new0 (SdDevice, devices->len);
  guint n = 0;

  for (guint i = 0; i < devices->len; i++)
    {
      Device *device = g_ptr_array_index (devices, i);

      if (!device->monitor)
        sd_device_copy (&device->device, &copies[n++]);
    }
  *n_copied = n;
  return copies;
}

static void
build_status (SdPulse *pulse, SdStatus *status)
{
  Device *sink = find_device (pulse->lists[LIST_SINKS], pulse->default_sink);
  Device *source = find_device (pulse->lists[LIST_SOURCES], pulse->default_source);
  GPtrArray *recording = pulse->lists[LIST_RECORDING];
  GPtrArray *cards = pulse->lists[LIST_CARDS];
  GPtrArray *system_sounds = pulse->lists[LIST_SYSTEM_SOUNDS];
  g_autoptr (GArray) streams = g_array_new (FALSE, FALSE, sizeof (SdStream));
  GArray *apps = g_array_new (FALSE, TRUE, sizeof (SdApp));

  status->running = TRUE;
  if (sink != NULL)
    {
      status->has_output = TRUE;
      status->output_name = g_strdup (sink->device.description);
      status->volume = sink->device.volume;
      status->muted = sink->device.muted;
      status->headphones = sink->device.form == SD_FORM_HEADPHONES;
      status->default_output = g_strdup (sink->device.name);
    }
  if (source != NULL && !source->monitor)
    {
      status->has_input = TRUE;
      status->input_muted = source->device.muted;
      status->default_input = g_strdup (source->device.name);
    }
  status->outputs = copy_devices (pulse->lists[LIST_SINKS], &status->n_outputs);
  status->inputs = copy_devices (pulse->lists[LIST_SOURCES], &status->n_inputs);
  for (guint i = 0; i < recording->len; i++)
    {
      Stream *stream = g_ptr_array_index (recording, i);
      SdStream named = stream_named (stream, pulse->lists[LIST_SOURCES]);

      if (stream_listed (stream))
        g_array_append_val (streams, named);
    }
  status->recording = sd_state_recording_names ((SdStream *) streams->data, streams->len);
  status->n_cards = cards->len;
  status->cards = g_new0 (SdCard, cards->len);
  for (guint i = 0; i < cards->len; i++)
    sd_card_copy (g_ptr_array_index (cards, i), &status->cards[i]);
  add_apps (pulse, apps, SD_OUTPUT);
  add_apps (pulse, apps, SD_INPUT);
  status->n_apps = apps->len;
  status->apps = (SdApp *) g_array_free (apps, FALSE);
  add_pinned (pulse, status);
  status->has_system_sounds = pulse->listed[LIST_SYSTEM_SOUNDS];
  /* With no entry, an event sound plays at the server's full volume. */
  status->system_volume = system_sounds->len > 0
                            ? pa_cvolume_max (&((SystemSounds *) g_ptr_array_index (system_sounds, 0))->volume)
                            : PA_VOLUME_NORM;
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

/* The loudest of the peaks a peak stream has for us, from 0 to 1; FALSE with none. */
static gboolean
read_peaks (pa_stream *stream, double *peak)
{
  float level = 0;
  gboolean read = FALSE;

  while (pa_stream_readable_size (stream) > 0)
    {
      const void *samples;
      size_t size;

      if (pa_stream_peek (stream, &samples, &size) < 0)
        return FALSE;
      /* A hole, which has a size but no samples. */
      if (samples != NULL)
        {
          for (size_t i = 0; i < size / sizeof (float); i++)
            level = MAX (level, ((const float *) samples)[i]);
          read = TRUE;
        }
      if (size > 0)
        pa_stream_drop (stream);
      else
        break;
    }
  *peak = CLAMP (level, 0, 1);
  return read;
}

static void
on_peaks (pa_stream *stream, size_t length G_GNUC_UNUSED, void *data)
{
  SdPulse *pulse = data;
  double level;

  if (read_peaks (stream, &level) && pulse->level != NULL)
    pulse->level (level, pulse->level_data);
}

/* An application's level is its loudest stream's. */
static void
on_app_peaks (pa_stream *stream, size_t length G_GNUC_UNUSED, void *data)
{
  AppMonitor *monitor = data;
  SdPulse *pulse = monitor->pulse;
  GHashTableIter iter;
  gpointer value;
  double level = 0;

  if (!read_peaks (stream, &monitor->level) || pulse->app_level == NULL)
    return;
  g_hash_table_iter_init (&iter, pulse->monitors);
  while (g_hash_table_iter_next (&iter, NULL, &value))
    {
      AppMonitor *other = value;

      if (other->side == monitor->side && g_str_equal (other->app, monitor->app))
        level = MAX (level, other->level);
    }
  pulse->app_level (monitor->side, monitor->app, level, pulse->app_level_data);
}

static void
app_monitor_free (gpointer data)
{
  AppMonitor *monitor = data;

  if (monitor->stream != NULL)
    {
      pa_stream_set_read_callback (monitor->stream, NULL, NULL);
      pa_stream_disconnect (monitor->stream);
      pa_stream_unref (monitor->stream);
    }
  g_free (monitor->app);
  g_free (monitor);
}

static void
close_level_stream (SdPulse *pulse)
{
  if (pulse->level_stream == NULL)
    return;
  pa_stream_set_read_callback (pulse->level_stream, NULL, NULL);
  pa_stream_disconnect (pulse->level_stream);
  g_clear_pointer (&pulse->level_stream, pa_stream_unref);
  g_clear_pointer (&pulse->level_source, g_free);
}

/* A peak stream on the Default input while someone hears it, as pavucontrol's
   (src/mainwindow.cc, createMonitorStreamForSource()), with its flags. */
static void
follow_input_level (SdPulse *pulse)
{
  pa_sample_spec spec = { .format = PA_SAMPLE_FLOAT32NE, .rate = PEAKS_PER_SECOND, .channels = 1 };
  pa_buffer_attr attr = { .maxlength = (uint32_t) -1, .fragsize = sizeof (float) };
  const char *source = pulse->level != NULL && pulse->ready ? pulse->status.default_input : NULL;

  if (g_strcmp0 (source, pulse->level_source) == 0)
    return;
  close_level_stream (pulse);
  if (source == NULL)
    return;
  pulse->level_stream = pa_stream_new (pulse->context, "Peak detect", &spec, NULL);
  if (pulse->level_stream == NULL)
    return;
  pa_stream_set_read_callback (pulse->level_stream, on_peaks, pulse);
  if (pa_stream_connect_record (pulse->level_stream, source, &attr,
                                PA_STREAM_DONT_MOVE | PA_STREAM_PEAK_DETECT
                                | PA_STREAM_ADJUST_LATENCY
                                | PA_STREAM_DONT_INHIBIT_AUTO_SUSPEND)
      < 0)
    {
      g_warning ("can't hear the sound server's %s: %s", source,
                 pa_strerror (pa_context_errno (pulse->context)));
      g_clear_pointer (&pulse->level_stream, pa_stream_unref);
      return;
    }
  pulse->level_source = g_strdup (source);
}

/* A peak stream on one stream of an application: on its sink's monitor, hearing that stream
   alone, or on the source it records from (CMN-210's sp-peaks.c, as pavucontrol's). */
static void
open_app_monitor (SdPulse *pulse, SdSide side, const Stream *stream, const char *key)
{
  pa_sample_spec spec = { .format = PA_SAMPLE_FLOAT32NE, .rate = PEAKS_PER_SECOND, .channels = 1 };
  pa_buffer_attr attr = { .maxlength = (uint32_t) -1, .fragsize = sizeof (float) };
  SdStream named = stream_named (stream, devices_of (pulse, side));
  const char *app = sd_state_stream_identity (&named);
  g_autofree char *source = NULL;
  AppMonitor *monitor;

  if (app == NULL)
    return;
  if (side == SD_OUTPUT)
    {
      Device *sink = find_device_by_index (pulse->lists[LIST_SINKS], stream->device);

      if (sink == NULL)
        return;
      source = g_strdup_printf ("%u", sink->monitor_source);
    }
  else
    source = g_strdup_printf ("%u", stream->device);
  monitor = g_new0 (AppMonitor, 1);
  monitor->pulse = pulse;
  monitor->side = side;
  monitor->app = g_strdup (app);
  monitor->stream = pa_stream_new (pulse->context, "Peak detect", &spec, NULL);
  if (monitor->stream == NULL)
    {
      g_hash_table_insert (pulse->monitors, g_strdup (key), monitor);
      return;
    }
  if (side == SD_OUTPUT)
    pa_stream_set_monitor_stream (monitor->stream, stream->index);
  pa_stream_set_read_callback (monitor->stream, on_app_peaks, monitor);
  if (pa_stream_connect_record (monitor->stream, source, &attr,
                                PA_STREAM_DONT_MOVE | PA_STREAM_PEAK_DETECT
                                | PA_STREAM_ADJUST_LATENCY
                                | PA_STREAM_DONT_INHIBIT_AUTO_SUSPEND)
      < 0)
    {
      g_warning ("can't hear %s's stream: %s", app,
                 pa_strerror (pa_context_errno (pulse->context)));
      g_clear_pointer (&monitor->stream, pa_stream_unref);
    }
  g_hash_table_insert (pulse->monitors, g_strdup (key), monitor);
}

/* A peak stream on each stream of the applications while someone hears them, and none
   otherwise. */
static void
follow_app_levels (SdPulse *pulse)
{
  g_autoptr (GHashTable) kept = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  GHashTableIter iter;
  gpointer key;

  if (pulse->app_level != NULL && pulse->ready)
    for (SdSide side = SD_OUTPUT; side <= SD_INPUT; side++)
      {
        GPtrArray *streams = streams_of (pulse, side);

        for (guint i = 0; i < streams->len; i++)
          {
            Stream *stream = g_ptr_array_index (streams, i);
            char *name;

            if (!stream_listed (stream))
              continue;
            name = g_strdup_printf ("%d:%u:%u", side, stream->index, stream->device);
            if (!g_hash_table_contains (pulse->monitors, name))
              open_app_monitor (pulse, side, stream, name);
            g_hash_table_add (kept, name);
          }
      }
  g_hash_table_iter_init (&iter, pulse->monitors);
  while (g_hash_table_iter_next (&iter, &key, NULL))
    if (!g_hash_table_contains (kept, key))
      g_hash_table_iter_remove (&iter);
}

/* The shares of streams that are gone. */
static void
prune_shares (SdPulse *pulse)
{
  GHashTableIter iter;
  gpointer key;

  g_hash_table_iter_init (&iter, pulse->shares);
  while (g_hash_table_iter_next (&iter, &key, NULL))
    {
      int side;
      guint32 index;

      if (sscanf (key, "%d:%u", &side, &index) != 2
          || find_stream (streams_of (pulse, side), index) == NULL)
        g_hash_table_iter_remove (&iter);
    }
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
  apply_pins (pulse);
  follow_input_level (pulse);
  follow_app_levels (pulse);
  prune_shares (pulse);
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
      pulse->listed[request->kind] = TRUE;
    }
  else
    g_warning ("can't read the sound server's %s: %s", list_names[request->kind],
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

/* The fields sinks and sources share, which libpulse gives in structs of their own. */
static Device *
device_new (SdSide side, const char *name, const char *description, const pa_cvolume *volume,
            int mute, guint32 card, pa_proplist *properties, const SdPortInfo *ports,
            guint32 n_ports, const char *active_name, const char *active_description,
            int active_type)
{
  Device *device = g_new0 (Device, 1);

  device->device.name = g_strdup (name);
  device->device.description = g_strdup (description != NULL ? description : name);
  device->volume = *volume;
  device->device.volume = pa_cvolume_max (volume);
  device->device.muted = mute != 0;
  device->device.virtual_device = card == PA_INVALID_INDEX;
  device->device.form = sd_state_device_form (side,
                                               property (properties, PA_PROP_DEVICE_FORM_FACTOR),
                                               active_name, active_type);
  device->device.active_port = g_strdup (active_name);
  device->device.active_port_description = g_strdup (active_description);
  device->device.ports = sd_state_plugged_ports (ports, n_ports, &device->device.n_ports);
  return device;
}

static void
on_sink (pa_context *context G_GNUC_UNUSED, const pa_sink_info *info, int eol, void *data)
{
  Request *request = data;
  Device *device;
  g_autofree SdPortInfo *ports = NULL;

  if (eol != 0)
    {
      request_done (request, eol);
      return;
    }
  /* A port whose availability isn't known counts as plugged. */
  ports = g_new0 (SdPortInfo, info->n_ports);
  for (guint32 i = 0; i < info->n_ports; i++)
    ports[i] = (SdPortInfo) { info->ports[i]->name, info->ports[i]->description,
                              info->ports[i]->available == PA_PORT_AVAILABLE_NO };
  device = device_new (SD_OUTPUT, info->name, info->description, &info->volume, info->mute,
                       info->card, info->proplist, ports, info->n_ports,
                       info->active_port != NULL ? info->active_port->name : NULL,
                       info->active_port != NULL ? info->active_port->description : NULL,
                       info->active_port != NULL ? info->active_port->type
                                                 : PA_DEVICE_PORT_TYPE_UNKNOWN);
  device->index = info->index;
  device->monitor_source = info->monitor_source;
  g_ptr_array_add (request->items, device);
}

static void
on_source (pa_context *context G_GNUC_UNUSED, const pa_source_info *info, int eol, void *data)
{
  Request *request = data;
  Device *device;
  g_autofree SdPortInfo *ports = NULL;

  if (eol != 0)
    {
      request_done (request, eol);
      return;
    }
  ports = g_new0 (SdPortInfo, info->n_ports);
  for (guint32 i = 0; i < info->n_ports; i++)
    ports[i] = (SdPortInfo) { info->ports[i]->name, info->ports[i]->description,
                              info->ports[i]->available == PA_PORT_AVAILABLE_NO };
  device = device_new (SD_INPUT, info->name, info->description, &info->volume, info->mute,
                       info->card, info->proplist, ports, info->n_ports,
                       info->active_port != NULL ? info->active_port->name : NULL,
                       info->active_port != NULL ? info->active_port->description : NULL,
                       info->active_port != NULL ? info->active_port->type
                                                 : PA_DEVICE_PORT_TYPE_UNKNOWN);
  device->index = info->index;
  device->monitor = info->monitor_of_sink != PA_INVALID_INDEX;
  g_ptr_array_add (request->items, device);
}

/* The fields sink inputs and source outputs share. */
static Stream *
stream_new (pa_context *context, guint32 index, guint32 client, guint32 device,
            pa_proplist *properties, const pa_cvolume *volume, gboolean has_volume, int corked)
{
  Stream *stream = g_new0 (Stream, 1);
  const char *app_id = property (properties, PA_PROP_APPLICATION_ID);

  /* PipeWire's for a Flatpak's client. */
  stream->app_id = g_strdup (property (properties, "pipewire.sec.app-id"));
  stream->name = g_strdup (property (properties, PA_PROP_APPLICATION_NAME));
  stream->binary = g_strdup (property (properties, PA_PROP_APPLICATION_PROCESS_BINARY));
  stream->icon = g_strdup (property (properties, PA_PROP_APPLICATION_ICON_NAME));
  stream->title = g_strdup (property (properties, PA_PROP_MEDIA_NAME));
  stream->index = index;
  stream->device = device;
  if (has_volume)
    stream->volume = *volume;
  stream->corked = corked != 0;
  stream->ignored = client == pa_context_get_index (context)
                      || (app_id != NULL && g_strv_contains (volume_controls, app_id));
  stream->event = g_strcmp0 (property (properties, PA_PROP_MEDIA_ROLE), "event") == 0;
  return stream;
}

static void
on_playing_stream (pa_context *context, const pa_sink_input_info *info, int eol, void *data)
{
  Request *request = data;

  if (eol != 0)
    {
      request_done (request, eol);
      return;
    }
  g_ptr_array_add (request->items,
                   stream_new (context, info->index, info->client, info->sink, info->proplist,
                               &info->volume, info->has_volume, info->corked));
}

static void
on_recording_stream (pa_context *context, const pa_source_output_info *info, int eol, void *data)
{
  Request *request = data;

  if (eol != 0)
    {
      request_done (request, eol);
      return;
    }
  g_ptr_array_add (request->items,
                   stream_new (context, info->index, info->client, info->source, info->proplist,
                               &info->volume, info->has_volume, info->corked));
}

static void
on_system_sounds (pa_context *context G_GNUC_UNUSED, const pa_ext_stream_restore_info *info,
                  int eol, void *data)
{
  Request *request = data;
  SystemSounds *entry;

  if (eol != 0)
    {
      request_done (request, eol);
      return;
    }
  if (g_strcmp0 (info->name, EVENT_ROLE) != 0)
    return;
  entry = g_new0 (SystemSounds, 1);
  entry->volume = info->volume;
  entry->device = g_strdup (info->device);
  entry->mute = info->mute != 0;
  g_ptr_array_add (request->items, entry);
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
    case LIST_PLAYING:
      operation = pa_context_get_sink_input_info_list (pulse->context, on_playing_stream, request);
      break;
    case LIST_RECORDING:
      operation = pa_context_get_source_output_info_list (pulse->context, on_recording_stream, request);
      break;
    case LIST_SYSTEM_SOUNDS:
      operation = pa_ext_stream_restore_read (pulse->context, on_system_sounds, request);
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
on_restore_target_off (GObject *source, GAsyncResult *result, gpointer data G_GNUC_UNUSED)
{
  g_autoptr (GError) error = NULL;

  if (!g_subprocess_wait_check_finish (G_SUBPROCESS (source), result, &error))
    g_warning ("can't turn off WirePlumber's target restore: %s", error->message);
}

/* WirePlumber remembers no stream's device, so that a move without a pin is forgotten when
   the application closes (ADR 0045): saved, so that it lasts across WirePlumber's restarts
   and logins. Asked on each connection, which covers a user who turned it back on. Without
   wpctl a move is remembered as WirePlumber remembers it by default, and the pins still
   work. */
static void
turn_off_restore_target (void)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GSubprocess) wpctl = g_subprocess_new (G_SUBPROCESS_FLAGS_STDOUT_SILENCE, &error,
                                                    "wpctl", "settings", "--save",
                                                    "node.stream.restore-target", "false", NULL);

  if (wpctl == NULL)
    {
      g_warning ("can't turn off WirePlumber's target restore: %s", error->message);
      return;
    }
  g_subprocess_wait_check_async (wpctl, NULL, on_restore_target_off, NULL);
}

static void
on_server_info (pa_context *context, const pa_server_info *info, void *data)
{
  SdPulse *pulse = data;

  if (context != pulse->context || info == NULL)
    return;
  /* pipewire-pulse's "PulseAudio (on PipeWire 1.6.8)": PulseAudio has no WirePlumber. */
  if (!pulse->restore_target_off && info->server_name != NULL
      && strstr (info->server_name, "PipeWire") != NULL)
    {
      pulse->restore_target_off = TRUE;
      turn_off_restore_target ();
    }
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
  drop_operation (pa_context_get_server_info (pulse->context, on_server_info, pulse));
}

/* A new stream's info, asked for from its event: moved first thing where it's pinned. */
static void
on_new_playing_stream (pa_context *context, const pa_sink_input_info *info, int eol, void *data)
{
  SdPulse *pulse = data;
  Stream *stream;

  if (eol != 0 || context != pulse->context)
    return;
  stream = stream_new (context, info->index, info->client, info->sink, info->proplist,
                       &info->volume, info->has_volume, info->corked);
  apply_pin (pulse, SD_OUTPUT, stream);
  stream_free (stream);
}

static void
on_new_recording_stream (pa_context *context, const pa_source_output_info *info, int eol,
                         void *data)
{
  SdPulse *pulse = data;
  Stream *stream;

  if (eol != 0 || context != pulse->context)
    return;
  stream = stream_new (context, info->index, info->client, info->source, info->proplist,
                       &info->volume, info->has_volume, info->corked);
  apply_pin (pulse, SD_INPUT, stream);
  stream_free (stream);
}

static void
on_event (pa_context *context, pa_subscription_event_type_t type, uint32_t index, void *data)
{
  SdPulse *pulse = data;
  /* Before the list, which the server answers after: WirePlumber links the stream about 4 ms
     after the event, and the move must land first (ADR 0045). */
  gboolean pinning = (type & PA_SUBSCRIPTION_EVENT_TYPE_MASK) == PA_SUBSCRIPTION_EVENT_NEW
                     && pulse->pins->len > 0;

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
    case PA_SUBSCRIPTION_EVENT_SINK_INPUT:
      if (pinning)
        drop_operation (pa_context_get_sink_input_info (context, index, on_new_playing_stream,
                                                        pulse));
      request_list (pulse, LIST_PLAYING);
      break;
    case PA_SUBSCRIPTION_EVENT_SOURCE_OUTPUT:
      if (pinning)
        drop_operation (pa_context_get_source_output_info (context, index,
                                                           on_new_recording_stream, pulse));
      request_list (pulse, LIST_RECORDING);
      break;
    case PA_SUBSCRIPTION_EVENT_CARD:
      request_list (pulse, LIST_CARDS);
      break;
    case PA_SUBSCRIPTION_EVENT_SERVER:
      request_server_info (pulse);
      break;
    default:
      break;
    }
}

static void
on_system_sounds_changed (pa_context *context, void *data)
{
  SdPulse *pulse = data;

  if (context == pulse->context)
    request_list (pulse, LIST_SYSTEM_SOUNDS);
}

static void
forget_server (SdPulse *pulse)
{
  close_level_stream (pulse);
  g_hash_table_remove_all (pulse->monitors);
  g_hash_table_remove_all (pulse->shares);
  g_hash_table_remove_all (pulse->moving);
  pulse->restore_target_off = FALSE;
  g_list_free_full (g_steal_pointer (&pulse->requests), (GDestroyNotify) request_free);
  for (guint i = 0; i < N_LISTS; i++)
    {
      g_clear_pointer (&pulse->lists[i], g_ptr_array_unref);
      pulse->lists[i] = list_new (i);
      pulse->listed[i] = FALSE;
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
  close_level_stream (pulse);
  g_hash_table_remove_all (pulse->monitors);
  pa_context_set_state_callback (pulse->context, NULL, NULL);
  pa_context_set_subscribe_callback (pulse->context, NULL, NULL);
  pa_ext_stream_restore_set_subscribe_cb (pulse->context, NULL, NULL);
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

  switch (pa_context_get_state (context))
    {
    case PA_CONTEXT_READY:
      pa_context_set_subscribe_callback (context, on_event, pulse);
      drop_operation (pa_context_subscribe (context,
                                            PA_SUBSCRIPTION_MASK_SINK
                                            | PA_SUBSCRIPTION_MASK_SOURCE
                                            | PA_SUBSCRIPTION_MASK_SINK_INPUT
                                            | PA_SUBSCRIPTION_MASK_SOURCE_OUTPUT
                                            | PA_SUBSCRIPTION_MASK_CARD
                                            | PA_SUBSCRIPTION_MASK_SERVER,
                                            NULL, NULL));
      /* System sounds' entry is the extension's, which has events of its own. */
      pa_ext_stream_restore_set_subscribe_cb (context, on_system_sounds_changed, pulse);
      drop_operation (pa_ext_stream_restore_subscribe (context, 1, NULL, NULL));
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
      /* Nothing more is sent to it. */
      pulse->ready = FALSE;
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
  pulse->monitors = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, app_monitor_free);
  pulse->shares = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, g_free);
  pulse->pins = sd_pins_new ();
  pulse->moving = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  forget_server (pulse);
  connect_to_server (pulse);
  return pulse;
}

const SdStatus *
sd_pulse_status (SdPulse *pulse)
{
  return &pulse->status;
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
  drop_operation (pa_context_set_sink_volume_by_name (pulse->context, sink->device.name, &channels,
                                                      NULL, NULL));
}

void
sd_pulse_set_input_volume (SdPulse *pulse, pa_volume_t volume)
{
  Device *source;
  pa_cvolume channels;

  if (!connected (pulse))
    return;
  source = find_device (pulse->lists[LIST_SOURCES], pulse->default_source);
  if (source == NULL || source->monitor)
    return;
  channels = source->volume;
  pa_cvolume_scale (&channels, volume);
  drop_operation (pa_context_set_source_volume_by_name (pulse->context, source->device.name,
                                                        &channels, NULL, NULL));
}

void
sd_pulse_set_default (SdPulse *pulse, SdSide side, const char *device)
{
  if (!connected (pulse))
    return;
  if (side == SD_OUTPUT)
    drop_operation (pa_context_set_default_sink (pulse->context, device, NULL, NULL));
  else
    drop_operation (pa_context_set_default_source (pulse->context, device, NULL, NULL));
}

void
sd_pulse_set_port (SdPulse *pulse, SdSide side, const char *device, const char *port)
{
  if (!connected (pulse))
    return;
  if (side == SD_OUTPUT)
    drop_operation (pa_context_set_sink_port_by_name (pulse->context, device, port, NULL, NULL));
  else
    drop_operation (pa_context_set_source_port_by_name (pulse->context, device, port, NULL, NULL));
}

void
sd_pulse_watch_input_level (SdPulse *pulse, SdPulseLevel level, gpointer data)
{
  pulse->level = level;
  pulse->level_data = data;
  follow_input_level (pulse);
}

void
sd_pulse_set_muted (SdPulse *pulse, gboolean muted)
{
  if (!connected (pulse) || pulse->default_sink == NULL)
    return;
  drop_operation (pa_context_set_sink_mute_by_name (pulse->context, pulse->default_sink, muted, NULL,
                                          NULL));
}

void
sd_pulse_set_input_muted (SdPulse *pulse, gboolean muted)
{
  if (!connected (pulse) || pulse->default_source == NULL)
    return;
  drop_operation (pa_context_set_source_mute_by_name (pulse->context, pulse->default_source, muted, NULL,
                                            NULL));
}

/* One stream's loudest channel to volume, the others in proportion. */
static void
set_stream_volume (SdPulse *pulse, SdSide side, const Stream *stream, pa_volume_t volume)
{
  pa_cvolume channels = stream->volume;

  /* A stream with no volume of its own: a passthrough one. */
  if (channels.channels == 0)
    return;
  pa_cvolume_scale (&channels, volume);
  if (side == SD_OUTPUT)
    drop_operation (pa_context_set_sink_input_volume (pulse->context, stream->index, &channels,
                                                      NULL, NULL));
  else
    drop_operation (pa_context_set_source_output_volume (pulse->context, stream->index,
                                                         &channels, NULL, NULL));
}

void
sd_pulse_set_app_volume (SdPulse *pulse, SdSide side, const char *app, pa_volume_t volume)
{
  const SdApp *found;

  if (!connected (pulse))
    return;
  found = sd_status_app (&pulse->status, side, app);
  if (found == NULL)
    return;
  /* The ratio from the volumes the slider showed, which are the status's; from silence, the
     shares the streams had before it. */
  for (guint i = 0; i < found->n_streams; i++)
    {
      Stream *stream = find_stream (streams_of (pulse, side), found->streams[i].index);
      g_autofree char *key = g_strdup_printf ("%d:%u", side, found->streams[i].index);
      double *share = g_hash_table_lookup (pulse->shares, key);
      pa_volume_t scaled;

      if (found->volume > 0)
        {
          scaled = sd_state_scaled_volume (found->streams[i].volume, found->volume, volume);
          if (volume == 0)
            {
              share = g_new (double, 1);
              *share = (double) found->streams[i].volume / found->volume;
              g_hash_table_insert (pulse->shares, g_steal_pointer (&key), share);
            }
        }
      else
        scaled = share != NULL ? (pa_volume_t) (*share * volume + 0.5) : volume;
      if (stream != NULL)
        set_stream_volume (pulse, side, stream, scaled);
    }
}

void
sd_pulse_set_stream_volume (SdPulse *pulse, SdSide side, guint32 index, pa_volume_t volume)
{
  Stream *stream;

  if (!connected (pulse))
    return;
  stream = find_stream (streams_of (pulse, side), index);
  if (stream != NULL)
    set_stream_volume (pulse, side, stream, volume);
}

void
sd_pulse_move_app (SdPulse *pulse, SdSide side, const char *app, const char *device)
{
  const SdApp *found;
  const char *target;

  if (!connected (pulse))
    return;
  found = sd_status_app (&pulse->status, side, app);
  /* The server forgets a stream's target when it's moved to the default, as PulseAudio
     does (pipewire-pulse's pulse-server.c, do_move_stream()), so that it follows the
     default from then on. */
  target = device ?: (side == SD_OUTPUT ? pulse->default_sink : pulse->default_source);
  if (found == NULL || target == NULL)
    return;
  for (guint i = 0; i < found->n_streams; i++)
    if (side == SD_OUTPUT)
      drop_operation (pa_context_move_sink_input_by_name (pulse->context,
                                                          found->streams[i].index, target,
                                                          NULL, NULL));
    else
      drop_operation (pa_context_move_source_output_by_name (pulse->context,
                                                             found->streams[i].index, target,
                                                             NULL, NULL));
}

void
sd_pulse_set_system_volume (SdPulse *pulse, pa_volume_t volume)
{
  GPtrArray *entries = pulse->lists[LIST_SYSTEM_SOUNDS];
  SystemSounds *entry = entries->len > 0 ? g_ptr_array_index (entries, 0) : NULL;
  pa_ext_stream_restore_info info = { .name = EVENT_ROLE };

  if (!connected (pulse) || !pulse->listed[LIST_SYSTEM_SOUNDS])
    return;
  /* Stereo, whatever the entry had: WirePlumber gives a mono entry's volume to a stereo
     stream's first channel alone and silences the other, where a stereo one's reaches a
     mono stream too. The entry's device and mute are kept. */
  pa_channel_map_init_stereo (&info.channel_map);
  pa_cvolume_set (&info.volume, info.channel_map.channels, volume);
  if (entry != NULL)
    {
      info.device = entry->device;
      info.mute = entry->mute;
    }
  drop_operation (pa_ext_stream_restore_write (pulse->context, PA_UPDATE_REPLACE, &info, 1, TRUE,
                                               NULL, NULL));
  /* Read back, after the write, for a server that sends no event for it. */
  request_list (pulse, LIST_SYSTEM_SOUNDS);
}

void
sd_pulse_watch_app_levels (SdPulse *pulse, SdPulseAppLevel level, gpointer data)
{
  pulse->app_level = level;
  pulse->app_level_data = data;
  follow_app_levels (pulse);
}

void
sd_pulse_set_pins (SdPulse *pulse, const SdPin *pins, guint n_pins)
{
  g_array_set_size (pulse->pins, 0);
  for (guint i = 0; i < n_pins; i++)
    sd_pins_set (pulse->pins, &pins[i]);
  update (pulse);
}

void
sd_pulse_set_profile (SdPulse *pulse, guint32 card, const char *profile)
{
  if (!connected (pulse))
    return;
  drop_operation (pa_context_set_card_profile_by_index (pulse->context, card, profile, NULL, NULL));
}

void
sd_pulse_free (SdPulse *pulse)
{
  g_clear_handle_id (&pulse->reconnect_id, g_source_remove);
  disconnect (pulse);
  for (guint i = 0; i < N_LISTS; i++)
    g_clear_pointer (&pulse->lists[i], g_ptr_array_unref);
  g_hash_table_unref (pulse->monitors);
  g_hash_table_unref (pulse->shares);
  g_hash_table_unref (pulse->moving);
  g_array_unref (pulse->pins);
  pa_glib_mainloop_free (pulse->mainloop);
  sd_status_clear (&pulse->status);
  g_free (pulse);
}
