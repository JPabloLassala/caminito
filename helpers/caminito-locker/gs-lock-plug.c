/* -*- Mode: C; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 8 -*-
 *
 * Copyright (C) 2004-2006 William Jon McCann <mccann@jhu.edu>
 * Copyright (C) 2012-2021 MATE Developers
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301, USA.
 *
 * Authors: William Jon McCann <mccann@jhu.edu>
 *
 * Ported to caminito-locker (131, ADR 0007) from mate-screensaver 60b2db3's
 * src/gs-lock-plug.c. GSLockPlug is a GtkBin rather than a GtkPlug, in whichever of the
 * locker's lock windows has the pointer, and what it did as a window it now does to that
 * window: the default button, the key events it holds back and the busy cursor. The rest
 * is mate-screensaver's, but for what a lock surface can't have or Caminito leaves out:
 *  - the dialog built in code when the theme's .ui is missing: caminito-locker's own copy
 *    of lock-dialog-default.ui stands in for it;
 *  - tooltips and the entry's menu, as a lock surface can't show a popup;
 *  - the keyboard layout indicator, which is libmatekbd's on X11;
 *  - the note, which needs libnotify, as a mate-screensaver built without it has none.
 */

#include "config.h"

#include <stdlib.h>
#include <unistd.h>
#include <string.h>
#include <errno.h>
#include <time.h>
#include <unistd.h>
#include <sys/utsname.h>
#include <sys/stat.h>

#include <glib/gprintf.h>
#include <glib/gstdio.h>

#include <glib/gi18n.h>
#include <gdk/gdkkeysyms.h>
#include <gtk/gtk.h>
#include <gio/gio.h>

#define MATE_DESKTOP_USE_UNSTABLE_API
#include <libmate-desktop/mate-desktop-utils.h>

#include "gs-lock-plug.h"

#define gs_debug g_debug

#define GSETTINGS_SCHEMA "org.mate.screensaver"

#define KEY_LOCK_DIALOG_THEME "lock-dialog-theme"

#define KEY_LOCK_DIALOG_T_FMT "lock-dialog-time-format"
#define KEY_LOCK_DIALOG_D_FMT "lock-dialog-date-format"

#define MDM_FLEXISERVER_COMMAND "mdmflexiserver"
#define MDM_FLEXISERVER_ARGS    "--startnew Standard"

#define GDM_FLEXISERVER_COMMAND "gdmflexiserver"
#define GDM_FLEXISERVER_ARGS    "--startnew Standard"

/* caminito-locker's lock-dialog-default.ui, for a theme whose .ui isn't installed. */
#define DEFAULT_THEME_RESOURCE "/org/mate/caminito/locker/lock-dialog-default.ui"

#define DIALOG_TIMEOUT_MSEC 60000

static void gs_lock_plug_finalize   (GObject         *object);

struct GSLockPlugPrivate
{
	GtkWidget   *auth_action_area;

	GtkWidget   *notebook;
	GtkWidget   *auth_face_image;
	GtkWidget   *auth_time_label;
	GtkWidget   *auth_date_label;
	GtkWidget   *auth_realname_label;
	GtkWidget   *auth_username_label;
	GtkWidget   *auth_prompt_label;
	GtkWidget   *auth_prompt_entry;
	GtkWidget   *auth_prompt_box;
	GtkWidget   *auth_capslock_label;
	GtkWidget   *auth_message_label;
	GtkWidget   *status_message_label;

	GtkWidget   *auth_unlock_button;
	GtkWidget   *auth_switch_button;
	GtkWidget   *auth_cancel_button;
	GtkWidget   *auth_logout_button;
	GtkWidget   *auth_note_button;

	GtkWidget   *auth_prompt_kbd_layout_indicator;

	/* The lock window the dialog is in, and its handler of the keys typed there. */
	GtkWidget   *toplevel;
	gulong       toplevel_key_press_id;
	/* The button that is the window's default, whichever window that is. */
	GtkWidget   *default_button;

	gboolean     caps_lock_on;
	gboolean     switch_enabled;
	gboolean     logout_enabled;
	char        *logout_command;
	char        *status_message;

	guint        timeout;

	guint        datetime_timeout_id;
	guint        cancel_timeout_id;
	guint        auth_check_idle_id;
	guint        response_idle_id;

	GList       *key_events;
};

typedef struct _ResponseData ResponseData;

struct _ResponseData
{
	gint response_id;
};

enum
{
    RESPONSE,
    CLOSE,
    LAST_SIGNAL
};

enum
{
    PROP_0,
    PROP_LOGOUT_ENABLED,
    PROP_LOGOUT_COMMAND,
    PROP_SWITCH_ENABLED,
    PROP_STATUS_MESSAGE
};

static guint lock_plug_signals [LAST_SIGNAL] = { 0 };

G_DEFINE_TYPE_WITH_PRIVATE (GSLockPlug, gs_lock_plug, GTK_TYPE_BIN)

static gboolean
process_is_running (const char * name)
{
        int num_processes;
        gchar *command = g_strdup_printf ("pidof %s | wc -l", name);
        FILE *fp = popen(command, "r");

        if (fscanf(fp, "%d", &num_processes) != 1)
                num_processes = 0;

        pclose(fp);
        g_free (command);

        if (num_processes > 0) {
                return TRUE;
        } else {
                return FALSE;
        }
}

static void
do_user_switch (GSLockPlug *plug G_GNUC_UNUSED)
{
	GError  *error;
	gboolean res;
	char    *command;

	if (process_is_running ("mdm"))
	{
		/* MDM */
		command = g_strdup_printf ("%s %s",
								   MDM_FLEXISERVER_COMMAND,
								   MDM_FLEXISERVER_ARGS);

		error = NULL;
		res = mate_gdk_spawn_command_line_on_screen (gdk_screen_get_default (),
												command,
												&error);

		g_free (command);

		if (! res)
		{
			gs_debug ("Unable to start MDM greeter: %s", error->message);
			g_error_free (error);
		}
	}
	else if (process_is_running ("gdm") || process_is_running("gdm3") || process_is_running("gdm-binary"))
	{
		/* GDM */
		command = g_strdup_printf ("%s %s",
								   GDM_FLEXISERVER_COMMAND,
								   GDM_FLEXISERVER_ARGS);

		error = NULL;
		res = mate_gdk_spawn_command_line_on_screen (gdk_screen_get_default (),
												command,
												&error);

		g_free (command);

		if (! res) {
			gs_debug ("Unable to start GDM greeter: %s", error->message);
			g_error_free (error);
		}
	}
	else if (g_getenv ("XDG_SEAT_PATH") != NULL)
	{
		/* LightDM */
		GDBusProxyFlags flags = G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START;
		GDBusProxy *proxy = NULL;

		error = NULL;
		proxy = g_dbus_proxy_new_for_bus_sync(G_BUS_TYPE_SYSTEM,
											  flags,
											  NULL,
											  "org.freedesktop.DisplayManager",
											  g_getenv ("XDG_SEAT_PATH"),
											  "org.freedesktop.DisplayManager.Seat",
											  NULL,
											  &error);
		if (proxy != NULL) {
			g_dbus_proxy_call_sync (proxy,
									"SwitchToGreeter",
									g_variant_new ("()"),
									G_DBUS_CALL_FLAGS_NONE,
									-1,
									NULL,
									NULL);
			g_object_unref (proxy);
		}
		else {
			gs_debug ("Unable to start LightDM greeter: %s", error->message);
			g_error_free (error);
		}
	}

}

