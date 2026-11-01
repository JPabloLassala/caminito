/* -*- Mode: C; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 8 -*-
 *
 * Copyright (C) 2004-2006 William Jon McCann <mccann@jhu.edu>
 * Copyright (C) 2012-2021 MATE Developers
 * SPDX-FileCopyrightText: 2026 Juan Pablo Lassala
 * SPDX-License-Identifier: GPL-3.0-or-later AND GPL-2.0-or-later
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License as
 * published by the Free Software Foundation; either version 2 of the
 * License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
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
 * Ported from mate-screensaver 60b2db3's src/mate-screensaver-command.c, from dbus-glib to
 * GDBus (CMN-297). mate-panel, mate-session and mate-settings-daemon's media keys run this
 * command by name, and without the mate-screensaver package nothing else provides it, so it's
 * in a directory of Caminito's own at the front of the session command's PATH. It only
 * talks to org.mate.ScreenSaver, which Caminito owns, and never starts it. It fails when
 * the name has no owner or refuses a call, as Caminito does with `disable-lock-screen`.
 */

#include "config.h"

#include <locale.h>
#include <stdlib.h>

#include <glib/gi18n.h>
#include <gio/gio.h>

#define NAME      "org.mate.ScreenSaver"
#define PATH      "/org/mate/ScreenSaver"
#define INTERFACE "org.mate.ScreenSaver"

static gboolean do_quit       = FALSE;
static gboolean do_lock       = FALSE;
static gboolean do_unlock     = FALSE;
static gboolean do_cycle      = FALSE;
static gboolean do_activate   = FALSE;
static gboolean do_deactivate = FALSE;
static gboolean do_version    = FALSE;
static gboolean do_poke       = FALSE;
static gboolean do_inhibit    = FALSE;
static gboolean do_query      = FALSE;
static gboolean do_time       = FALSE;

static char *inhibit_reason      = NULL;
static char *inhibit_application = NULL;

static GOptionEntry entries [] =
{
	{
		"exit", 0, 0, G_OPTION_ARG_NONE, &do_quit,
		N_("Causes the screensaver to exit gracefully"), NULL
	},
	{
		"query", 'q', 0, G_OPTION_ARG_NONE, &do_query,
		N_("Query the state of the screensaver"), NULL
	},
	{
		"time", 't', 0, G_OPTION_ARG_NONE, &do_time,
		N_("Query the length of time the screensaver has been active"), NULL
	},
	{
		"lock", 'l', 0, G_OPTION_ARG_NONE, &do_lock,
		N_("Tells the running screensaver process to lock the screen immediately"), NULL
	},
	{
		"unlock", 'u', 0, G_OPTION_ARG_NONE, &do_unlock,
		N_("Tells the running screensaver process to unlock the screen immediately"), NULL
	},
	{
		"cycle", 'c', 0, G_OPTION_ARG_NONE, &do_cycle,
		N_("If the screensaver is active then switch to another graphics demo"), NULL
	},
	{
		"activate", 'a', 0, G_OPTION_ARG_NONE, &do_activate,
		N_("Turn the screensaver on (blank the screen)"), NULL
	},
	{
		"deactivate", 'd', 0, G_OPTION_ARG_NONE, &do_deactivate,
		N_("If the screensaver is active then deactivate it (un-blank the screen)"), NULL
	},
	{
		"poke", 'p', 0, G_OPTION_ARG_NONE, &do_poke,
		N_("Poke the running screensaver to simulate user activity"), NULL
	},
	{
		"inhibit", 'i', 0, G_OPTION_ARG_NONE, &do_inhibit,
		N_("Inhibit the screensaver from activating.  Command blocks while inhibit is active."), NULL
	},
	{
		"application-name", 'n', 0, G_OPTION_ARG_STRING, &inhibit_application,
		N_("The calling application that is inhibiting the screensaver"), NULL
	},
	{
		"reason", 'r', 0, G_OPTION_ARG_STRING, &inhibit_reason,
		N_("The reason for inhibiting the screensaver"), NULL
	},
	{
		"version", 'V', 0, G_OPTION_ARG_NONE, &do_version,
		N_("Version of this application"), NULL
	},
	{ NULL, 0, 0, G_OPTION_ARG_NONE, NULL, NULL, NULL }
};

