/*
 * caminito-notifications as the Notifications applet uses it (CMN-260, ADR 0051): the
 * Notification history over the daemon's private API, org.mate.Caminito.Notifications,
 * followed through HistoryChanged and as the daemon leaves and comes back to the bus, and the
 * calls that dismiss, clear and act on its entries, and Do Not Disturb (CMN-263), followed
 * through DoNotDisturbChanged. The daemon passes Super+N on to the applet with
 * OpenCenterRequested, since the applet owns no name to be called at. It never talks to
 * org.freedesktop.Notifications.
 */

#pragma once

#include <gio/gio.h>

#include "nt-state.h"

typedef struct _NtDaemon NtDaemon;

/* Called from the main loop once a burst of changes has settled: the history was read
   again, Do Not Disturb changed, or the daemon came or went. */
typedef void (*NtDaemonChanged) (NtDaemon *daemon, gpointer data);
/* Called as the daemon asks for the Notification center, as Caminito has it for Super+N. */
typedef void (*NtDaemonOpenCenter) (NtDaemon *daemon, gpointer data);

NtDaemon        *nt_daemon_new           (NtDaemonChanged    changed,
                                          NtDaemonOpenCenter open_center,
                                          gpointer           data);
void             nt_daemon_free          (NtDaemon *daemon);
/* Whether the daemon owns its name on the session bus. */
gboolean         nt_daemon_running       (NtDaemon *daemon);
/* The NtEntries of the history, newest first. Empty while the daemon isn't running. */
const GPtrArray *nt_daemon_entries       (NtDaemon *daemon);
/* The history's calls. The daemon's HistoryChanged brings what they change. */
void             nt_daemon_dismiss       (NtDaemon *daemon, guint32 id);
void             nt_daemon_clear_all     (NtDaemon *daemon);
/* key "default" is a click on the entry. */
void             nt_daemon_invoke_action (NtDaemon *daemon, guint32 id, const char *key);
/* Whether Do Not Disturb is on, and its end in seconds since the epoch, or 0 for none. Off
   while the daemon isn't running. */
gboolean         nt_daemon_do_not_disturb (NtDaemon *daemon, guint64 *end);
/* until is "one-hour", "four-hours", "tomorrow-morning", "until-turned-off" or "off". The
   daemon's DoNotDisturbChanged brings what they change. */
void             nt_daemon_set_do_not_disturb    (NtDaemon *daemon, const char *until);
void             nt_daemon_toggle_do_not_disturb (NtDaemon *daemon);