static void
set_status_text (GSLockPlug *plug,
                 const char *text)
{
	if (plug->priv->auth_message_label != NULL)
	{
		gtk_label_set_text (GTK_LABEL (plug->priv->auth_message_label), text);
	}
}

static gboolean
date_time_update (GSLockPlug *plug)
{
	GDateTime *datetime;
	gchar *time;
	gchar *date;
	gchar *tfmt;
	gchar *dfmt;

	gchar *str;

	GSettings *settings;

	settings = g_settings_new (GSETTINGS_SCHEMA);
	tfmt = g_settings_get_string (settings, KEY_LOCK_DIALOG_T_FMT);
	dfmt = g_settings_get_string (settings, KEY_LOCK_DIALOG_D_FMT);
	g_object_unref (settings);

	/* Time/Date formating https://developer.gnome.org/glib/stable/glib-GDateTime.html#g-date-time-format */

	datetime = g_date_time_new_now_local ();
	if (g_strcmp0(tfmt, "locale") == 0)
	{
		// Use locale default format
		time = g_date_time_format (datetime, "%X");
	}
	else
	{
		// Apply user defined format
		time = g_date_time_format (datetime, tfmt);
	}

	if (g_strcmp0(dfmt, "locale") == 0)
	{
		// Use locale default format
		date = g_date_time_format (datetime, _("%A, %B %e"));
	}
	else
	{
		// Apply user defined format
		date = g_date_time_format (datetime, dfmt);
	}

	str = g_strdup_printf ("<span size=\"xx-large\" weight=\"ultrabold\">%s</span>", time);
	gtk_label_set_text (GTK_LABEL (plug->priv->auth_time_label), str);
	gtk_label_set_use_markup (GTK_LABEL (plug->priv->auth_time_label), TRUE);
	g_free (str);

	str = g_strdup_printf ("<span size=\"large\">%s</span>", date);
	gtk_label_set_markup (GTK_LABEL (plug->priv->auth_date_label), str);
	gtk_label_set_use_markup (GTK_LABEL (plug->priv->auth_date_label), TRUE);
	g_free (str);

	g_free (time);
	g_free (date);
	g_free (tfmt);
	g_free (dfmt);
	g_date_time_unref (datetime);

	return G_SOURCE_CONTINUE;
}

void
gs_lock_plug_set_sensitive (GSLockPlug *plug,
                            gboolean    sensitive)
{
	g_return_if_fail (GS_IS_LOCK_PLUG (plug));

	gtk_widget_set_sensitive (plug->priv->auth_prompt_entry, sensitive);
	gtk_widget_set_sensitive (plug->priv->auth_action_area, sensitive);
}

static void
remove_datetime_timeout (GSLockPlug *plug)
{
	if (plug->priv->datetime_timeout_id > 0)
	{
		g_source_remove (plug->priv->datetime_timeout_id);
		plug->priv->datetime_timeout_id = 0;
	}
}

static void
remove_cancel_timeout (GSLockPlug *plug)
{
	if (plug->priv->cancel_timeout_id > 0)
	{
		g_source_remove (plug->priv->cancel_timeout_id);
		plug->priv->cancel_timeout_id = 0;
	}
}

static void
remove_response_idle (GSLockPlug *plug)
{
	if (plug->priv->response_idle_id > 0)
	{
		g_source_remove (plug->priv->response_idle_id);
		plug->priv->response_idle_id = 0;
	}
}

static void
gs_lock_plug_response (GSLockPlug *plug,
                       gint        response_id)
{
	int new_response;

	new_response = response_id;

	g_return_if_fail (GS_IS_LOCK_PLUG (plug));

	/* Act only on response IDs we recognize */
	if (!(response_id == GS_LOCK_PLUG_RESPONSE_OK
	        || response_id == GS_LOCK_PLUG_RESPONSE_CANCEL))
	{
		return;
	}

	remove_cancel_timeout (plug);
	remove_response_idle (plug);

	if (response_id == GS_LOCK_PLUG_RESPONSE_CANCEL)
	{
		gtk_entry_set_text (GTK_ENTRY (plug->priv->auth_prompt_entry), "");
	}

	g_signal_emit (plug,
	               lock_plug_signals [RESPONSE],
	               0,
	               new_response);
}

static gboolean
response_cancel_idle_cb (gpointer user_data)
{
	GSLockPlug *plug = user_data;

	plug->priv->response_idle_id = 0;

	gs_lock_plug_response (plug, GS_LOCK_PLUG_RESPONSE_CANCEL);

	return FALSE;
}

static gboolean
dialog_timed_out (gpointer user_data)
{
	GSLockPlug *plug = user_data;

	gs_lock_plug_set_sensitive (plug, FALSE);
	set_status_text (plug, _("Time has expired."));

	if (plug->priv->response_idle_id != 0)
	{
		g_warning ("Response idle ID already set but shouldn't be");
	}

	remove_response_idle (plug);

	plug->priv->response_idle_id = g_timeout_add (2000,
	                                              response_cancel_idle_cb,
	                                              plug);
	return FALSE;
}

static void
capslock_update (GSLockPlug *plug,
                 gboolean    is_on)
{

	plug->priv->caps_lock_on = is_on;

	if (plug->priv->auth_capslock_label == NULL)
	{
		return;
	}

	if (is_on)
	{
		gtk_label_set_text (GTK_LABEL (plug->priv->auth_capslock_label),
		                    _("You have the Caps Lock key on."));
	}
	else
	{
		gtk_label_set_text (GTK_LABEL (plug->priv->auth_capslock_label),
		                    "");
	}
}

static gboolean
is_capslock_on (void)
{
	GdkKeymap *keymap;
	gboolean res;

	res = FALSE;

	keymap = gdk_keymap_get_for_display (gdk_display_get_default ());
	if (keymap != NULL) {
		res = gdk_keymap_get_caps_lock_state (keymap);
	}

	return res;
}

static void
restart_cancel_timeout (GSLockPlug *plug)
{
	remove_cancel_timeout (plug);

	plug->priv->cancel_timeout_id = g_timeout_add (plug->priv->timeout,
	                                               dialog_timed_out,
	                                               plug);
}

void
gs_lock_plug_get_text (GSLockPlug *plug,
                       char      **text)
{
	const char *typed_text;
	char       *null_text;
	char       *local_text;

	typed_text = gtk_entry_get_text (GTK_ENTRY (plug->priv->auth_prompt_entry));
	local_text = g_locale_from_utf8 (typed_text, strlen (typed_text), NULL, NULL, NULL);

	null_text = g_strnfill (strlen (typed_text) + 1, '\b');
	gtk_entry_set_text (GTK_ENTRY (plug->priv->auth_prompt_entry), null_text);
	gtk_entry_set_text (GTK_ENTRY (plug->priv->auth_prompt_entry), "");
	g_free (null_text);

	if (text != NULL)
	{
		*text = local_text;
	}
	else
	{
		g_free (local_text);
	}
}

