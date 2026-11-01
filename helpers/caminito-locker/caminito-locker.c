/* -*- Mode: C; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 8 -*-
 *
 * Copyright (C) 2004-2006 William Jon McCann <mccann@jhu.edu>
 * Copyright (C) 2012-2021 MATE Developers
 * SPDX-FileCopyrightText: 2025-2026 Juan Pablo Lassala
 * SPDX-License-Identifier: GPL-3.0-or-later AND GPL-2.0-or-later
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License as
 * published by the Free Software Foundation; either version 2 of the
 * License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin St, Fifth Floor, Boston, MA
 * 02110-1301, USA.
 *
 * Authors: William Jon McCann <mccann@jhu.edu>
 *
 * caminito-locker: the lock screen's locker (12, 131, ADR 0007). It takes the session lock
 * over ext-session-lock through gtk-session-lock, and puts a lock window on every monitor,
 * with its wallpaper and a clock: the Idle view. A key or a click shows the Unlock dialog,
 * mate-screensaver's, on the monitor under the pointer (CMN-298, ADR 0056). The right
 * password unlocks and the locker exits; nothing else ends the lock.
 *
 * Ported from mate-screensaver 60b2db3's src/mate-screensaver-dialog.c, which mate-screensaver
 * runs as a GtkPlug in its own lock window and which tells the daemon how the conversation
 * went on stdout. Here the program is the lock: the conversation's outcome unlocks it, and
 * neither a cancel nor the fifth failure ends it, as mate-screensaver's daemon would end its
 * dialog. A failure starts the conversation again. A cancel or a timeout doesn't answer
 * PAM, whose pam_faillock would count it: the dialog hides, and the prompt stays open in the
 * conversation's thread for the next time it shows (CMN-256). The dialog isn't setuid, so
 * hack_uid and the privileged start-up are gone.
 *
 * What the daemon gave the dialog, the locker finds itself (132), where the daemon did:
 *  - the background, as gs-manager.c's apply_background_to_window: the desktop's through
 *    MateBG, replaced by picture-filename when that file exists, drawn under each lock
 *    window's content as gs-window-x11.c's gs_window_real_draw does. Under Caminito the
 *    desktop's is each monitor's own wallpaper, from the file caminito-wallpaperd writes
 *    (218), and org.mate.background's where the file doesn't have it;
 *  - the buttons and the status message, from gs-prefs.c's keys as gs-monitor.c's
 *    _gs_monitor_update_from_prefs gives them to the windows, and the status message from
 *    gs-watcher.c, as gs-monitor.c's on_watcher_status_message_changed;
 *  - Log Out only once logout-delay has passed, as gs-window-x11.c's is_logout_enabled. The
 *    daemon asks when it starts a dialog, and its timer starts when its windows show; the
 *    locker's dialog lasts the whole lock, so the button shows once the delay has passed
 *    since the locker started.
 * mate-screensaver applies a new status message even with status-message-enabled off,
 * which is its bug: here the key holds for a new message too.
 */

#include "config.h"

#include <langinfo.h>
#include <locale.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <glib/gi18n.h>
#include <gtk/gtk.h>
#include <gtk-session-lock.h>

#define MATE_DESKTOP_USE_UNSTABLE_API
#include <libmate-desktop/mate-bg.h>

#include "gs-lock-plug.h"
#include "gs-prefs.h"
#include "gs-watcher.h"

#include "gs-auth.h"

#define gs_debug g_debug

/* How soon a conversation starts again after a module gave up by itself in the Idle view:
 * no message has to be read, but a service that denies everyone mustn't go round at once. */
#define QUIET_RESTART_MSEC 1000

/* How far the clock stays above the dialog, or it's made smaller. */
#define CLOCK_GAP 8

static gboolean verbose = FALSE;
static gboolean show_version = FALSE;

static GOptionEntry entries[] = {
	{"verbose", 0, 0, G_OPTION_ARG_NONE, &verbose, N_("Show debugging output"), NULL},
	{"version", 0, 0, G_OPTION_ARG_NONE, &show_version, N_("Version of this application"), NULL},
	{NULL, 0, 0, G_OPTION_ARG_NONE, NULL, NULL, NULL}
};

/* The lock, its window on each monitor, and the dialog in one of them. */
static GtkSessionLockLock *lock = NULL;
static GHashTable *lock_windows = NULL;
static GtkWidget *plug = NULL;
/* Whether Caminito has said `locked`, before which the lock can't be unlocked, and whether
 * the user has authenticated, which may come first. */
static gboolean is_locked = FALSE;
static gboolean authenticated = FALSE;

/* What the daemon had: the keys, the session's status message, the background, and the
 * time since the lock windows showed, with the timeout that shows Log Out once the delay
 * has passed. */
static GSPrefs *prefs = NULL;
static GSWatcher *watcher = NULL;
static MateBG *bg = NULL;
static GTimer *timer = NULL;
static guint logout_timeout_id = 0;