/* The call's reply, or the end of the program with what went wrong. */
static GVariant *
call (GDBusConnection *connection,
      const char      *method,
      GVariant        *parameters)
{
	GError   *error = NULL;
	GVariant *reply;

	reply = g_dbus_connection_call_sync (connection, NAME, PATH, INTERFACE, method, parameters,
	                                     NULL, G_DBUS_CALL_FLAGS_NO_AUTO_START, -1, NULL, &error);
	if (reply == NULL)
	{
		g_printerr ("mate-screensaver-command: %s: %s\n", method, error->message);
		exit (1);
	}

	return reply;
}

static void
call_and_forget (GDBusConnection *connection,
                 const char      *method,
                 GVariant        *parameters)
{
	g_variant_unref (call (connection, method, parameters));
}

static gboolean
get_active (GDBusConnection *connection)
{
	GVariant *reply = call (connection, "GetActive", NULL);
	gboolean  active;

	g_variant_get (reply, "(b)", &active);
	g_variant_unref (reply);

	return active;
}

static void
query (GDBusConnection *connection)
{
	GVariant *reply;
	char    **inhibitors;

	g_print (_("The screensaver is %s\n"), get_active (connection) ? _("active") : _("inactive"));

	reply = call (connection, "GetInhibitors", NULL);
	g_variant_get (reply, "(^as)", &inhibitors);
	g_variant_unref (reply);

	if (inhibitors[0] == NULL)
	{
		g_print (_("The screensaver is not inhibited\n"));
	}
	else
	{
		g_print (_("The screensaver is being inhibited by:\n"));
		for (int i = 0; inhibitors[i] != NULL; i++)
		{
			g_print ("\t%s\n", inhibitors[i]);
		}
	}
	g_strfreev (inhibitors);
}

static void
active_time (GDBusConnection *connection)
{
	if (get_active (connection))
	{
		GVariant *reply = call (connection, "GetActiveTime", NULL);
		guint32   seconds;

		g_variant_get (reply, "(u)", &seconds);
		g_variant_unref (reply);
		g_print (_("The screensaver has been active for %d seconds.\n"), (int) seconds);
	}
	else
	{
		g_print (_("The screensaver is not currently active.\n"));
	}
}

int
main (int argc, char **argv)
{
	GOptionContext  *context;
	GDBusConnection *connection;
	GError          *error = NULL;

	setlocale (LC_ALL, "");
	bindtextdomain (GETTEXT_PACKAGE, MATELOCALEDIR);
	bind_textdomain_codeset (GETTEXT_PACKAGE, "UTF-8");
	textdomain (GETTEXT_PACKAGE);

	context = g_option_context_new (NULL);
	g_option_context_add_main_entries (context, entries, GETTEXT_PACKAGE);
	if (! g_option_context_parse (context, &argc, &argv, &error))
	{
		g_printerr ("%s\n", error->message);
		return 1;
	}
	g_option_context_free (context);

	if (do_version)
	{
		g_print ("%s %s\n", argv[0], VERSION);
		return 0;
	}

	connection = g_bus_get_sync (G_BUS_TYPE_SESSION, NULL, &error);
	if (connection == NULL)
	{
		g_printerr ("mate-screensaver-command: %s\n", error->message);
		return 1;
	}

	if (do_quit)
	{
		call_and_forget (connection, "Quit", NULL);
	}
	if (do_query)
	{
		query (connection);
	}
	if (do_time)
	{
		active_time (connection);
	}
	if (do_lock)
	{
		call_and_forget (connection, "Lock", NULL);
	}
	if (do_unlock)
	{
		call_and_forget (connection, "Unlock", NULL);
	}
	if (do_cycle)
	{
		call_and_forget (connection, "Cycle", NULL);
	}
	if (do_poke)
	{
		call_and_forget (connection, "SimulateUserActivity", NULL);
	}
	if (do_activate)
	{
		call_and_forget (connection, "SetActive", g_variant_new ("(b)", TRUE));
	}
	if (do_deactivate)
	{
		call_and_forget (connection, "SetActive", g_variant_new ("(b)", FALSE));
	}
	if (do_inhibit)
	{
		call_and_forget (connection, "Inhibit",
		                 g_variant_new ("(ss)",
		                                inhibit_application ? inhibit_application : "Unknown",
		                                inhibit_reason ? inhibit_reason : "Unknown"));
		/* The hold is the connection's: it ends as this process does. */
		g_main_loop_run (g_main_loop_new (NULL, FALSE));
	}

	g_object_unref (connection);

	return 0;
}