typedef struct
{
	gint response_id;
	GMainLoop *loop;
	gboolean destroyed;
} RunInfo;

static void
shutdown_loop (RunInfo *ri)
{
	if (g_main_loop_is_running (ri->loop))
		g_main_loop_quit (ri->loop);
}

static void
run_response_handler (GSLockPlug *plug G_GNUC_UNUSED,
                      gint response_id,
                      gpointer data)
{
	RunInfo *ri;

	ri = data;

	ri->response_id = response_id;

	shutdown_loop (ri);
}

static void
run_destroy_handler (GSLockPlug *plug G_GNUC_UNUSED,
                     gpointer data)
{
	RunInfo *ri = data;

	ri->destroyed = TRUE;
	shutdown_loop (ri);
}

static void
run_keymap_handler (GdkKeymap *keymap G_GNUC_UNUSED,
                    GSLockPlug *plug)
{
	capslock_update (plug, is_capslock_on ());
}

/* adapted from GTK+ gtkdialog.c. A lock window is neither modal nor deleted, so only
 * the response and the plug's destruction end the run. */
int
gs_lock_plug_run (GSLockPlug *plug)
{
	RunInfo ri = { GTK_RESPONSE_NONE, NULL, FALSE };
	gulong response_handler;
	gulong destroy_handler;
	gulong keymap_handler;
	GdkKeymap *keymap;

	g_return_val_if_fail (GS_IS_LOCK_PLUG (plug), -1);

	g_object_ref (plug);

	if (!gtk_widget_get_visible (GTK_WIDGET (plug)))
	{
		gtk_widget_show (GTK_WIDGET (plug));
	}

	keymap = gdk_keymap_get_for_display (gtk_widget_get_display (GTK_WIDGET (plug)));

	keymap_handler =
	    g_signal_connect (keymap,
	                      "state-changed",
	                      G_CALLBACK (run_keymap_handler),
	                      plug);

	response_handler =
	    g_signal_connect (plug,
	                      "response",
	                      G_CALLBACK (run_response_handler),
	                      &ri);

	destroy_handler =
	    g_signal_connect (plug,
	                      "destroy",
	                      G_CALLBACK (run_destroy_handler),
	                      &ri);

	ri.loop = g_main_loop_new (NULL, FALSE);

	g_main_loop_run (ri.loop);

	g_main_loop_unref (ri.loop);

	ri.loop = NULL;

	if (!ri.destroyed)
	{
		g_signal_handler_disconnect (plug, response_handler);
		g_signal_handler_disconnect (plug, destroy_handler);
	}
	g_signal_handler_disconnect (keymap, keymap_handler);

	g_object_unref (plug);

	return ri.response_id;
}

static cairo_surface_t *
surface_from_pixbuf (GdkPixbuf *pixbuf,
                     double     scale)
{
	cairo_surface_t *surface;
	cairo_t         *cr;

	surface = cairo_image_surface_create (gdk_pixbuf_get_has_alpha (pixbuf) ?
	                                      CAIRO_FORMAT_ARGB32 : CAIRO_FORMAT_RGB24,
	                                      gdk_pixbuf_get_width (pixbuf) * scale,
	                                      gdk_pixbuf_get_height (pixbuf) * scale);
	cr = cairo_create (surface);
	cairo_scale (cr, scale, scale);
	gdk_cairo_set_source_pixbuf (cr, pixbuf, 0, 0);
	cairo_paint (cr);
	cairo_destroy (cr);

	return surface;
}

static void
rounded_rectangle (cairo_t *cr,
                   gdouble  aspect,
                   gdouble  x,
                   gdouble  y,
                   gdouble  corner_radius,
                   gdouble  width,
                   gdouble  height)
{
	gdouble radius;
	gdouble degrees;

	radius = corner_radius / aspect;
	degrees = G_PI / 180.0;

	cairo_new_sub_path (cr);
	cairo_arc (cr,
	           x + width - radius,
	           y + radius,
	           radius,
	           -90 * degrees,
	           0 * degrees);
	cairo_arc (cr,
	           x + width - radius,
	           y + height - radius,
	           radius,
	           0 * degrees,
	           90 * degrees);
	cairo_arc (cr,
	           x + radius,
	           y + height - radius,
	           radius,
	           90 * degrees,
	           180 * degrees);
	cairo_arc (cr,
	           x + radius,
	           y + radius,
	           radius,
	           180 * degrees,
	           270 * degrees);
	cairo_close_path (cr);
}

static cairo_surface_t *
frame_surface (cairo_surface_t *source)
{
	cairo_surface_t *dest;
	cairo_t         *cr;
	guint            w;
	guint            h;
	int              frame_width;
	double           radius;

	frame_width = 5;

	w = cairo_image_surface_get_width (source) + frame_width * 2;
	h = cairo_image_surface_get_height (source) + frame_width * 2;
	radius = w / 10;

	dest = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
	                                   w,
	                                   h);
	cr = cairo_create (dest);

	/* set up image */
	cairo_rectangle (cr, 0, 0, w, h);
	cairo_set_source_rgba (cr, 1.0, 1.0, 1.0, 0.0);
	cairo_fill (cr);

	rounded_rectangle (cr,
					   1.0,
					   frame_width + 0.5,
					   frame_width + 0.5,
					   radius,
					   w - frame_width * 2 - 1,
					   h - frame_width * 2 - 1);
	cairo_set_source_rgba (cr, 0.5, 0.5, 0.5, 0.3);
	cairo_fill_preserve (cr);

	cairo_set_source_surface (cr, source, frame_width, frame_width);
	cairo_fill (cr);

	cairo_destroy (cr);

	return dest;
}

/* end copied from gdm-user.c */

static void
image_set_from_surface (GtkImage        *image,
                        cairo_surface_t *source,
                        int              scale_factor)
{
	cairo_surface_t *surface;

	surface = frame_surface (source);
	cairo_surface_set_device_scale (surface, scale_factor, scale_factor);
	gtk_image_set_from_surface (image, surface);
	cairo_surface_destroy (surface);

}

static gboolean
check_user_file (const gchar *filename,
                 uid_t        user,
                 gssize       max_file_size,
                 gboolean     relax_group,
                 gboolean     relax_other)
{
	struct stat fileinfo;

	if (max_file_size < 0)
	{
		max_file_size = G_MAXSIZE;
	}

	/* Exists/Readable? */
	if (g_stat (filename, &fileinfo) < 0)
	{
		return FALSE;
	}

	/* Is a regular file */
	if (G_UNLIKELY (!S_ISREG (fileinfo.st_mode)))
	{
		return FALSE;
	}

	/* Owned by user? */
	if (G_UNLIKELY (fileinfo.st_uid != user))
	{
		return FALSE;
	}

	/* Group not writable or relax_group? */
	if (G_UNLIKELY ((fileinfo.st_mode & S_IWGRP) == S_IWGRP && !relax_group))
	{
		return FALSE;
	}

	/* Other not writable or relax_other? */
	if (G_UNLIKELY ((fileinfo.st_mode & S_IWOTH) == S_IWOTH && !relax_other))
	{
		return FALSE;
	}

	/* Size is kosher? */
	if (G_UNLIKELY (fileinfo.st_size > max_file_size))
	{
		return FALSE;
	}

	return TRUE;
}

