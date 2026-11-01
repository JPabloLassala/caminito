/*
 * The Lock screen's list of notifications (CMN-300, ADR 0051).
 *
 * caminito-notifications decides which notifications the Lock screen shows and gives them as
 * text, through LockScreenList of its private API, and tells with LockScreenChanged when to
 * ask again. This draws what it gets, read-only: no actions, no replies, no dismissing and no
 * images. Everything is asynchronous and nothing waits for the daemon: a call that gets no
 * answer in CALL_TIMEOUT_MSEC, a daemon that isn't on the bus, or an answer that isn't what
 * the method gives is an empty list. The daemon limits the number and the length of the
 * entries, and so does this, so that no daemon can crash the locker into its give-up state.
 * The texts go to labels as text, never as markup.
 */

#include "config.h"

#include <string.h>

#include <glib/gi18n.h>

#include "gs-notifications.h"

#define gs_debug g_debug

#define DAEMON_NAME      "org.mate.Caminito.Notifications"
#define DAEMON_PATH      "/org/mate/Caminito/Notifications"
#define DAEMON_INTERFACE "org.mate.Caminito.Notifications"

/* How long the daemon has to answer. */
#define CALL_TIMEOUT_MSEC 3000

/* The most entries shown, and the most characters of an app's name, a summary and a body. */
#define MAX_ENTRIES 8
#define MAX_APP_CHARS 60
#define MAX_SUMMARY_CHARS 120
#define MAX_BODY_CHARS 300

static GDBusProxy *proxy = NULL;
static GCancellable *pending = NULL;
static GSNotificationsChanged changed_func = NULL;
static guint count = 0;
static gboolean newest_last = FALSE;

/* `text` as valid UTF-8 of at most `max` characters, the last of them an ellipsis when it was
 * longer. It looks at no more of `text` than `max` characters can take. */
static char* limited(const char* text, guint max)
{
	gsize length = strnlen(text, (gsize) max * 4 + 4);
	char* valid = g_utf8_make_valid(text, (gssize) length);
	char* end;

	if (g_utf8_strlen(valid, -1) > (glong) max)
	{
		end = g_utf8_offset_to_pointer(valid, max - 1);
		*end = '\0';
		char* cut = g_strconcat(valid, "…", NULL);
		g_free(valid);
		valid = cut;
	}

	return valid;
}

static GtkWidget* add_label(GtkWidget* entry, const char* text, guint max, const char* css_class, guint lines)
{
	GtkWidget* label;
	char* shown = limited(text, max);

	gs_debug("Notification text: %ld characters", (long) g_utf8_strlen(shown, -1));
	label = gtk_label_new(shown);
	g_free(shown);
	gtk_style_context_add_class(gtk_widget_get_style_context(label), css_class);
	gtk_label_set_xalign(GTK_LABEL(label), 0.0f);
	gtk_label_set_line_wrap(GTK_LABEL(label), TRUE);
	gtk_label_set_line_wrap_mode(GTK_LABEL(label), PANGO_WRAP_WORD_CHAR);
	gtk_label_set_ellipsize(GTK_LABEL(label), PANGO_ELLIPSIZE_END);
	gtk_label_set_lines(GTK_LABEL(label), (gint) lines);
	/* Their width is the list's, whatever the text. */
	gtk_label_set_max_width_chars(GTK_LABEL(label), 1);
	gtk_container_add(GTK_CONTAINER(entry), label);

	return label;
}

static void add_entry(GtkWidget* list, const char* app, const char* summary, const char* body)
{
	GtkWidget* entry = gtk_box_new(GTK_ORIENTATION_VERTICAL, 2);

	gtk_style_context_add_class(gtk_widget_get_style_context(entry), "lock-notification");
	add_label(entry, app, MAX_APP_CHARS, "lock-notification-app", 1);
	add_label(entry, summary, MAX_SUMMARY_CHARS, "lock-notification-summary", 2);
	if (body[0] != '\0')
	{
		add_label(entry, body, MAX_BODY_CHARS, "lock-notification-body", 4);
	}
	gtk_container_add(GTK_CONTAINER(list), entry);
	if (newest_last)
	{
		/* Each entry, older than the one before, goes before it. */
		gtk_box_reorder_child(GTK_BOX(list), entry, 0);
	}
	gtk_widget_show_all(entry);
}

static void destroy_child(GtkWidget* child, gpointer data G_GNUC_UNUSED)
{
	gtk_widget_destroy(child);
}

/* Shows what LockScreenList gave, `entries` being NULL for no answer, and says when the
 * number of entries is another. Anything but the method's type is no entries. */