/* The clock's text, and the idle that fits the clocks around the dialog. */
static char *clock_time = NULL;
static char *clock_date = NULL;
static guint fit_clocks_id = 0;

static char* request_response(GSLockPlug* plug, const char* prompt, gboolean visible)
{
	int response;
	char* text;

	gs_lock_plug_enable_prompt(plug, prompt, visible);
	response = gs_lock_plug_run(plug);

	gs_debug ("got response: %d", response);

	text = NULL;

	/* Only a submitted password is a response: the dialog's timeout and its Cancel button
	 * leave the prompt open, so the run ends for a password or the dialog's destruction. */
	if (response == GS_LOCK_PLUG_RESPONSE_OK)
	{
		gs_lock_plug_get_text(plug, &text);
	}

	gs_lock_plug_disable_prompt(plug);

	return text;
}

/* Adapted from MDM2 daemon/verify-pam.c on 2006-06-13 */
static const char* maybe_translate_message(const char* msg)
{
	char* s;
	const char* ret;
	static GHashTable* hash = NULL;

	if (hash == NULL)
	{
		/* Here we come with some fairly standard messages so that
		   we have as much as possible translated.  Should really be
		   translated in pam I suppose.  This way we can "change"
		   some of these messages to be more sane. */
		hash = g_hash_table_new (g_str_hash, g_str_equal);
		/* login: is whacked always translate to Username: */
		g_hash_table_insert(hash, "login:", _("Username:"));
		g_hash_table_insert(hash, "Username:", _("Username:"));
		g_hash_table_insert(hash, "username:", _("Username:"));
		g_hash_table_insert(hash, "Password:", _("Password:"));
		g_hash_table_insert(hash, "password:", _("Password:"));
		g_hash_table_insert(hash, "You are required to change your password immediately (password aged)", _("You are required to change your password immediately (password aged)"));
		g_hash_table_insert(hash, "You are required to change your password immediately (root enforced)", _("You are required to change your password immediately (root enforced)"));
		g_hash_table_insert(hash, "Your account has expired; please contact your system administrator", _("Your account has expired; please contact your system administrator"));
		g_hash_table_insert(hash, "No password supplied", _("No password supplied"));
		g_hash_table_insert(hash, "Password unchanged", _("Password unchanged"));
		g_hash_table_insert(hash, "Can not get username", _("Can not get username"));
		g_hash_table_insert(hash, "Retype new UNIX password:", _("Retype new UNIX password:"));
		g_hash_table_insert(hash, "Enter new UNIX password:", _("Enter new UNIX password:"));
		g_hash_table_insert(hash, "(current) UNIX password:", _("(current) UNIX password:"));
		g_hash_table_insert(hash, "Error while changing NIS password.", _("Error while changing NIS password."));
		g_hash_table_insert(hash, "You must choose a longer password", _("You must choose a longer password"));
		g_hash_table_insert(hash, "Password has been already used. Choose another.", _("Password has been already used. Choose another."));
		g_hash_table_insert(hash, "You must wait longer to change your password", _("You must wait longer to change your password"));
		g_hash_table_insert(hash, "Sorry, passwords do not match", _("Sorry, passwords do not match"));
		/* FIXME: what about messages which have some variables in them, perhaps try to do those as well */
	}

	s = g_strstrip(g_strdup(msg));
	ret = g_hash_table_lookup(hash, s);
	g_free(s);

	if (ret != NULL)
	{
		return ret;
	}
	else
	{
		return msg;
	}
}

static gboolean auth_message_handler(GSAuthMessageStyle style, const char* msg, char** response, gpointer data)
{
	gboolean ret;
	GSLockPlug* plug;
	const char* message;

	plug = GS_LOCK_PLUG(data);

	gs_debug("Got message style %d: '%s'", style, msg);

	gs_lock_plug_set_ready(plug);

	ret = TRUE;
	*response = NULL;
	message = maybe_translate_message(msg);

	switch (style)
	{
		case GS_AUTH_MESSAGE_PROMPT_ECHO_ON:
			if (msg != NULL)
			{
				char *resp;
				resp = request_response(plug, message, TRUE);
				*response = resp;
			}
			break;
		case GS_AUTH_MESSAGE_PROMPT_ECHO_OFF:
			if (msg != NULL)
			{
				char *resp;
				resp = request_response(plug, message, FALSE);
				*response = resp;
			}
			break;
		case GS_AUTH_MESSAGE_ERROR_MSG:
			/* An error shows only if the dialog is open. */
			if (gs_lock_plug_is_open(plug))
			{
				gs_lock_plug_show_message(plug, message);
			}
			break;
		case GS_AUTH_MESSAGE_TEXT_INFO:
			gs_lock_plug_show_message(plug, message);
			break;
		default:
			g_assert_not_reached();
	}

	if (*response == NULL)
	{
		gs_debug("Got no response");
		ret = FALSE;
	}
	else
	{
		gs_lock_plug_show_message(plug, _("Checking..."));
		gs_lock_plug_set_sensitive(plug, FALSE);
	}

	/* we may have pending events that should be processed before continuing back into PAM */
	while (gtk_events_pending())
	{
		gtk_main_iteration();
	}

	gs_lock_plug_set_busy(plug);

	return ret;
}