static gboolean
set_face_image (GSLockPlug *plug)
{
	GdkPixbuf       *pixbuf;
	cairo_surface_t *surface;
	const char      *homedir;
	char            *path;
	int              icon_size = 96;
	gsize            user_max_file = 1048576;
	uid_t            uid;
	int              scale_factor;
	int              width;
	int              height;
	double           scale;

	homedir = g_get_home_dir ();
	uid = getuid ();

	path = g_build_filename (homedir, ".face", NULL);

	pixbuf = NULL;
	if (check_user_file (path, uid, user_max_file, 0, 0))
	{
		pixbuf = gdk_pixbuf_new_from_file (path, NULL);
	}

	g_free (path);

	if (pixbuf == NULL)
	{
		return FALSE;
	}

	scale_factor = gtk_widget_get_scale_factor (GTK_WIDGET (plug->priv->auth_face_image));

	width = gdk_pixbuf_get_width (pixbuf);
	height = gdk_pixbuf_get_height (pixbuf);

	scale = 1.0;
	if (width > icon_size || height > icon_size)
	{
		double scale_x = (double) icon_size / width;
		double scale_y = (double) icon_size / height;
		scale = MIN (scale_x, scale_y) * scale_factor;
	}

	surface = surface_from_pixbuf (pixbuf, scale);
	g_object_unref (pixbuf);

	image_set_from_surface (GTK_IMAGE (plug->priv->auth_face_image), surface, scale_factor);
	cairo_surface_destroy (surface);

	return TRUE;
}

static void
gs_lock_plug_show (GtkWidget *widget)
{
	GSLockPlug *plug = GS_LOCK_PLUG (widget);

	if (GTK_WIDGET_CLASS (gs_lock_plug_parent_class)->show)
	{
		GTK_WIDGET_CLASS (gs_lock_plug_parent_class)->show (widget);
	}

	if (plug->priv->auth_face_image)
	{
		set_face_image (plug);
	}

	capslock_update (plug, is_capslock_on ());

	restart_cancel_timeout (plug);
}

static void
gs_lock_plug_hide (GtkWidget *widget)
{
	if (GTK_WIDGET_CLASS (gs_lock_plug_parent_class)->hide)
	{
		GTK_WIDGET_CLASS (gs_lock_plug_parent_class)->hide (widget);
	}
}

static void
queue_key_event (GSLockPlug  *plug,
                 GdkEventKey *event)
{
	GdkEvent *saved_event;

	saved_event = gdk_event_copy ((GdkEvent *)event);
	plug->priv->key_events = g_list_prepend (plug->priv->key_events,
	                         saved_event);
}

static void
forward_key_events (GSLockPlug *plug)
{
	plug->priv->key_events = g_list_reverse (plug->priv->key_events);
	while (plug->priv->key_events != NULL)
	{
		GdkEventKey *event = plug->priv->key_events->data;

		if (plug->priv->toplevel != NULL)
		{
			gtk_window_propagate_key_event (GTK_WINDOW (plug->priv->toplevel), event);
		}

		gdk_event_free ((GdkEvent *)event);

		plug->priv->key_events = g_list_delete_link (plug->priv->key_events,
		                         plug->priv->key_events);
	}
}

static void
gs_lock_plug_set_logout_enabled (GSLockPlug *plug,
                                 gboolean    logout_enabled)
{
	g_return_if_fail (GS_LOCK_PLUG (plug));

	if (plug->priv->logout_enabled == logout_enabled)
	{
		return;
	}

	plug->priv->logout_enabled = logout_enabled;
	g_object_notify (G_OBJECT (plug), "logout-enabled");

	if (plug->priv->auth_logout_button == NULL)
	{
		return;
	}

	if (logout_enabled)
	{
		gtk_widget_show (plug->priv->auth_logout_button);
	}
	else
	{
		gtk_widget_hide (plug->priv->auth_logout_button);
	}
}

static void
gs_lock_plug_set_logout_command (GSLockPlug *plug,
                                 const char *command)
{
	g_return_if_fail (GS_LOCK_PLUG (plug));

	g_free (plug->priv->logout_command);

	if (command)
	{
		plug->priv->logout_command = g_strdup (command);
	}
	else
	{
		plug->priv->logout_command = NULL;
	}
}

static void
gs_lock_plug_set_status_message (GSLockPlug *plug,
                                 const char *status_message)
{
	g_return_if_fail (GS_LOCK_PLUG (plug));

	g_free (plug->priv->status_message);
	plug->priv->status_message = g_strdup (status_message);

	if (plug->priv->status_message_label)
	{
		if (plug->priv->status_message)
		{
			gtk_label_set_text (GTK_LABEL (plug->priv->status_message_label),
			                    plug->priv->status_message);
			gtk_widget_show (plug->priv->status_message_label);
		}
		else
		{
			gtk_widget_hide (plug->priv->status_message_label);
		}
	}
}

static void
gs_lock_plug_get_property (GObject    *object,
                           guint       prop_id,
                           GValue     *value,
                           GParamSpec *pspec)
{
	GSLockPlug *self;

	self = GS_LOCK_PLUG (object);

	switch (prop_id)
	{
	case PROP_LOGOUT_ENABLED:
		g_value_set_boolean (value, self->priv->logout_enabled);
		break;
	case PROP_LOGOUT_COMMAND:
		g_value_set_string (value, self->priv->logout_command);
		break;
	case PROP_SWITCH_ENABLED:
		g_value_set_boolean (value, self->priv->switch_enabled);
		break;
	case PROP_STATUS_MESSAGE:
		g_value_set_string (value, self->priv->status_message);
		break;
	default:
		G_OBJECT_WARN_INVALID_PROPERTY_ID (object, prop_id, pspec);
		break;
	}
}

static void
gs_lock_plug_set_switch_enabled (GSLockPlug *plug,
                                 gboolean    switch_enabled)
{
	g_return_if_fail (GS_LOCK_PLUG (plug));

	if (plug->priv->switch_enabled == switch_enabled)
	{
		return;
	}

	plug->priv->switch_enabled = switch_enabled;
	g_object_notify (G_OBJECT (plug), "switch-enabled");

	if (plug->priv->auth_switch_button == NULL)
	{
		return;
	}

	if (switch_enabled)
	{
		if (process_is_running ("mdm"))
		{
			/* MDM  */
			gtk_widget_show (plug->priv->auth_switch_button);
		}
		else if (process_is_running ("gdm") || process_is_running("gdm3") || process_is_running("gdm-binary"))
		{
			/* GDM */
			gtk_widget_show (plug->priv->auth_switch_button);
		}
		else if (g_getenv ("XDG_SEAT_PATH") != NULL)
		{
			/* LightDM */
			gtk_widget_show (plug->priv->auth_switch_button);
		}
		else
		{
			gs_debug ("Warning: Unknown DM for switch button");
			gtk_widget_hide (plug->priv->auth_switch_button);
		}
	}
	else
	{
		gtk_widget_hide (plug->priv->auth_switch_button);
	}
}

