/*
 * What the Sound applet's icon shows (CMN-213). See sd-state.h.
 */

#include "sd-state.h"

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
         && sd_cards_equal (a->cards, a->n_cards, b->cards, b->n_cards);
}

/* As pactl rounds it. */
static guint
percent_of (pa_volume_t volume)
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
  percent = percent_of (status->volume);
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
                              percent_of (status->volume));
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
  guint percent = percent_of (volume);
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

char **
sd_state_recording_names (const SdStream *streams, guint n_streams)
{
  g_autoptr (GHashTable) seen = g_hash_table_new (g_str_hash, g_str_equal);
  GPtrArray *names = g_ptr_array_new ();

  for (guint i = 0; i < n_streams; i++)
    {
      const SdStream *stream = &streams[i];
      const char *name = nonempty (stream->name) ?: nonempty (stream->binary);
      /* Not the binary first: several Electron programs run as electron. */
      const char *identity = nonempty (stream->app_id) ?: name;

      if (identity == NULL || !g_hash_table_add (seen, (char *) identity))
        continue;
      g_ptr_array_add (names, g_strdup (name ?: identity));
    }
  if (names->len == 0)
    {
      g_ptr_array_free (names, TRUE);
      return NULL;
    }
  g_ptr_array_add (names, NULL);
  return (char **) g_ptr_array_free (names, FALSE);
}