static gboolean auth_check_idle(gpointer data);

/* After a failure: the next conversation starts once the failure's message has shown for
 * 3 s, so nothing of the last round's reset lands on the next one's messages. */
static gboolean reset_idle_cb(GSLockPlug* plug)
{
	gs_lock_plug_set_sensitive(plug, TRUE);
	gs_lock_plug_show_message(plug, NULL);
	g_idle_add(auth_check_idle, plug);

	return FALSE;
}

static gboolean do_auth_check(GSLockPlug* plug)
{
	GError* error;
	gboolean res;

	error = NULL;

	gs_lock_plug_disable_prompt(plug);
	gs_lock_plug_set_busy(plug);
	res = gs_auth_verify_user(g_get_user_name(), g_getenv("DISPLAY"), auth_message_handler, plug, &error);

	gs_debug("Verify user returned: %s", res ? "TRUE" : "FALSE");

	if (! res)
	{
		const char *message = _("Authentication failed.");

		if (error != NULL)
		{
			gs_debug("Verify user returned error: %s", error->message);
			message = error->message;
		}

		/* What a module's giving up says isn't for the Idle view. */
		gs_lock_plug_show_message(plug, gs_lock_plug_is_open(plug) ? message : NULL);
	}

	g_clear_error(&error);

	return res;
}

static void unlock(void)
{
	gs_debug("Unlocking");

	gtk_session_lock_lock_unlock_and_destroy(lock);
	g_hash_table_remove_all(lock_windows);
	/* gtk-session-lock's README: the unlock reaches Caminito before the locker exits. */
	gdk_display_sync(gdk_display_get_default());
	gtk_main_quit();
}

static gboolean auth_check_idle(gpointer data)
{
	GSLockPlug* plug = data;
	gboolean res;
	static guint conversations = 0;

	gs_debug("Starting conversation %u", ++conversations);
	res = do_auth_check(plug);

	if (res)
	{
		authenticated = TRUE;
		if (is_locked)
		{
			unlock();
		}
		return G_SOURCE_REMOVE;
	}

	/* mate-screensaver-dialog.c ends its run here after five failures, and the daemon
	 * starts another dialog at the next key: here the conversation starts again however
	 * many failed, once the failure's message has shown for its 3 s. That also keeps a
	 * service that denies everyone from going round at once. */
	gs_debug ("Authentication failed, retrying");
	if (! gs_lock_plug_is_open(plug))
	{
		gs_debug("The conversation ended with the dialog hidden: the next starts quietly");
		g_timeout_add(QUIET_RESTART_MSEC, (GSourceFunc) reset_idle_cb, plug);
		return G_SOURCE_REMOVE;
	}
	g_timeout_add (3000, (GSourceFunc) reset_idle_cb, plug);
	return G_SOURCE_REMOVE;
}

static gboolean logout_delay_passed_cb(gpointer data);

/* gs-window-x11.c's is_logout_enabled, asked again when the delay will have passed: the
 * dialog lasts the whole lock, where the daemon asked once per dialog. */
static void update_logout_enabled(void)
{
	gboolean enabled;
	gboolean shown;
	double elapsed;

	g_clear_handle_id(&logout_timeout_id, g_source_remove);

	enabled = prefs->logout_enabled && prefs->logout_command != NULL;
	elapsed = g_timer_elapsed(timer, NULL);
	shown = enabled && prefs->logout_timeout < (elapsed * 1000);

	g_object_set(plug, "logout-command", prefs->logout_command, "logout-enabled", shown, NULL);
	gs_debug("Log Out: %s", shown ? "shown" : enabled ? "after its delay" : "hidden");

	if (enabled && ! shown)
	{
		logout_timeout_id = g_timeout_add(prefs->logout_timeout - (guint) (elapsed * 1000) + 1, logout_delay_passed_cb, NULL);
	}
}

static gboolean logout_delay_passed_cb(gpointer data G_GNUC_UNUSED)
{
	logout_timeout_id = 0;
	update_logout_enabled();

	return G_SOURCE_REMOVE;
}

static void update_status_message(void)
{
	char* text = NULL;

	if (prefs->status_message_enabled)
	{
		g_object_get(watcher, "status-message", &text, NULL);
	}

	g_object_set(plug, "status-message", text, NULL);
	gs_debug("Status message: %s", text != NULL ? text : "(none)");
	g_free(text);
}