static void
gs_lock_plug_set_property (GObject            *object,
                           guint               prop_id,
                           const GValue       *value,
                           GParamSpec         *pspec)
{
	GSLockPlug *self;

	self = GS_LOCK_PLUG (object);

	switch (prop_id)
	{
	case PROP_LOGOUT_ENABLED:
		gs_lock_plug_set_logout_enabled (self, g_value_get_boolean (value));
		break;
	case PROP_LOGOUT_COMMAND:
		gs_lock_plug_set_logout_command (self, g_value_get_string (value));
		break;
	case PROP_STATUS_MESSAGE:
		gs_lock_plug_set_status_message (self, g_value_get_string (value));
		break;
	case PROP_SWITCH_ENABLED:
		gs_lock_plug_set_switch_enabled (self, g_value_get_boolean (value));
		break;
	default:
		G_OBJECT_WARN_INVALID_PROPERTY_ID (object, prop_id, pspec);
		break;
	}
}

/* The plug's delete_event handler cancelled, so closing it cancels. */
static void
gs_lock_plug_close (GSLockPlug *plug)
{
	gs_lock_plug_response (plug, GS_LOCK_PLUG_RESPONSE_CANCEL);
}

static void
set_default_button (GSLockPlug *plug,
                    GtkWidget  *button)
{
	plug->priv->default_button = button;

	if (plug->priv->toplevel != NULL)
	{
		gtk_window_set_default (GTK_WINDOW (plug->priv->toplevel), button);
	}
}

static gboolean
toplevel_key_press (GtkWidget   *toplevel,
                    GdkEventKey *event,
                    GSLockPlug  *plug);

/* The plug was a window, and it's now in one: the lock window it moved to gets its
 * default button, its handler of keys typed while the prompt isn't ready, and the
 * keyboard focus on the prompt. */
static void
gs_lock_plug_hierarchy_changed (GtkWidget *widget,
                                GtkWidget *previous_toplevel G_GNUC_UNUSED)
{
	GSLockPlug *plug = GS_LOCK_PLUG (widget);
	GtkWidget  *toplevel;

	toplevel = gtk_widget_get_toplevel (widget);
	if (! gtk_widget_is_toplevel (toplevel))
	{
		toplevel = NULL;
	}

	if (toplevel == plug->priv->toplevel)
	{
		return;
	}

	if (plug->priv->toplevel != NULL)
	{
		g_signal_handler_disconnect (plug->priv->toplevel, plug->priv->toplevel_key_press_id);
		g_object_remove_weak_pointer (G_OBJECT (plug->priv->toplevel),
		                              (gpointer *) &plug->priv->toplevel);
	}

	plug->priv->toplevel = toplevel;
	plug->priv->toplevel_key_press_id = 0;

	if (toplevel == NULL)
	{
		return;
	}

	g_object_add_weak_pointer (G_OBJECT (toplevel), (gpointer *) &plug->priv->toplevel);
	plug->priv->toplevel_key_press_id =
	    g_signal_connect (toplevel, "key-press-event", G_CALLBACK (toplevel_key_press), plug);

	set_default_button (plug, plug->priv->default_button);

	if (gtk_widget_is_sensitive (plug->priv->auth_prompt_entry))
	{
		gtk_widget_grab_focus (plug->priv->auth_prompt_entry);
	}
}

static void
gs_lock_plug_class_init (GSLockPlugClass *klass)
{
	GObjectClass   *object_class = G_OBJECT_CLASS (klass);
	GtkWidgetClass *widget_class = GTK_WIDGET_CLASS (klass);
	GtkBindingSet  *binding_set;

	object_class->finalize     = gs_lock_plug_finalize;
	object_class->get_property = gs_lock_plug_get_property;
	object_class->set_property = gs_lock_plug_set_property;

	widget_class->show                           = gs_lock_plug_show;
	widget_class->hide                           = gs_lock_plug_hide;
	widget_class->hierarchy_changed              = gs_lock_plug_hierarchy_changed;

	klass->close = gs_lock_plug_close;

	lock_plug_signals [RESPONSE] = g_signal_new ("response",
	                               G_OBJECT_CLASS_TYPE (klass),
	                               G_SIGNAL_RUN_LAST,
	                               G_STRUCT_OFFSET (GSLockPlugClass, response),
	                               NULL, NULL,
	                               g_cclosure_marshal_VOID__INT,
	                               G_TYPE_NONE, 1,
	                               G_TYPE_INT);
	lock_plug_signals [CLOSE] = g_signal_new ("close",
	                            G_OBJECT_CLASS_TYPE (klass),
	                            G_SIGNAL_RUN_LAST | G_SIGNAL_ACTION,
	                            G_STRUCT_OFFSET (GSLockPlugClass, close),
	                            NULL, NULL,
	                            g_cclosure_marshal_VOID__VOID,
	                            G_TYPE_NONE, 0);

	g_object_class_install_property (object_class,
	                                 PROP_LOGOUT_ENABLED,
	                                 g_param_spec_boolean ("logout-enabled",
	                                         NULL,
	                                         NULL,
	                                         FALSE,
	                                         G_PARAM_READWRITE));
	g_object_class_install_property (object_class,
	                                 PROP_LOGOUT_COMMAND,
	                                 g_param_spec_string ("logout-command",
	                                         NULL,
	                                         NULL,
	                                         NULL,
	                                         G_PARAM_READWRITE));
	g_object_class_install_property (object_class,
	                                 PROP_STATUS_MESSAGE,
	                                 g_param_spec_string ("status-message",
	                                         NULL,
	                                         NULL,
	                                         NULL,
	                                         G_PARAM_READWRITE));
	g_object_class_install_property (object_class,
	                                 PROP_SWITCH_ENABLED,
	                                 g_param_spec_boolean ("switch-enabled",
	                                         NULL,
	                                         NULL,
	                                         FALSE,
	                                         G_PARAM_READWRITE));

	binding_set = gtk_binding_set_by_class (klass);

	gtk_binding_entry_add_signal (binding_set, GDK_KEY_Escape, 0,
	                              "close", 0);
}

static void
clear_clipboards (GSLockPlug *plug)
{
	GtkClipboard *clipboard;

	clipboard = gtk_widget_get_clipboard (GTK_WIDGET (plug), GDK_SELECTION_PRIMARY);
	gtk_clipboard_clear (clipboard);
	gtk_clipboard_set_text (clipboard, "", -1);
	clipboard = gtk_widget_get_clipboard (GTK_WIDGET (plug), GDK_SELECTION_CLIPBOARD);
	gtk_clipboard_clear (clipboard);
	gtk_clipboard_set_text (clipboard, "", -1);
}

