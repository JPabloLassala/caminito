/* -*- Mode: C; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 8 -*-
 *
 * Copyright (C) 2004-2006 William Jon McCann <mccann@jhu.edu>
 * Copyright (C) 2008      Red Hat, Inc.
 * Copyright (C) 2012-2021 MATE Developers
 * SPDX-FileCopyrightText: 2025-2026 Juan Pablo Lassala
 * SPDX-License-Identifier: GPL-3.0-or-later AND GPL-2.0-or-later
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
 * Ported to caminito-locker (132) from mate-screensaver 60b2db3's src/gs-watcher-x11.c.
 * Caminito watches idleness, so only the session's status message is left: mate-session's
 * presence status-text, which mate-screensaver's monitor puts in the dialog when
 * status-message-enabled is on (gs-monitor.c). dbus-glib is gone, so the presence is a
 * GDBusProxy on the well-known name, which also follows mate-session to a new owner and
 * reads its status text then, where dbus_g_proxy_new_for_name_owner stayed with the first
 * and warned without one. The status text is read asynchronously, as the locker makes its
 * lock windows meanwhile, where mate-screensaver's daemon read it at its own start. The
 * status, idle and away, isn't read, as the dialog shows none of it.
 */

#include <gio/gio.h>

#include "gs-watcher.h"

static void     gs_watcher_finalize     (GObject        *object);

struct GSWatcherPrivate
{
	char           *status_message;

	GDBusProxy     *presence_proxy;
	/* The read of the status text in flight, cancelled with the watcher. */
	GCancellable   *cancellable;
};

enum
{
    PROP_0,
    PROP_STATUS_MESSAGE
};

G_DEFINE_TYPE_WITH_PRIVATE (GSWatcher, gs_watcher, G_TYPE_OBJECT)

static void
gs_watcher_get_property (GObject    *object,
                         guint       prop_id,
                         GValue     *value,
                         GParamSpec *pspec)
{
	GSWatcher *self;

	self = GS_WATCHER (object);

	switch (prop_id)
	{
	case PROP_STATUS_MESSAGE:
		g_value_set_string (value, self->priv->status_message);
		break;
	default:
		G_OBJECT_WARN_INVALID_PROPERTY_ID (object, prop_id, pspec);
		break;
	}
}

static void
set_status_text (GSWatcher  *watcher,
                 const char *text)
{
	g_free (watcher->priv->status_message);

	watcher->priv->status_message = g_strdup (text);
	g_object_notify (G_OBJECT (watcher), "status-message");
}

static void
gs_watcher_set_property (GObject          *object,
                         guint             prop_id,
                         const GValue     *value,
                         GParamSpec       *pspec)
{
	GSWatcher *self;

	self = GS_WATCHER (object);

	switch (prop_id)
	{
	case PROP_STATUS_MESSAGE:
		set_status_text (self, g_value_get_string (value));
		break;
	default:
		G_OBJECT_WARN_INVALID_PROPERTY_ID (object, prop_id, pspec);
		break;
	}
}

static void
gs_watcher_class_init (GSWatcherClass *klass)
{
	GObjectClass   *object_class = G_OBJECT_CLASS (klass);

	object_class->finalize = gs_watcher_finalize;
	object_class->get_property = gs_watcher_get_property;
	object_class->set_property = gs_watcher_set_property;

	g_object_class_install_property (object_class,
	                                 PROP_STATUS_MESSAGE,
	                                 g_param_spec_string ("status-message",
	                                         NULL,
	                                         NULL,
	                                         NULL,
	                                         G_PARAM_READWRITE));
}

static void
on_presence_signal (GDBusProxy *presence_proxy G_GNUC_UNUSED,
                    const char *sender_name G_GNUC_UNUSED,
                    const char *signal_name,
                    GVariant   *parameters,
                    GSWatcher  *watcher)
{
	const char *status_text;

	if (g_strcmp0 (signal_name, "StatusTextChanged") != 0
	    || ! g_variant_is_of_type (parameters, G_VARIANT_TYPE ("(s)")))
	{
		return;
	}

	g_variant_get (parameters, "(&s)", &status_text);
	set_status_text (watcher, status_text);
}