/* gs-monitor.c's _gs_monitor_update_from_prefs, of the keys the dialog shows. */
static void update_from_prefs(void)
{
	gboolean user_switch_enabled;

	user_switch_enabled = (prefs->user_switch_enabled && !prefs->user_switch_disabled);

	g_object_set(plug, "switch-enabled", user_switch_enabled, NULL);
	gs_debug("Switch User: %s", user_switch_enabled ? "enabled" : "disabled");

	update_logout_enabled();
	update_status_message();
}

static void prefs_changed_cb(GSPrefs* changed_prefs G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
	update_from_prefs();
}

static void on_watcher_status_message_changed(GSWatcher* changed_watcher G_GNUC_UNUSED, GParamSpec* pspec G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
	update_status_message();
}

/* A lock window's background, at the size it was made for. */
typedef struct
{
	cairo_surface_t *surface;
	int              width;
	int              height;
} Background;

static void background_free(Background* background)
{
	cairo_surface_destroy(background->surface);
	g_free(background);
}

/* org.mate.background's picture-options, as MateBG's placement. */
static MateBGPlacement placement_of(const char* options)
{
	static const struct
	{
		const char      *options;
		MateBGPlacement  placement;
	} placements[] = {
		{"wallpaper", MATE_BG_PLACEMENT_TILED},
		{"zoom", MATE_BG_PLACEMENT_ZOOMED},
		{"centered", MATE_BG_PLACEMENT_CENTERED},
		{"scaled", MATE_BG_PLACEMENT_SCALED},
		{"stretched", MATE_BG_PLACEMENT_FILL_SCREEN},
		{"spanned", MATE_BG_PLACEMENT_SPANNED},
	};

	for (gsize i = 0; i < G_N_ELEMENTS(placements); i++)
	{
		if (g_strcmp0(options, placements[i].options) == 0)
		{
			return placements[i].placement;
		}
	}

	return MATE_BG_PLACEMENT_ZOOMED;
}

/* Whether the wallpaper file's group `group` is `monitor`'s: GTK 3 has no connector, so
 * a monitor is told by its make, model and place, which come from the same wl_output as
 * the drawer's. The file's format is caminito-wallpaper's src/lock_screen.rs. */
static gboolean is_monitors_group(GKeyFile* file, const char* group, GdkMonitor* monitor)
{
	GdkRectangle  geometry;
	GError       *error = NULL;
	const char   *monitor_make;
	const char   *monitor_model;
	char         *make;
	char         *model;
	int           x;
	int           y;
	gboolean      matches;

	make = g_key_file_get_string(file, group, "make", NULL);
	model = g_key_file_get_string(file, group, "model", NULL);
	x = g_key_file_get_integer(file, group, "x", &error);
	if (error == NULL)
	{
		y = g_key_file_get_integer(file, group, "y", &error);
	}

	gdk_monitor_get_geometry(monitor, &geometry);
	monitor_make = gdk_monitor_get_manufacturer(monitor);
	monitor_model = gdk_monitor_get_model(monitor);
	matches = error == NULL
		&& make != NULL
		&& model != NULL
		&& g_strcmp0(make, monitor_make != NULL ? monitor_make : "") == 0
		&& g_strcmp0(model, monitor_model != NULL ? monitor_model : "") == 0
		&& x == geometry.x
		&& y == geometry.y;

	g_clear_error(&error);
	g_free(make);
	g_free(model);

	return matches;
}

/* Sets `bg` to the wallpaper caminito-wallpaperd says `monitor` shows (218), and returns
 * whether it could. The file is read, never the drawer asked, so the lock comes up when
 * the drawer has crashed or hung. Without the file, without the monitor in it, or with
 * its picture gone, `bg` is left as it was. */