static void
logout_button_clicked (GtkButton  *button G_GNUC_UNUSED,
                       GSLockPlug *plug)
{
	char   **argv  = NULL;
	GError  *error = NULL;
	gboolean res;

	if (! plug->priv->logout_command)
	{
		return;
	}

	res = g_shell_parse_argv (plug->priv->logout_command, NULL, &argv, &error);

	if (! res)
	{
		g_warning ("Could not parse logout command: %s", error->message);
		g_error_free (error);
		return;
	}

	g_spawn_async (g_get_home_dir (),
	               argv,
	               NULL,
	               G_SPAWN_SEARCH_PATH,
	               NULL,
	               NULL,
	               NULL,
	               &error);

	g_strfreev (argv);

	if (error)
	{
		g_warning ("Could not run logout command: %s", error->message);
		g_error_free (error);
	}
}

/* On the lock window the plug is in, once that has a GdkWindow. */
static void
set_cursor (GSLockPlug    *plug,
            GdkCursorType  cursor_type)
{
	GdkDisplay *display;
	GdkCursor *cursor;
	GdkWindow *window;

	if (plug->priv->toplevel == NULL)
	{
		return;
	}

	window = gtk_widget_get_window (plug->priv->toplevel);
	if (window == NULL)
	{
		return;
	}

	display = gtk_widget_get_display (GTK_WIDGET (plug));
	cursor = gdk_cursor_new_for_display (display, cursor_type);
	gdk_window_set_cursor (window, cursor);
	g_object_unref (cursor);
}

void
gs_lock_plug_set_busy (GSLockPlug *plug)
{
	set_cursor (plug, GDK_WATCH);
}

void
gs_lock_plug_set_ready (GSLockPlug *plug)
{
	set_cursor (plug, GDK_LEFT_PTR);
}

void
gs_lock_plug_enable_prompt (GSLockPlug *plug,
                            const char *message,
                            gboolean    visible)
{
	g_return_if_fail (GS_IS_LOCK_PLUG (plug));

	gs_debug ("Setting prompt to: %s", message);

	gtk_widget_set_sensitive (plug->priv->auth_unlock_button, TRUE);
	gtk_widget_show (plug->priv->auth_unlock_button);
	set_default_button (plug, plug->priv->auth_unlock_button);
	gtk_label_set_text (GTK_LABEL (plug->priv->auth_prompt_label), message);
	gtk_widget_show (plug->priv->auth_prompt_label);
	gtk_entry_set_visibility (GTK_ENTRY (plug->priv->auth_prompt_entry), visible);
	gtk_widget_set_sensitive (plug->priv->auth_prompt_entry, TRUE);
	gtk_widget_show (plug->priv->auth_prompt_entry);

	if (! gtk_widget_has_focus (plug->priv->auth_prompt_entry))
	{
		gtk_widget_grab_focus (plug->priv->auth_prompt_entry);
	}

	/* were there any key events sent to the plug while the
	 * entry wasnt ready? If so, forward them along
	 */
	forward_key_events (plug);

	restart_cancel_timeout (plug);
}

void
gs_lock_plug_disable_prompt (GSLockPlug *plug)
{
	g_return_if_fail (GS_IS_LOCK_PLUG (plug));

	/* gtk_widget_hide (plug->priv->auth_prompt_entry); */
	/* gtk_widget_hide (plug->priv->auth_prompt_label); */
	gtk_widget_set_sensitive (plug->priv->auth_unlock_button, FALSE);
	gtk_widget_set_sensitive (plug->priv->auth_prompt_entry, FALSE);
	/* gtk_widget_hide (plug->priv->auth_unlock_button); */

	set_default_button (plug, plug->priv->auth_cancel_button);
}

void
gs_lock_plug_show_message (GSLockPlug *plug,
                           const char *message)
{
	g_return_if_fail (GS_IS_LOCK_PLUG (plug));

	set_status_text (plug, message ? message : "");
}

/* button press handler used to inhibit popup menu */
static gint
entry_button_press (GtkWidget      *widget G_GNUC_UNUSED,
                    GdkEventButton *event)
{
	if (event->button == 3 && event->type == GDK_BUTTON_PRESS)
	{
		return TRUE;
	}

	return FALSE;
}

static gboolean
toplevel_key_press (GtkWidget   *toplevel G_GNUC_UNUSED,
                    GdkEventKey *event,
                    GSLockPlug  *plug)
{
	restart_cancel_timeout (plug);

	/* if the input widget is visible and ready for input
	 * then just carry on as usual
	 */
	if (gtk_widget_get_visible (plug->priv->auth_prompt_entry) &&
	        gtk_widget_is_sensitive (plug->priv->auth_prompt_entry))
	{
		return FALSE;
	}

	if (strcmp (event->string, "") == 0)
	{
		return FALSE;
	}

	queue_key_event (plug, event);

	return TRUE;
}

static char *
get_user_display_name (void)
{
	const char *name;
	char       *utf8_name;

	name = g_get_real_name ();

	if (name == NULL || g_strcmp0 (name, "") == 0 ||
	    g_strcmp0 (name, "Unknown") == 0)
	{
		name = g_get_user_name ();
	}

	utf8_name = NULL;

	if (name != NULL)
	{
		utf8_name = g_locale_to_utf8 (name, -1, NULL, NULL, NULL);
	}

	return utf8_name;
}

static char *
get_user_name (void)
{
	const char *name;
	char       *utf8_name;

	name = g_get_user_name ();

	utf8_name = NULL;
	if (name != NULL)
	{
		utf8_name = g_locale_to_utf8 (name, -1, NULL, NULL, NULL);
	}

	return utf8_name;
}

/* adapted from MDM */
static char *
expand_string (const char *text)
{
	GString       *str;
	const char    *p;
	char          *username;
	glong          i;
	glong          n_chars;
	struct utsname name;

	str = g_string_sized_new (strlen (text));

	p = text;
	n_chars = g_utf8_strlen (text, -1);
	i = 0;

	while (i < n_chars)
	{
		gunichar ch;

		ch = g_utf8_get_char (p);

		/* Backslash commands */
		if (ch == '\\')
		{
			p = g_utf8_next_char (p);
			i++;
			ch = g_utf8_get_char (p);

			if (i >= n_chars || ch == '\0')
			{
				g_warning ("Unescaped \\ at end of text\n");
				goto bail;
			}
			else if (ch == 'n')
			{
				g_string_append_unichar (str, '\n');
			}
			else
			{
				g_string_append_unichar (str, ch);
			}
		}
		else if (ch == '%')
		{
			p = g_utf8_next_char (p);
			i++;
			ch = g_utf8_get_char (p);

			if (i >= n_chars || ch == '\0')
			{
				g_warning ("Unescaped %% at end of text\n");
				goto bail;
			}

			switch (ch)
			{
			case '%':
				g_string_append (str, "%");
				break;
			case 'c':
				/* clock */
				break;
			case 'd':
				/* display */
				g_string_append (str, g_getenv ("DISPLAY"));
				break;
			case 'h':
				/* hostname */
				g_string_append (str, g_get_host_name ());
				break;
			case 'm':
				/* machine name */
				uname (&name);
				g_string_append (str, name.machine);
				break;
			case 'n':
				/* nodename */
				uname (&name);
				g_string_append (str, name.nodename);
				break;
			case 'r':
				/* release */
				uname (&name);
				g_string_append (str, name.release);
				break;
			case 'R':
				/* Real name */
				username = get_user_display_name ();
				g_string_append (str, username);
				g_free (username);
				break;
			case 's':
				/* system name */
				uname (&name);
				g_string_append (str, name.sysname);
				break;
			case 'U':
				/* Username */
				username = get_user_name ();
				g_string_append (str, username);
				g_free (username);
				break;
			default:
				if (ch < 127)
				{
					g_warning ("unknown escape code %%%c in text\n", (char)ch);
				}
				else
				{
					g_warning ("unknown escape code %%(U%x) in text\n", (int)ch);
				}
			}
		}
		else
		{
			g_string_append_unichar (str, ch);
		}
		p = g_utf8_next_char (p);
		i++;
	}

bail:

	return g_string_free (str, FALSE);
}