static void
got_status_text (GObject      *source,
                 GAsyncResult *result,
                 gpointer      data)
{
	GSWatcher *watcher;
	GError    *error;
	GVariant  *reply;
	GVariant  *value;

	error = NULL;
	reply = g_dbus_proxy_call_finish (G_DBUS_PROXY (source), result, &error);

	if (reply == NULL)
	{
		if (! g_error_matches (error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
		{
			g_warning ("Couldn't get presence status text: %s", error->message);
		}
		g_error_free (error);
		return;
	}

	watcher = GS_WATCHER (data);

	g_variant_get (reply, "(v)", &value);
	if (g_variant_is_of_type (value, G_VARIANT_TYPE_STRING))
	{
		set_status_text (watcher, g_variant_get_string (value, NULL));
	}
	g_variant_unref (value);
	g_variant_unref (reply);
}

/* The presence's status text, once mate-session owns its name. */
static void
get_status_text (GSWatcher *watcher)
{
	char *owner;

	owner = g_dbus_proxy_get_name_owner (watcher->priv->presence_proxy);
	if (owner == NULL)
	{
		return;
	}
	g_free (owner);

	g_dbus_proxy_call (watcher->priv->presence_proxy,
	                   "org.freedesktop.DBus.Properties.Get",
	                   g_variant_new ("(ss)",
	                                  "org.gnome.SessionManager.Presence",
	                                  "status-text"),
	                   G_DBUS_CALL_FLAGS_NONE,
	                   -1,
	                   watcher->priv->cancellable,
	                   got_status_text,
	                   watcher);
}

static void
on_presence_name_owner_changed (GDBusProxy *presence_proxy G_GNUC_UNUSED,
                                GParamSpec *pspec G_GNUC_UNUSED,
                                GSWatcher  *watcher)
{
	get_status_text (watcher);
}

static gboolean
connect_presence_watcher (GSWatcher *watcher)
{
	GError            *error;
	gboolean           ret;

	ret = FALSE;

	error = NULL;
	watcher->priv->presence_proxy = g_dbus_proxy_new_for_bus_sync (G_BUS_TYPE_SESSION,
	                                G_DBUS_PROXY_FLAGS_DO_NOT_LOAD_PROPERTIES
	                                | G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START,
	                                NULL,
	                                "org.gnome.SessionManager",
	                                "/org/gnome/SessionManager/Presence",
	                                "org.gnome.SessionManager.Presence",
	                                NULL,
	                                &error);
	if (watcher->priv->presence_proxy == NULL)
	{
		g_warning ("Failed to get session presence proxy: %s", error->message);
		g_error_free (error);
		goto done;
	}

	g_signal_connect (watcher->priv->presence_proxy,
	                  "g-signal",
	                  G_CALLBACK (on_presence_signal),
	                  watcher);
	g_signal_connect (watcher->priv->presence_proxy,
	                  "notify::g-name-owner",
	                  G_CALLBACK (on_presence_name_owner_changed),
	                  watcher);

	get_status_text (watcher);

	ret = TRUE;

done:
	return ret;
}

static void
gs_watcher_init (GSWatcher *watcher)
{
	watcher->priv = gs_watcher_get_instance_private (watcher);
	watcher->priv->cancellable = g_cancellable_new ();

	connect_presence_watcher (watcher);
}

static void
gs_watcher_finalize (GObject *object)
{
	GSWatcher *watcher;

	g_return_if_fail (object != NULL);
	g_return_if_fail (GS_IS_WATCHER (object));

	watcher = GS_WATCHER (object);

	g_return_if_fail (watcher->priv != NULL);

	g_cancellable_cancel (watcher->priv->cancellable);
	g_object_unref (watcher->priv->cancellable);

	if (watcher->priv->presence_proxy != NULL)
	{
		g_signal_handlers_disconnect_by_data (watcher->priv->presence_proxy, watcher);
		g_object_unref (watcher->priv->presence_proxy);
	}

	g_free (watcher->priv->status_message);

	G_OBJECT_CLASS (gs_watcher_parent_class)->finalize (object);
}

GSWatcher *
gs_watcher_new (void)
{
	GSWatcher *watcher;

	watcher = g_object_new (GS_TYPE_WATCHER, NULL);

	return GS_WATCHER (watcher);
}