static gboolean load_from_wallpaper_file(MateBG* bg, GdkMonitor* monitor)
{
	GKeyFile      *file;
	GError        *error = NULL;
	char          *path;
	char         **groups;
	char          *group = NULL;
	char          *picture = NULL;
	char          *options = NULL;
	char          *color = NULL;
	GdkRectangle   geometry;
	GdkRGBA        rgba;
	gboolean       loaded = FALSE;

	path = g_build_filename(g_get_user_runtime_dir(), "caminito-wallpaper", "lock-screen", NULL);
	file = g_key_file_new();
	if (!g_key_file_load_from_file(file, path, G_KEY_FILE_NONE, &error))
	{
		gs_debug("No wallpaper file: %s", error->message);
		g_error_free(error);
		goto out;
	}

	groups = g_key_file_get_groups(file, NULL);
	for (char** each = groups; *each != NULL && group == NULL; each++)
	{
		if (is_monitors_group(file, *each, monitor))
		{
			group = g_strdup(*each);
		}
	}
	g_strfreev(groups);
	if (group == NULL)
	{
		gdk_monitor_get_geometry(monitor, &geometry);
		gs_debug("The wallpaper file has no %s %s at %d,%d",
		         gdk_monitor_get_manufacturer(monitor),
		         gdk_monitor_get_model(monitor),
		         geometry.x,
		         geometry.y);
		goto out;
	}

	picture = g_key_file_get_string(file, group, "picture-filename", NULL);
	options = g_key_file_get_string(file, group, "picture-options", NULL);
	color = g_key_file_get_string(file, group, "primary-color", NULL);
	if (picture == NULL || color == NULL || !gdk_rgba_parse(&rgba, color))
	{
		gs_debug("The wallpaper file's %s is incomplete", group);
		goto out;
	}
	if (picture[0] != '\0' && !g_file_test(picture, G_FILE_TEST_EXISTS))
	{
		gs_debug("The wallpaper file's %s names a picture that is gone: %s", group, picture);
		goto out;
	}

	gs_debug("Wallpaper of %s from the wallpaper file: %s", group, picture[0] != '\0' ? picture : color);
	mate_bg_set_color(bg, MATE_BG_COLOR_SOLID, &rgba, NULL);
	mate_bg_set_placement(bg, placement_of(options));
	mate_bg_set_filename(bg, picture[0] != '\0' ? picture : NULL);
	mate_bg_set_draw_background(bg, TRUE);
	loaded = TRUE;

out:
	g_free(color);
	g_free(options);
	g_free(picture);
	g_free(group);
	g_key_file_free(file);
	g_free(path);

	return loaded;
}

/* gs-manager.c's apply_background_to_window, for a lock window at its size: MateBG's
 * background, read again for each, with picture-filename in its place when that file
 * exists. The desktop's is the window's monitor's in the wallpaper file, if it's there. */
static Background* apply_background_to_window(GtkWidget* window, int width, int height)
{
	GSettings       *settings;
	char            *filename;
	Background      *background;
	GdkMonitor      *monitor;

	mate_bg_load_from_preferences(bg);

	monitor = g_object_get_data(G_OBJECT(window), "monitor");
	if (monitor == NULL || !load_from_wallpaper_file(bg, monitor))
	{
		gs_debug("Background from org.mate.background");
	}

	settings = g_settings_new ("org.mate.screensaver");
	filename = g_settings_get_string (settings, "picture-filename");

	if (g_file_test (filename, G_FILE_TEST_EXISTS)) {
		mate_bg_set_filename (bg, filename);
	}
	g_free (filename);
	g_object_unref (settings);

	gs_debug ("Creating background w:%d h:%d", width, height);
	background = g_new(Background, 1);
	background->surface = mate_bg_create_surface (bg,
	                                              gtk_widget_get_window (window),
	                                              width,
	                                              height,
	                                              FALSE);
	background->width = width;
	background->height = height;
	g_object_set_data_full(G_OBJECT(window), "background", background, (GDestroyNotify) background_free);

	return background;
}

/* gs-window-x11.c's gs_window_real_draw, under the window's content. The background is
 * made at the first draw at a size, as the window has its size from Caminito only then. */
static gboolean draw_background(GtkWidget* window, cairo_t* cr, gpointer data G_GNUC_UNUSED)
{
	Background *background;
	int width = gtk_widget_get_allocated_width(window);
	int height = gtk_widget_get_allocated_height(window);

	background = g_object_get_data(G_OBJECT(window), "background");
	if (background == NULL || background->width != width || background->height != height)
	{
		background = apply_background_to_window(window, width, height);
	}

	cairo_set_operator (cr, CAIRO_OPERATOR_OVER);
	if (background->surface != NULL)
	{
		cairo_set_source_surface (cr, background->surface, 0, 0);
		/* MateBG makes a solid colour's surface 1x1, which X tiled as the window's
		 * background pixmap, and which cairo has to be told to. */
		cairo_pattern_set_extend (cairo_get_source (cr), CAIRO_EXTEND_REPEAT);
	}
	else
	{
		cairo_set_source_rgb (cr, 0, 0, 0);
	}
	cairo_paint (cr);

	return FALSE;
}

/* The dialog moves to the lock window the pointer or the keyboard is in, shown or hidden, so
 * it shows on the monitor under the pointer, where Caminito gives the keyboard (12). Motion
 * chooses its output and doesn't show it. */
static gboolean move_dialog(GtkWidget* window, GdkEvent* event G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
	GtkWidget* parent = gtk_widget_get_parent(plug);
	GtkWidget* overlay = g_object_get_data(G_OBJECT(window), "overlay");

	if (parent != overlay)
	{
		gs_debug("Moving the dialog to %s", gdk_monitor_get_model(g_object_get_data(G_OBJECT(window), "monitor")));

		/* With its monitor's window gone and none left, the dialog has no window. */
		if (parent != NULL)
		{
			gtk_container_remove(GTK_CONTAINER(parent), plug);
		}
		gtk_overlay_add_overlay(GTK_OVERLAY(overlay), plug);
	}

	return FALSE;
}