static void
expand_string_for_label (GtkWidget *label)
{
	const char *template;
	char       *str;

	template = gtk_label_get_label (GTK_LABEL (label));
	str = expand_string (template);
	gtk_label_set_label (GTK_LABEL (label), str);
	g_free (str);
}

static void
unlock_button_clicked (GtkButton  *button G_GNUC_UNUSED,
                       GSLockPlug *plug)
{
	gs_lock_plug_response (plug, GS_LOCK_PLUG_RESPONSE_OK);
}

static void
entry_activate_handler (GtkEntry   *entry,
                        GSLockPlug *plug G_GNUC_UNUSED)
{
	/* Disable input widget to prevent characters typed
	 * after Enter from being included in the password. */
	gtk_widget_set_sensitive (GTK_WIDGET (entry), FALSE);
}

static void
cancel_button_clicked (GtkButton  *button G_GNUC_UNUSED,
                       GSLockPlug *plug)
{
	gs_lock_plug_response (plug, GS_LOCK_PLUG_RESPONSE_CANCEL);
}

static void
switch_user_button_clicked (GtkButton  *button G_GNUC_UNUSED,
                            GSLockPlug *plug)
{

	remove_response_idle (plug);

	gs_lock_plug_set_sensitive (plug, FALSE);

	plug->priv->response_idle_id = g_timeout_add (2000,
	                                              response_cancel_idle_cb,
	                                              plug);

	gs_lock_plug_set_busy (plug);
	do_user_switch (plug);
}

static char *
get_dialog_theme_name (void)
{
	char        *name;
	GSettings *settings;

	settings = g_settings_new (GSETTINGS_SCHEMA);
	name = g_settings_get_string (settings, KEY_LOCK_DIALOG_THEME);
	g_object_unref (settings);

	return name;
}

/* A lock surface can't show a popup (gtk-session-lock's README), so no widget of the
 * theme's has a tooltip: lock-dialog-default.ui's entry has one on its icon. */
static void
remove_tooltips (GtkWidget *widget,
                 gpointer   data G_GNUC_UNUSED)
{
	if (GTK_IS_ENTRY (widget))
	{
		gtk_entry_set_icon_tooltip_text (GTK_ENTRY (widget), GTK_ENTRY_ICON_PRIMARY, NULL);
		gtk_entry_set_icon_tooltip_text (GTK_ENTRY (widget), GTK_ENTRY_ICON_SECONDARY, NULL);
	}

	gtk_widget_set_has_tooltip (widget, FALSE);

	if (GTK_IS_CONTAINER (widget))
	{
		gtk_container_forall (GTK_CONTAINER (widget), remove_tooltips, NULL);
	}
}

/* mate-screensaver's theme, lock-dialog-<lock-dialog-theme>.ui and .css from its data
 * directory. Where the .ui isn't there, caminito-locker's own lock-dialog-default.ui
 * stands in for the dialog mate-screensaver builds in code. */
static void
load_theme (GSLockPlug *plug)
{
	char       *theme;
	char       *filename;
	char       *gtkbuilder;
	char       *css;
	GtkBuilder *builder;
	GtkWidget  *lock_dialog;
	GError     *error=NULL;
	gboolean    loaded;

	theme = get_dialog_theme_name ();

	filename = g_strdup_printf ("lock-dialog-%s.ui", theme);
	gtkbuilder = g_build_filename (GTKBUILDERDIR, filename, NULL);
	g_free (filename);

	filename = g_strdup_printf ("lock-dialog-%s.css", theme);
	g_free (theme);
	css = g_build_filename (GTKBUILDERDIR, filename, NULL);
	g_free (filename);

	builder = gtk_builder_new();
	loaded = FALSE;
	if (g_file_test (gtkbuilder, G_FILE_TEST_IS_REGULAR))
	{
		loaded = gtk_builder_add_from_file (builder, gtkbuilder, &error);
		if (! loaded)
		{
			g_warning ("Couldn't load builder file '%s': %s", gtkbuilder, error->message);
			g_clear_error (&error);
			g_object_unref (builder);
			builder = gtk_builder_new ();
		}
	}
	else
	{
		gs_debug ("No theme at %s", gtkbuilder);
	}
	g_free (gtkbuilder);

	if (loaded && g_file_test (css, G_FILE_TEST_IS_REGULAR))
	{
		static GtkCssProvider *style_provider = NULL;

		if (style_provider == NULL)
		{
			style_provider = gtk_css_provider_new ();

			gtk_style_context_add_provider_for_screen (gdk_screen_get_default(),
			                                           GTK_STYLE_PROVIDER (style_provider),
			                                           GTK_STYLE_PROVIDER_PRIORITY_USER);
		}

		gtk_css_provider_load_from_path (style_provider, css, NULL);
	}
	g_free (css);

	if (! loaded)
	{
		gtk_builder_add_from_resource (builder, DEFAULT_THEME_RESOURCE, &error);
		g_assert_no_error (error);
	}

	lock_dialog = GTK_WIDGET (gtk_builder_get_object(builder, "lock-dialog"));
	gtk_container_add (GTK_CONTAINER (plug), lock_dialog);

	plug->priv->notebook = GTK_WIDGET (gtk_builder_get_object(builder, "notebook"));

	plug->priv->auth_face_image = GTK_WIDGET (gtk_builder_get_object(builder, "auth-face-image"));
	plug->priv->auth_action_area = GTK_WIDGET (gtk_builder_get_object(builder, "auth-action-area"));
	plug->priv->auth_time_label = GTK_WIDGET (gtk_builder_get_object(builder, "auth-time-label"));
	plug->priv->auth_date_label = GTK_WIDGET (gtk_builder_get_object(builder, "auth-date-label"));
	plug->priv->auth_realname_label = GTK_WIDGET (gtk_builder_get_object(builder, "auth-realname-label"));
	plug->priv->auth_username_label = GTK_WIDGET (gtk_builder_get_object(builder, "auth-username-label"));
	plug->priv->auth_prompt_label = GTK_WIDGET (gtk_builder_get_object(builder, "auth-prompt-label"));
	plug->priv->auth_prompt_entry = GTK_WIDGET (gtk_builder_get_object(builder, "auth-prompt-entry"));
	plug->priv->auth_prompt_box = GTK_WIDGET (gtk_builder_get_object(builder, "auth-prompt-box"));
	plug->priv->auth_capslock_label = GTK_WIDGET (gtk_builder_get_object(builder, "auth-capslock-label"));
	plug->priv->auth_message_label = GTK_WIDGET (gtk_builder_get_object(builder, "auth-status-label"));
	plug->priv->auth_unlock_button = GTK_WIDGET (gtk_builder_get_object(builder, "auth-unlock-button"));
	plug->priv->auth_cancel_button = GTK_WIDGET (gtk_builder_get_object(builder, "auth-cancel-button"));
	plug->priv->auth_logout_button = GTK_WIDGET (gtk_builder_get_object(builder, "auth-logout-button"));
	plug->priv->auth_switch_button = GTK_WIDGET (gtk_builder_get_object(builder, "auth-switch-button"));
	plug->priv->auth_note_button = GTK_WIDGET (gtk_builder_get_object(builder, "auth-note-button"));

	/* Placeholder for the keyboard indicator */
	plug->priv->auth_prompt_kbd_layout_indicator = GTK_WIDGET (gtk_builder_get_object(builder, "auth-prompt-kbd-layout-indicator"));
	if (plug->priv->auth_logout_button != NULL)
	{
		gtk_widget_set_no_show_all (plug->priv->auth_logout_button, TRUE);
	}
	if (plug->priv->auth_switch_button != NULL)
	{
		gtk_widget_set_no_show_all (plug->priv->auth_switch_button, TRUE);
	}
	if (plug->priv->auth_note_button != NULL)
	{
		gtk_widget_set_no_show_all (plug->priv->auth_note_button, TRUE);
	}

	date_time_update (plug);
	remove_tooltips (lock_dialog, NULL);
	gtk_widget_show_all (lock_dialog);

	plug->priv->status_message_label = GTK_WIDGET (gtk_builder_get_object(builder, "status-message-label"));

	g_object_unref(builder);
}

