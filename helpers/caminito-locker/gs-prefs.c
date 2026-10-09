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
 * Ported to caminito-locker (132) from mate-screensaver 60b2db3's src/gs-prefs.c, cut to
 * the keys of the dialog's buttons and status message: user-switch-enabled and
 * org.mate.lockdown's disable-user-switching, status-message-enabled, and logout-enabled,
 * logout-delay and logout-command. Caminito reads the others it honours (09, 12), and
 * embedded-keyboard-enabled and -command, power-management-delay and screensaver-arguments
 * are never honoured (12). As in mate-screensaver, each is read at the start and again when
 * it changes, and "changed" says so.
 *
 * An empty logout-command is no command, so the Log Out button isn't shown for one: in
 * mate-screensaver it is, and does nothing but warn (12: "only if the command is set").
 */

#include <string.h>

#include <glib.h>
#include <glib-object.h>
#include <gio/gio.h>

#include "gs-prefs.h"

static void gs_prefs_finalize   (GObject      *object);

#define LOCKDOWN_SETTINGS_SCHEMA "org.mate.lockdown"
#define KEY_USER_SWITCH_DISABLE "disable-user-switching"

#define GSETTINGS_SCHEMA "org.mate.screensaver"
#define KEY_USER_SWITCH_ENABLED "user-switch-enabled"
#define KEY_LOGOUT_ENABLED "logout-enabled"
#define KEY_LOGOUT_DELAY "logout-delay"
#define KEY_LOGOUT_COMMAND "logout-command"
#define KEY_STATUS_MESSAGE_ENABLED "status-message-enabled"

#define _gs_prefs_set_user_switch_disabled(x,y) ((x)->user_switch_disabled = ((y) != FALSE))
#define _gs_prefs_set_status_message_enabled(x,y) ((x)->status_message_enabled = ((y) != FALSE))
#define _gs_prefs_set_logout_enabled(x,y) ((x)->logout_enabled = ((y) != FALSE))
#define _gs_prefs_set_user_switch_enabled(x,y) ((x)->user_switch_enabled = ((y) != FALSE))

struct GSPrefsPrivate
{
	GSettings *settings;
	GSettings *lockdown_settings;
};

enum
{
    CHANGED,
    LAST_SIGNAL
};

static guint         signals [LAST_SIGNAL] = { 0, };

G_DEFINE_TYPE_WITH_PRIVATE (GSPrefs, gs_prefs, G_TYPE_OBJECT)

static void
gs_prefs_class_init (GSPrefsClass *klass)
{
	GObjectClass   *object_class = G_OBJECT_CLASS (klass);

	object_class->finalize     = gs_prefs_finalize;

	signals [CHANGED] =
	    g_signal_new ("changed",
	                  G_TYPE_FROM_CLASS (object_class),
	                  G_SIGNAL_RUN_LAST,
	                  G_STRUCT_OFFSET (GSPrefsClass, changed),
	                  NULL,
	                  NULL,
	                  g_cclosure_marshal_VOID__VOID,
	                  G_TYPE_NONE,
	                  0);
}

static void
_gs_prefs_set_logout_command (GSPrefs    *prefs,
                              const char *value)
{
	g_free (prefs->logout_command);
	prefs->logout_command = NULL;

	if (value && value[0] != '\0')
	{
		/* FIXME: check command */

		prefs->logout_command = g_strdup (value);
	}
}

static void
_gs_prefs_set_logout_timeout (GSPrefs *prefs,
                              int      value)
{
	if (value < 0)
		value = 0;

	/* pick a reasonable large number for the
	   upper bound */
	if (value > 480)
		value = 480;

	prefs->logout_timeout = value * 60000;
}