/* A click on any output shows the dialog there. */
static gboolean show_dialog_at_click(GtkWidget* window, GdkEvent* event G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
	move_dialog(window, NULL, NULL);
	gs_lock_plug_open(GS_LOCK_PLUG(plug));

	return FALSE;
}

/* A key in the Idle view shows the dialog, before the dialog's own handler of the window's
 * keys. Once it shows, keys go to the dialog. */
static gboolean show_dialog_at_key(GtkWidget* window, GdkEventKey* event, gpointer data G_GNUC_UNUSED)
{
	if (gs_lock_plug_is_open(GS_LOCK_PLUG(plug)))
	{
		return FALSE;
	}

	move_dialog(window, NULL, NULL);
	gs_lock_plug_open_at_key(GS_LOCK_PLUG(plug), event);

	return TRUE;
}

/* The clock's markup, at its size. */
static void set_clock_markup(GtkWidget* window)
{
	gboolean small = GPOINTER_TO_INT(g_object_get_data(G_OBJECT(window), "clock-small"));
	char* markup;

	markup = g_markup_printf_escaped("<span size=\"%s\" weight=\"light\">%s</span>", small ? "xx-large" : "60000", clock_time);
	gtk_label_set_markup(GTK_LABEL(g_object_get_data(G_OBJECT(window), "clock-time")), markup);
	g_free(markup);

	markup = g_markup_printf_escaped("<span size=\"%s\">%s</span>", small ? "large" : "x-large", clock_date);
	gtk_label_set_markup(GTK_LABEL(g_object_get_data(G_OBJECT(window), "clock-date")), markup);
	g_free(markup);
}

/* Whether the locale writes the time on a 12-hour clock: glibc gives AM and PM to 24-hour
 * locales too, so only its time format can say. */
static gboolean locale_has_12_hour_time(void)
{
	const char* format = nl_langinfo(T_FMT);

	return strstr(format, "%r") != NULL || strstr(format, "%p") != NULL
		|| strstr(format, "%P") != NULL || strstr(format, "%I") != NULL
		|| strstr(format, "%l") != NULL;
}

/* lock-dialog-time-format's text for now. "locale" is the locale's time without its seconds,
 * which strftime has no conversion for. */
static char* format_time(GDateTime* now, GSettings* settings)
{
	char* format = g_settings_get_string(settings, "lock-dialog-time-format");
	char* text;

	if (g_strcmp0(format, "locale") == 0)
	{
		text = g_date_time_format(now, locale_has_12_hour_time() ? "%l:%M %p" : "%H:%M");
	}
	else
	{
		text = g_date_time_format(now, format);
	}

	g_free(format);

	return text != NULL ? g_strstrip(text) : g_strdup("");
}

/* lock-dialog-date-format's text for now. */
static char* format_date(GDateTime* now, GSettings* settings)
{
	char* format = g_settings_get_string(settings, "lock-dialog-date-format");
	char* text = g_date_time_format(now, g_strcmp0(format, "locale") == 0 ? _("%A, %B %e") : format);

	g_free(format);

	return text != NULL ? text : g_strdup("");
}

/* Each second, for a format with seconds. The labels change only when the text does. */
static gboolean update_clocks(gpointer data G_GNUC_UNUSED)
{
	static GSettings* settings = NULL;
	GDateTime* now = g_date_time_new_now_local();
	GHashTableIter iter;
	gpointer window;
	char* time;
	char* date;

	if (settings == NULL)
	{
		settings = g_settings_new("org.mate.screensaver");
	}

	time = format_time(now, settings);
	date = format_date(now, settings);
	g_date_time_unref(now);

	if (g_strcmp0(time, clock_time) == 0 && g_strcmp0(date, clock_date) == 0)
	{
		g_free(time);
		g_free(date);
		return G_SOURCE_CONTINUE;
	}

	g_free(clock_time);
	g_free(clock_date);
	clock_time = time;
	clock_date = date;

	g_hash_table_iter_init(&iter, lock_windows);
	while (g_hash_table_iter_next(&iter, NULL, &window))
	{
		set_clock_markup(window);
	}

	return G_SOURCE_CONTINUE;
}