static void
entry_icon_clicked (GtkEntry       *entry,
                    gint            position,
                    GdkEventButton *event,
                    gpointer        data)
{
	(void)data;

	if ((position == GTK_ENTRY_ICON_SECONDARY) && (event->type == GDK_DOUBLE_BUTTON_PRESS))
	{
		if (g_strcmp0 (gtk_entry_get_icon_name (entry, position), "emblem-readonly"))
		{
			gtk_entry_set_icon_from_icon_name (entry, position, "emblem-readonly");
			gtk_entry_set_visibility (entry, FALSE);
		}
		else
		{
			gtk_entry_set_icon_from_icon_name (entry, position, "emblem-unreadable");
			gtk_entry_set_visibility (entry, TRUE);
		}
	}
}

static void
gs_lock_plug_init (GSLockPlug *plug)
{
	plug->priv = gs_lock_plug_get_instance_private (plug);

	clear_clipboards (plug);

	GtkStyleContext *context;

	context = gtk_widget_get_style_context (GTK_WIDGET (plug));
	gtk_style_context_add_class (context, "lock-dialog");

	/* The dialog's size, in the middle of the lock window, as in mate-screensaver's. */
	gtk_widget_set_halign (GTK_WIDGET (plug), GTK_ALIGN_CENTER);
	gtk_widget_set_valign (GTK_WIDGET (plug), GTK_ALIGN_CENTER);

	load_theme (plug);
	plug->priv->datetime_timeout_id = g_timeout_add_seconds (1, G_SOURCE_FUNC (date_time_update), plug);

	/* Layout indicator: libmatekbd's, on X11 alone. */
	if (plug->priv->auth_prompt_kbd_layout_indicator != NULL)
	{
		gtk_widget_hide (plug->priv->auth_prompt_kbd_layout_indicator);
	}

	if (plug->priv->auth_note_button != NULL)
	{
		gtk_widget_hide (plug->priv->auth_note_button);
	}
	if (plug->priv->auth_switch_button != NULL)
	{
		if (plug->priv->switch_enabled)
		{
			gtk_widget_show_all (plug->priv->auth_switch_button);
		}
		else
		{
			gtk_widget_hide (plug->priv->auth_switch_button);
		}
	}

	set_default_button (plug, plug->priv->auth_unlock_button);

	if (plug->priv->auth_username_label != NULL)
	{
		expand_string_for_label (plug->priv->auth_username_label);
	}

	if (plug->priv->auth_realname_label != NULL)
	{
		expand_string_for_label (plug->priv->auth_realname_label);
	}

	if (! plug->priv->logout_enabled || ! plug->priv->logout_command)
	{
		if (plug->priv->auth_logout_button != NULL)
		{
			gtk_widget_hide (plug->priv->auth_logout_button);
		}
	}

	plug->priv->timeout = DIALOG_TIMEOUT_MSEC;

	/* button press handler used to inhibit popup menu */
	g_signal_connect (plug->priv->auth_prompt_entry, "button_press_event",
	                  G_CALLBACK (entry_button_press), NULL);
	/* capture text on Enter before default button activation */
	g_signal_connect (plug->priv->auth_prompt_entry, "activate",
	                  G_CALLBACK (entry_activate_handler), plug);
	gtk_entry_set_activates_default (GTK_ENTRY (plug->priv->auth_prompt_entry), TRUE);
	gtk_entry_set_visibility (GTK_ENTRY (plug->priv->auth_prompt_entry), FALSE);

	/* show/hide password */
	g_signal_connect (plug->priv->auth_prompt_entry, "icon-press",
	                  G_CALLBACK (entry_icon_clicked), NULL);

	g_signal_connect (plug->priv->auth_unlock_button, "clicked",
	                  G_CALLBACK (unlock_button_clicked), plug);

	g_signal_connect (plug->priv->auth_cancel_button, "clicked",
	                  G_CALLBACK (cancel_button_clicked), plug);

	if (plug->priv->status_message_label)
	{
		if (plug->priv->status_message)
		{
			gtk_label_set_text (GTK_LABEL (plug->priv->status_message_label),
			                    plug->priv->status_message);
		}
		else
		{
			gtk_widget_hide (plug->priv->status_message_label);
		}
	}

	if (plug->priv->auth_switch_button != NULL)
	{
		g_signal_connect (plug->priv->auth_switch_button, "clicked",
		                  G_CALLBACK (switch_user_button_clicked), plug);
	}

	if (plug->priv->auth_logout_button != NULL)
	{
		g_signal_connect (plug->priv->auth_logout_button, "clicked",
		                  G_CALLBACK (logout_button_clicked), plug);
	}
}

static void
gs_lock_plug_finalize (GObject *object)
{
	GSLockPlug *plug;

	g_return_if_fail (object != NULL);
	g_return_if_fail (GS_IS_LOCK_PLUG (object));

	plug = GS_LOCK_PLUG (object);

	g_return_if_fail (plug->priv != NULL);

	g_free (plug->priv->logout_command);
	g_free (plug->priv->status_message);
	g_list_free_full (plug->priv->key_events, (GDestroyNotify) gdk_event_free);

	remove_response_idle (plug);
	remove_cancel_timeout (plug);
	remove_datetime_timeout (plug);

	G_OBJECT_CLASS (gs_lock_plug_parent_class)->finalize (object);
}

GtkWidget *
gs_lock_plug_new (void)
{
	return g_object_new (GS_TYPE_LOCK_PLUG, NULL);
}