static void apply(GtkWidget* list, GVariant* entries)
{
	GVariantIter iter;
	const char* app;
	const char* summary;
	const char* body;
	guint shown = 0;

	gtk_container_foreach(GTK_CONTAINER(list), destroy_child, NULL);

	if (entries != NULL && g_variant_is_of_type(entries, G_VARIANT_TYPE("(a(sss))")))
	{
		g_autoptr(GVariant) array = g_variant_get_child_value(entries, 0);

		g_variant_iter_init(&iter, array);
		while (shown < MAX_ENTRIES && g_variant_iter_next(&iter, "(&s&s&s)", &app, &summary, &body))
		{
			add_entry(list, app, summary, body);
			shown++;
		}
	}

	gs_debug("Notifications: %u shown", shown);
	count = shown;
	if (changed_func != NULL)
	{
		changed_func();
	}
}

static void got_list(GObject* source, GAsyncResult* result, gpointer data)
{
	GtkWidget* list = data;
	g_autoptr(GError) error = NULL;
	g_autoptr(GVariant) entries = g_dbus_proxy_call_finish(G_DBUS_PROXY(source), result, &error);

	if (g_error_matches(error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
	{
		/* Another call took its place. */
		return;
	}
	if (entries == NULL)
	{
		gs_debug("Can't list the notifications: %s", error->message);
	}
	apply(list, entries);
}

/* Asks for the entries again, and drops a call that hasn't been answered. */
static void reload(GtkWidget* list)
{
	g_autofree char* owner = proxy != NULL ? g_dbus_proxy_get_name_owner(proxy) : NULL;

	if (pending != NULL)
	{
		g_cancellable_cancel(pending);
		g_clear_object(&pending);
	}
	if (owner == NULL)
	{
		apply(list, NULL);
		return;
	}

	pending = g_cancellable_new();
	g_dbus_proxy_call(proxy, "LockScreenList", NULL, G_DBUS_CALL_FLAGS_NO_AUTO_START, CALL_TIMEOUT_MSEC, pending, got_list, list);
}

static void on_signal(GDBusProxy* source G_GNUC_UNUSED, const char* sender G_GNUC_UNUSED, const char* signal_name, GVariant* parameters G_GNUC_UNUSED, gpointer data)
{
	if (g_strcmp0(signal_name, "LockScreenChanged") == 0)
	{
		reload(data);
	}
}

static void on_owner(GDBusProxy* source G_GNUC_UNUSED, GParamSpec* pspec G_GNUC_UNUSED, gpointer data)
{
	reload(data);
}

static void got_proxy(GObject* source G_GNUC_UNUSED, GAsyncResult* result, gpointer data)
{
	g_autoptr(GError) error = NULL;

	proxy = g_dbus_proxy_new_for_bus_finish(result, &error);
	if (proxy == NULL)
	{
		gs_debug("Can't ask for the notifications: %s", error->message);
		apply(data, NULL);
		return;
	}

	/* It lasts as long as the locker. */
	g_signal_connect(proxy, "g-signal", G_CALLBACK(on_signal), data);
	g_signal_connect(proxy, "notify::g-name-owner", G_CALLBACK(on_owner), data);
	reload(data);
}

GtkWidget* gs_notifications_new(void)
{
	GtkWidget* list = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);

	gtk_style_context_add_class(gtk_widget_get_style_context(list), "lock-notifications");
	gtk_widget_set_size_request(list, GS_NOTIFICATIONS_WIDTH, -1);
	/* The window it is in decides whether it shows, as it has entries. */
	gtk_widget_set_no_show_all(list, TRUE);

	return list;
}

void gs_notifications_start(GtkWidget* list, GSNotificationsChanged changed)
{
	changed_func = changed;
	g_dbus_proxy_new_for_bus(G_BUS_TYPE_SESSION,
	                         G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START | G_DBUS_PROXY_FLAGS_DO_NOT_LOAD_PROPERTIES,
	                         NULL,
	                         DAEMON_NAME,
	                         DAEMON_PATH,
	                         DAEMON_INTERFACE,
	                         NULL,
	                         got_proxy,
	                         list);
}

void gs_notifications_set_newest_last(GtkWidget* list, gboolean last)
{
	GList* children;

	if (newest_last == last)
	{
		return;
	}

	newest_last = last;
	children = gtk_container_get_children(GTK_CONTAINER(list));
	for (GList* child = children; child != NULL; child = child->next)
	{
		gtk_box_reorder_child(GTK_BOX(list), child->data, 0);
	}
	g_list_free(children);
}

guint gs_notifications_get_count(void)
{
	return count;
}
