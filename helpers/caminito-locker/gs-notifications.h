/*
 * The Lock screen's list of notifications (CMN-300): what caminito-notifications' private API
 * gives as LockScreenList, drawn read-only and as text only. See gs-notifications.c.
 */

#ifndef __GS_NOTIFICATIONS_H
#define __GS_NOTIFICATIONS_H

#include <gtk/gtk.h>

G_BEGIN_DECLS

/* How wide the list is, in pixels. */
#define GS_NOTIFICATIONS_WIDTH 360

/* Called when the entries shown are another's. */
typedef void (*GSNotificationsChanged)(void);

/* The list, empty and hidden. It is one widget, for the caller to put in the window that the
 * pointer is in. */
GtkWidget *gs_notifications_new(void);

/* Starts asking the daemon for the entries, and again whenever it says they changed. Nothing
 * waits for it: a daemon that is missing or doesn't answer is an empty list. */
void gs_notifications_start(GtkWidget *list, GSNotificationsChanged changed);

/* Whether the newest entry is the last, nearest a corner at the bottom, or the first. */
void gs_notifications_set_newest_last(GtkWidget *list, gboolean newest_last);

/* How many entries the list shows. */
guint gs_notifications_get_count(GtkWidget *list);

G_END_DECLS

#endif /* __GS_NOTIFICATIONS_H */
