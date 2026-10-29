/*
 * caminito-notifications as the Notifications applet uses it (CMN-260, ADR 0051): the
 * Notification history over the daemon's private API, org.mate.Caminito.Notifications,
 * followed through HistoryChanged and as the daemon leaves and comes back to the bus, and the
 * calls that dismiss, clear and act on its entries. The daemon hears whether the Notification
 * center is open, as it shows no bubbles while it is, and asks for it through
 * CenterRequested (CMN-285). The applet owns no name and never talks to
 * org.freedesktop.Notifications.
 */

#pragma once

#include <gio/gio.h>

#include "nt-state.h"

typedef struct _NtDaemon NtDaemon;

/* Called from the main loop once a burst of changes has settled: the history was read
   again, or the daemon came or went. */
typedef void (*NtDaemonChanged) (NtDaemon *daemon, gpointer data);
/* The daemon asks for the Notification center. */
typedef void (*NtDaemonCenterRequested) (NtDaemon *daemon, gpointer data);

NtDaemon        *nt_daemon_new           (NtDaemonChanged         changed,
                                          NtDaemonCenterRequested center_requested,
                                          gpointer                data);
void             nt_daemon_free          (NtDaemon *daemon);
/* Whether the daemon owns its name on the session bus. */
gboolean         nt_daemon_running       (NtDaemon *daemon);
/* The NtEntries of the history, newest first. Empty while the daemon isn't running. */
const GPtrArray *nt_daemon_entries       (NtDaemon *daemon);
/* The history's calls. The daemon's HistoryChanged brings what they change. */
void             nt_daemon_dismiss       (NtDaemon *daemon, guint32 id);
void             nt_daemon_clear_all     (NtDaemon *daemon);
/* key "default" is a click on the entry, which brings its app forward without a default
   action. token is an xdg-activation token for the app, or NULL. */
void             nt_daemon_invoke_action (NtDaemon   *daemon,
                                          guint32     id,
                                          const char *key,
                                          const char *token);
/* text, sent from the entry's reply field. */
void             nt_daemon_reply         (NtDaemon *daemon, guint32 id, const char *text);
/* Whether the Notification center is open, which the daemon hears now and whenever it comes
   back to the bus. */
void             nt_daemon_set_center_open (NtDaemon *daemon, gboolean open);