/* The clock is made smaller only on the output where the open dialog would reach it. */
static gboolean fit_clocks(gpointer data G_GNUC_UNUSED)
{
	GHashTableIter iter;
	gpointer each;

	fit_clocks_id = 0;

	g_hash_table_iter_init(&iter, lock_windows);
	while (g_hash_table_iter_next(&iter, NULL, &each))
	{
		GtkWidget* window = each;
		gboolean small = GPOINTER_TO_INT(g_object_get_data(G_OBJECT(window), "clock-small"));
		gboolean should_be_small = FALSE;

		if (gs_lock_plug_is_open(GS_LOCK_PLUG(plug))
		    && gtk_widget_get_parent(plug) == g_object_get_data(G_OBJECT(window), "overlay"))
		{
			int clock_height;
			int dialog_height;

			/* Its height at the large size, kept from before it was made small. */
			if (!small)
			{
				gtk_widget_get_preferred_height(g_object_get_data(G_OBJECT(window), "clock"), NULL, &clock_height);
				g_object_set_data(G_OBJECT(window), "clock-height", GINT_TO_POINTER(clock_height));
			}
			clock_height = GPOINTER_TO_INT(g_object_get_data(G_OBJECT(window), "clock-height"));
			gtk_widget_get_preferred_height(plug, NULL, &dialog_height);
			should_be_small = clock_height + CLOCK_GAP > (gtk_widget_get_allocated_height(window) - dialog_height) / 2;
		}

		if (small != should_be_small)
		{
			gs_debug("The clock is %s", should_be_small ? "smaller, to clear the dialog" : "back at its size");
			g_object_set_data(G_OBJECT(window), "clock-small", GINT_TO_POINTER(should_be_small));
			set_clock_markup(window);
		}
	}

	return G_SOURCE_REMOVE;
}

/* The dialog was shown, hidden, moved or resized, or its window was. */
static void queue_fit_clocks(void)
{
	if (fit_clocks_id == 0)
	{
		fit_clocks_id = g_idle_add(fit_clocks, NULL);
	}
}

/* The Idle view's clock and date, at the top in the middle of a lock window. */
static GtkWidget* add_clock(GtkWidget* window)
{
	GtkWidget* clock = gtk_box_new(GTK_ORIENTATION_VERTICAL, 2);
	GtkWidget* time = gtk_label_new(NULL);
	GtkWidget* date = gtk_label_new(NULL);

	gtk_style_context_add_class(gtk_widget_get_style_context(clock), "lock-clock");
	gtk_widget_set_halign(clock, GTK_ALIGN_CENTER);
	gtk_widget_set_valign(clock, GTK_ALIGN_START);
	gtk_widget_set_margin_top(clock, 24);
	gtk_container_add(GTK_CONTAINER(clock), time);
	gtk_container_add(GTK_CONTAINER(clock), date);

	g_object_set_data(G_OBJECT(window), "clock", clock);
	g_object_set_data(G_OBJECT(window), "clock-time", time);
	g_object_set_data(G_OBJECT(window), "clock-date", date);
	set_clock_markup(window);

	return clock;
}

static void add_lock_window(GdkMonitor* monitor)
{
	GtkWidget* window;
	GtkWidget* overlay;

	window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
	/* The background is drawn instead of the theme's. */
	gtk_widget_set_app_paintable(window, TRUE);
	g_signal_connect(window, "draw", G_CALLBACK(draw_background), NULL);
	gtk_widget_add_events(window, GDK_ENTER_NOTIFY_MASK | GDK_POINTER_MOTION_MASK | GDK_BUTTON_PRESS_MASK);
	g_signal_connect(window, "button-press-event", G_CALLBACK(show_dialog_at_click), NULL);
	g_signal_connect(window, "key-press-event", G_CALLBACK(show_dialog_at_key), NULL);
	g_signal_connect(window, "enter-notify-event", G_CALLBACK(move_dialog), NULL);
	g_signal_connect(window, "motion-notify-event", G_CALLBACK(move_dialog), NULL);
	g_signal_connect(window, "focus-in-event", G_CALLBACK(move_dialog), NULL);

	/* For its wallpaper. The window goes before its monitor does. */
	g_object_set_data(G_OBJECT(window), "monitor", monitor);
	gtk_session_lock_lock_new_surface(lock, GTK_WINDOW(window), monitor);
	g_hash_table_insert(lock_windows, monitor, window);

	overlay = gtk_overlay_new();
	g_object_set_data(G_OBJECT(window), "overlay", overlay);
	gtk_container_add(GTK_CONTAINER(overlay), add_clock(window));
	gtk_container_add(GTK_CONTAINER(window), overlay);

	if (gtk_widget_get_parent(plug) == NULL)
	{
		gtk_overlay_add_overlay(GTK_OVERLAY(overlay), plug);
	}

	gtk_widget_show_all(window);
}

static void monitor_added_cb(GdkDisplay* display G_GNUC_UNUSED, GdkMonitor* monitor, gpointer data G_GNUC_UNUSED)
{
	gs_debug("Monitor added: %s", gdk_monitor_get_model(monitor));

	add_lock_window(monitor);
}

