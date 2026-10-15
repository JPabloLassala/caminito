/*
 * mate-session as the Power applet uses it (265): Stay awake, which is an inhibitor the
 * applet holds, and the list of inhibitors that keep the computer awake, followed as
 * applications add and remove them and as mate-session stops and starts.
 */

#pragma once

#include <gio/gio.h>

#include "pw-state.h"

typedef struct _PwSession PwSession;

/* Called from the main loop once a burst of changes has settled: the list changed, Stay
   awake went on or off by itself (a failed call, mate-session gone), or mate-session came
   or went. */
typedef void (*PwSessionChanged) (PwSession *session, gpointer data);

PwSession       *pw_session_new              (PwSessionChanged changed, gpointer data);
/* Gives Stay awake back if it's on. */
void             pw_session_free             (PwSession *session);
/* Whether Stay awake is on, or turning on. It's off again after a failed call and while
   mate-session isn't running, and the applet is told. */
gboolean         pw_session_stay_awake       (PwSession *session);
/* Turns Stay awake on or off. */
void             pw_session_set_stay_awake   (PwSession *session, gboolean on);
/* The PwInhibitors that keep the computer awake now, with the suspend or idle flag, in
   the order mate-session lists them. Empty while mate-session isn't running. */
const GPtrArray *pw_session_inhibitors       (PwSession *session);