static void
gs_prefs_load_from_settings (GSPrefs *prefs)
{
	glong      value;
	gboolean   bvalue;
	char      *string;

	bvalue = g_settings_get_boolean (prefs->priv->lockdown_settings, KEY_USER_SWITCH_DISABLE);
	_gs_prefs_set_user_switch_disabled (prefs, bvalue);

	bvalue = g_settings_get_boolean (prefs->priv->settings, KEY_STATUS_MESSAGE_ENABLED);
	_gs_prefs_set_status_message_enabled (prefs, bvalue);

	/* Logout options */

	bvalue = g_settings_get_boolean (prefs->priv->settings, KEY_LOGOUT_ENABLED);
	_gs_prefs_set_logout_enabled (prefs, bvalue);

	string = g_settings_get_string (prefs->priv->settings, KEY_LOGOUT_COMMAND);
	_gs_prefs_set_logout_command (prefs, string);
	g_free (string);

	value = g_settings_get_int (prefs->priv->settings, KEY_LOGOUT_DELAY);
	_gs_prefs_set_logout_timeout (prefs, value);

	/* User switching options */

	bvalue = g_settings_get_boolean (prefs->priv->settings, KEY_USER_SWITCH_ENABLED);
	_gs_prefs_set_user_switch_enabled (prefs, bvalue);
}

/* The keys mate-screensaver's key_changed_cb handles that the locker doesn't read are
 * left alone here, rather than warned about. */
static void
key_changed_cb (GSettings *settings,
		gchar *key,
		GSPrefs *prefs)
{
	if (strcmp (key, KEY_USER_SWITCH_DISABLE) == 0)
	{
		gboolean disabled;

		disabled = g_settings_get_boolean (settings, key);
		_gs_prefs_set_user_switch_disabled (prefs, disabled);

	}
	else if (strcmp (key, KEY_STATUS_MESSAGE_ENABLED) == 0)
	{
		gboolean enabled;

		enabled = g_settings_get_boolean (settings, key);
		_gs_prefs_set_status_message_enabled (prefs, enabled);

	}
	else if (strcmp (key, KEY_LOGOUT_ENABLED) == 0)
	{
		gboolean enabled;

		enabled = g_settings_get_boolean (settings, key);
		_gs_prefs_set_logout_enabled (prefs, enabled);

	}
	else if (strcmp (key, KEY_LOGOUT_DELAY) == 0)
	{
		int delay;

		delay = g_settings_get_int (settings, key);
		_gs_prefs_set_logout_timeout (prefs, delay);

	}
	else if (strcmp (key, KEY_LOGOUT_COMMAND) == 0)
	{
		char *command;
		command = g_settings_get_string (settings, key);
		_gs_prefs_set_logout_command (prefs, command);
		g_free (command);
	}
	else if (strcmp (key, KEY_USER_SWITCH_ENABLED) == 0)
	{
		gboolean enabled;

		enabled = g_settings_get_boolean (settings, key);
		_gs_prefs_set_user_switch_enabled (prefs, enabled);

	}
	else
	{
		return;
	}

	g_signal_emit (prefs, signals [CHANGED], 0);
}

static void
gs_prefs_init (GSPrefs *prefs)
{
	prefs->priv = gs_prefs_get_instance_private (prefs);

	prefs->priv->settings = g_settings_new (GSETTINGS_SCHEMA);
	g_signal_connect (prefs->priv->settings,
			  "changed",
			  G_CALLBACK (key_changed_cb),
			  prefs);
	prefs->priv->lockdown_settings = g_settings_new (LOCKDOWN_SETTINGS_SCHEMA);
	g_signal_connect (prefs->priv->lockdown_settings,
			  "changed",
			  G_CALLBACK (key_changed_cb),
			  prefs);

	prefs->logout_enabled          = FALSE;
	prefs->user_switch_enabled     = FALSE;

	prefs->logout_timeout          = 14400000;

	gs_prefs_load_from_settings (prefs);
}

static void
gs_prefs_finalize (GObject *object)
{
	GSPrefs *prefs;

	g_return_if_fail (object != NULL);
	g_return_if_fail (GS_IS_PREFS (object));

	prefs = GS_PREFS (object);

	g_return_if_fail (prefs->priv != NULL);

	if (prefs->priv->settings)
	{
		g_object_unref (prefs->priv->settings);
		prefs->priv->settings = NULL;
	}

	if (prefs->priv->lockdown_settings) {
		g_object_unref (prefs->priv->lockdown_settings);
		prefs->priv->lockdown_settings = NULL;
	}

	g_free (prefs->logout_command);

	G_OBJECT_CLASS (gs_prefs_parent_class)->finalize (object);
}

GSPrefs *
gs_prefs_new (void)
{
	GObject *prefs;

	prefs = g_object_new (GS_TYPE_PREFS, NULL);

	return GS_PREFS (prefs);
}
