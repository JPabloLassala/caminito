/*
 * The popup's Stay awake row and the list of what keeps the computer awake (265), as in the
 * prototype's "Popup C". It draws what pw-session.c has and calls back for the switch, so
 * that it knows nothing of mate-session.
 */

#pragma once

#include <gtk/gtk.h>

#include "pw-state.h"

typedef struct _PwAwake PwAwake;

typedef struct
{
  /* The user turned the switch on or off. */
  void (*set_stay_awake) (gboolean on, gpointer data);
} PwAwakeActions;

/* actions is copied. */
PwAwake   *pw_awake_new             (const PwAwakeActions *actions, gpointer data);
void       pw_awake_free            (PwAwake *awake);
/* The row under the profile tiles: a lock icon, "Stay awake", and the switch. */
GtkWidget *pw_awake_row             (PwAwake *awake);
/* A separator and one line for each inhibitor, with its icon, name and reason. It is
   hidden while there are none. */
GtkWidget *pw_awake_inhibitors      (PwAwake *awake);
/* The switch as it is now, which doesn't call back. */
void       pw_awake_set_stay_awake  (PwAwake *awake, gboolean on);
/* The PwInhibitors to list. They're copied. */
void       pw_awake_set_inhibitors  (PwAwake *awake, const GPtrArray *inhibitors);
