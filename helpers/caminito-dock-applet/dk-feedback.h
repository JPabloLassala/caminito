/*
 * The Dock's launch and attention feedback (CMN-221), without a panel: which items pulse
 * because the dock launched them and no window of theirs has mapped yet, and which bounce and
 * keep a highlight because a window of theirs demands attention. Time is an argument, in
 * microseconds on any monotonic clock, so the tests are deterministic.
 */

#pragma once

#include <glib.h>

#include "dk-model.h"

/* How long an item pulses for a program that maps no window. */
#define DK_LAUNCH_TIMEOUT (10 * G_USEC_PER_SEC)
/* How long the bounce lasts: a few bounces, then the highlight alone. */
#define DK_BOUNCE_LENGTH (1500 * (G_USEC_PER_SEC / 1000))

typedef struct _DkFeedback DkFeedback;

typedef struct
{
  /* Launched from the dock, and no window of the program has mapped yet. */
  gboolean pulsing;
  /* A window began to demand attention less than DK_BOUNCE_LENGTH ago. */
  gboolean bouncing;
  /* When the bounce began; meaningful while bouncing. */
  gint64   bounce_start;
  /* A window of the item demands attention and doesn't have the focus. */
  gboolean highlighted;
} DkFeedbackState;

DkFeedback *dk_feedback_new  (void);
void        dk_feedback_free (DkFeedback *feedback);

/* Changes the pulse's timeout, for the tests that mean to see it end. */
void        dk_feedback_set_launch_timeout (DkFeedback *feedback, gint64 timeout);

/* The dock launched the item with key at now, when the latest window had opened at since (a
   DkWindow's opened): only a window opened later ends the pulse, so a new instance of a running
   program pulses until its own first window. */
void        dk_feedback_launch (DkFeedback *feedback,
                                const char *key,
                                guint64 since,
                                gint64 now);

/* The items as they are at now (dk_items_build's, with each window's urgent flag): ends the
   pulses of the items that have a new window or have timed out or gone, begins a bounce for
   each window that has begun to demand attention, and forgets the windows that no longer do
   or have been focused since they began to. */
void        dk_feedback_update (DkFeedback *feedback, const GArray *items, gint64 now);

/* Whether the Launcher entry of the item with key is urgent (CMN-227). Attention has a second
   source in it, beside the urgent windows: the next update bounces the icon when it becomes
   urgent, and the highlight lasts while it is, whatever the windows do, and the windows' lasts
   whatever it does. Focusing a window doesn't end it; the entry saying it isn't urgent does. */
void        dk_feedback_set_launcher_urgent (DkFeedback *feedback, const char *key,
                                             gboolean urgent);

DkFeedbackState dk_feedback_get (const DkFeedback *feedback, const char *key, gint64 now);

/* The next time after now that something ends by itself, a pulse's timeout or a bounce's end,
   or -1. The dock updates again then. */
gint64      dk_feedback_deadline (const DkFeedback *feedback, gint64 now);
