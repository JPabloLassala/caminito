/*
 * The Dock's launch and attention feedback (CMN-221).
 */

#include "dk-feedback.h"

/* A launch from the dock whose first window hasn't mapped. */
typedef struct
{
  gint64  start;
  /* The dock's window counter when it was launched: a window opened later is the launch's. */
  guint64 since;
} Launch;

/* An item's windows that demand attention. */
typedef struct
{
  /* The handles of the windows that do. */
  GHashTable *demanding;
  /* When the latest began to, or -1. */
  gint64      bounce_start;
} Attention;

struct _DkFeedback
{
  /* An item's key to its Launch. */
  GHashTable *launches;
  /* An item's key to its Attention, while a window of it demands. */
  GHashTable *attention;
  /* The handles of the windows that are still urgent but have been focused since they began
     to demand attention: an X11 client's urgency hint stays until the client drops it, and
     the highlight lasts only until the window is focused. */
  GHashTable *answered;
  gint64      launch_timeout;
};

static void
attention_free (gpointer data)
{
  Attention *attention = data;

  g_clear_pointer (&attention->demanding, g_hash_table_unref);
  g_free (attention);
}

DkFeedback *
dk_feedback_new (void)
{
  DkFeedback *feedback = g_new0 (DkFeedback, 1);

  feedback->launches = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, g_free);
  feedback->attention = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, attention_free);
  feedback->answered = g_hash_table_new (NULL, NULL);
  feedback->launch_timeout = DK_LAUNCH_TIMEOUT;
  return feedback;
}

void
dk_feedback_free (DkFeedback *feedback)
{
  if (feedback == NULL)
    return;
  g_hash_table_unref (feedback->launches);
  g_hash_table_unref (feedback->attention);
  g_hash_table_unref (feedback->answered);
  g_free (feedback);
}

void
dk_feedback_set_launch_timeout (DkFeedback *feedback, gint64 timeout)
{
  feedback->launch_timeout = timeout;
}

void
dk_feedback_launch (DkFeedback *feedback, const char *key, guint64 since, gint64 now)
{
  Launch *launch = g_hash_table_lookup (feedback->launches, key);

  /* A launch while one is pulsing waits for the same first window, a little longer. */
  if (launch != NULL)
    {
      launch->start = now;
      return;
    }
  launch = g_new0 (Launch, 1);
  launch->start = now;
  launch->since = since;
  g_hash_table_insert (feedback->launches, g_strdup (key), launch);
}

static const DkItem *
item_for (const GArray *items, const char *key)
{
  for (guint i = 0; i < items->len; i++)
    if (g_strcmp0 (g_array_index (items, DkItem, i).key, key) == 0)
      return &g_array_index (items, DkItem, i);
  return NULL;
}

/* Whether the item has a window opened after the launch. */
static gboolean
has_new_window (const DkItem *item, const Launch *launch)
{
  for (guint i = 0; i < item->windows->len; i++)
    if (g_array_index (item->windows, DkWindow, i).opened > launch->since)
      return TRUE;
  return FALSE;
}

static void
update_launches (DkFeedback *feedback, const GArray *items, gint64 now)
{
  GHashTableIter iter;
  gpointer key, value;

  g_hash_table_iter_init (&iter, feedback->launches);
  while (g_hash_table_iter_next (&iter, &key, &value))
    {
      const Launch *launch = value;
      const DkItem *item = item_for (items, key);

      if (item == NULL || now - launch->start >= feedback->launch_timeout
          || has_new_window (item, launch))
        g_hash_table_iter_remove (&iter);
    }
}

static void
update_attention (DkFeedback *feedback, const GArray *items, gint64 now)
{
  g_autoptr (GHashTable) seen = g_hash_table_new (g_str_hash, g_str_equal);
  GHashTable *answered = g_hash_table_new (NULL, NULL);
  GHashTableIter iter;
  gpointer key, value;

  for (guint i = 0; i < items->len; i++)
    {
      const DkItem *item = &g_array_index (items, DkItem, i);
      Attention *attention = g_hash_table_lookup (feedback->attention, item->key);
      g_autoptr (GHashTable) demanding = g_hash_table_new (NULL, NULL);
      gboolean began = FALSE;

      /* The focused window demands nothing, whatever its flag says, and nor does one that
         has been focused since it began to. */
      for (guint j = 0; j < item->windows->len; j++)
        {
          const DkWindow *window = &g_array_index (item->windows, DkWindow, j);

          if (!window->urgent)
            continue;
          if (window->active || g_hash_table_contains (feedback->answered, window->handle))
            g_hash_table_add (answered, window->handle);
          else
            {
              g_hash_table_add (demanding, window->handle);
              if (attention == NULL || !g_hash_table_contains (attention->demanding, window->handle))
                began = TRUE;
            }
        }
      if (g_hash_table_size (demanding) == 0)
        {
          g_hash_table_remove (feedback->attention, item->key);
          continue;
        }
      if (attention == NULL)
        {
          attention = g_new0 (Attention, 1);
          attention->bounce_start = -1;
          g_hash_table_insert (feedback->attention, g_strdup (item->key), attention);
        }
      g_clear_pointer (&attention->demanding, g_hash_table_unref);
      attention->demanding = g_hash_table_ref (demanding);
      if (began)
        attention->bounce_start = now;
      g_hash_table_add (seen, item->key);
    }
  g_hash_table_unref (feedback->answered);
  feedback->answered = answered;
  /* An item that has gone takes its attention with it. */
  g_hash_table_iter_init (&iter, feedback->attention);
  while (g_hash_table_iter_next (&iter, &key, &value))
    if (!g_hash_table_contains (seen, key))
      g_hash_table_iter_remove (&iter);
}

void
dk_feedback_update (DkFeedback *feedback, const GArray *items, gint64 now)
{
  update_launches (feedback, items, now);
  update_attention (feedback, items, now);
}

DkFeedbackState
dk_feedback_get (const DkFeedback *feedback, const char *key, gint64 now)
{
  const Launch *launch = g_hash_table_lookup (feedback->launches, key);
  const Attention *attention = g_hash_table_lookup (feedback->attention, key);
  DkFeedbackState state = { .bounce_start = -1 };

  state.pulsing = launch != NULL && now - launch->start < feedback->launch_timeout;
  if (attention != NULL)
    {
      state.highlighted = TRUE;
      state.bouncing = attention->bounce_start >= 0
                       && now - attention->bounce_start < DK_BOUNCE_LENGTH;
      if (state.bouncing)
        state.bounce_start = attention->bounce_start;
    }
  return state;
}

gint64
dk_feedback_deadline (const DkFeedback *feedback, gint64 now)
{
  GHashTableIter iter;
  gpointer value;
  gint64 deadline = -1;

  g_hash_table_iter_init (&iter, feedback->launches);
  while (g_hash_table_iter_next (&iter, NULL, &value))
    {
      gint64 end = ((const Launch *) value)->start + feedback->launch_timeout;

      if (end > now && (deadline < 0 || end < deadline))
        deadline = end;
    }
  g_hash_table_iter_init (&iter, feedback->attention);
  while (g_hash_table_iter_next (&iter, NULL, &value))
    {
      const Attention *attention = value;
      gint64 end = attention->bounce_start + DK_BOUNCE_LENGTH;

      if (attention->bounce_start >= 0 && end > now && (deadline < 0 || end < deadline))
        deadline = end;
    }
  return deadline;
}