/* The dialog goes to another monitor's window before its own goes, if there's another. */
static void monitor_removed_cb(GdkDisplay* display G_GNUC_UNUSED, GdkMonitor* monitor, gpointer data G_GNUC_UNUSED)
{
	GtkWidget* window;
	GtkWidget* overlay;
	GHashTableIter iter;
	gpointer other;

	gs_debug("Monitor removed: %s", gdk_monitor_get_model(monitor));

	window = g_hash_table_lookup(lock_windows, monitor);
	if (window == NULL)
	{
		return;
	}

	overlay = g_object_get_data(G_OBJECT(window), "overlay");
	if (gtk_widget_get_parent(plug) == overlay)
	{
		g_hash_table_iter_init(&iter, lock_windows);
		while (g_hash_table_iter_next(&iter, NULL, &other))
		{
			if (other != window)
			{
				move_dialog(other, NULL, NULL);
				break;
			}
		}
	}

	if (gtk_widget_get_parent(plug) == overlay)
	{
		gtk_container_remove(GTK_CONTAINER(overlay), plug);
	}

	g_hash_table_remove(lock_windows, monitor);
}

static void locked_cb(GtkSessionLockLock* session_lock G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
	gs_debug("Locked");

	is_locked = TRUE;
	if (authenticated)
	{
		unlock();
	}
}

/* Caminito refused the lock: another locker holds it, or only one Caminito spawned may
 * take it (12). */
static void finished_cb(GtkSessionLockLock* session_lock, gpointer data G_GNUC_UNUSED)
{
	g_printerr("caminito-locker: the compositor refused the lock\n");

	gtk_session_lock_lock_destroy(session_lock);
	gdk_display_sync(gdk_display_get_default());
	exit(1);
}

/* The clock has to be read over any wallpaper, a light one too. The Idle view has no blur. */
static void load_css(void)
{
	GtkCssProvider* provider = gtk_css_provider_new();

	gtk_css_provider_load_from_data(provider,
	                                ".lock-clock {"
	                                " color: white;"
	                                " background-color: rgba(0, 0, 0, 0.3);"
	                                " border-radius: 12px;"
	                                " padding: 6px 24px;"
	                                " text-shadow: 0 1px 4px rgba(0, 0, 0, 0.9), 0 0 12px rgba(0, 0, 0, 0.6);"
	                                " }",
	                                -1,
	                                NULL);
	gtk_style_context_add_provider_for_screen(gdk_screen_get_default(),
	                                          GTK_STYLE_PROVIDER(provider),
	                                          GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
	g_object_unref(provider);
}

int main(int argc, char** argv)
{
	GError* error = NULL;
	GdkDisplay* display;

	setlocale(LC_ALL, "");
	bindtextdomain(GETTEXT_PACKAGE, MATELOCALEDIR);
	bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8");
	textdomain(GETTEXT_PACKAGE);

	if (!gtk_init_with_args(&argc, &argv, NULL, entries, NULL, &error))
	{
		if (error != NULL)
		{
			fprintf(stderr, "%s\n", error->message);
			g_error_free(error);
		}

		exit(1);
	}

	if (show_version)
	{
		g_print("%s %s\n", argv[0], VERSION);
		exit(1);
	}

	if (verbose)
	{
		g_log_set_debug_enabled(TRUE);
		gs_auth_set_verbose(TRUE);
	}

	if (!gtk_session_lock_is_supported())
	{
		g_printerr("caminito-locker: the compositor has no ext-session-lock\n");
		exit(1);
	}

	gs_auth_priv_init();

	lock = gtk_session_lock_prepare_lock();
	g_signal_connect(lock, "locked", G_CALLBACK(locked_cb), NULL);
	g_signal_connect(lock, "finished", G_CALLBACK(finished_cb), NULL);
	gtk_session_lock_lock_lock(lock);

	plug = gs_lock_plug_new();
	g_object_ref_sink(plug);

	bg = mate_bg_new();
	prefs = gs_prefs_new();
	g_signal_connect(prefs, "changed", G_CALLBACK(prefs_changed_cb), NULL);
	watcher = gs_watcher_new();
	g_signal_connect(watcher, "notify::status-message", G_CALLBACK(on_watcher_status_message_changed), NULL);
	timer = g_timer_new();
	update_from_prefs();

	load_css();
	g_signal_connect_swapped(plug, "size-allocate", G_CALLBACK(queue_fit_clocks), NULL);
	g_signal_connect_swapped(plug, "hide", G_CALLBACK(queue_fit_clocks), NULL);

	lock_windows = g_hash_table_new_full(NULL, NULL, NULL, (GDestroyNotify) gtk_widget_destroy);
	update_clocks(NULL);
	g_timeout_add_seconds(1, update_clocks, NULL);
	display = gdk_display_get_default();
	for (int i = 0; i < gdk_display_get_n_monitors(display); i++)
	{
		add_lock_window(gdk_display_get_monitor(display, i));
	}
	g_signal_connect(display, "monitor-added", G_CALLBACK(monitor_added_cb), NULL);
	g_signal_connect(display, "monitor-removed", G_CALLBACK(monitor_removed_cb), NULL);

	g_idle_add(auth_check_idle, plug);

	gtk_main();

	return 0;
}
