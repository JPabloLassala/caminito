/*
 * CMN-224's probe: a layer surface along the bottom of the output, 28 high and blue, that asks
 * Caminito for intellihide with the mate-panel patch's own wayland-intellihide.c, which this
 * includes, so what is run is what the panel runs. Caminito's headless tests start it
 * (caminito/src/tests/mate_panel_intellihide.rs) and read where Caminito draws it.
 *
 * It reads one command a line from stdin, and writes what it did to stdout:
 *   ask    asks for intellihide on its layer surface, as the panel does when its Intellihide
 *          is turned on
 *   stop   stops asking
 *   quit   leaves
 * and writes "ready" once its window is mapped, with the global found or not:
 * "ready intellihide=yes".
 */
#include <config.h>

#include <stdio.h>
#include <string.h>

#include <gdk/gdkwayland.h>
#include <gtk/gtk.h>
#include <gtk-layer-shell.h>

#include "wayland-intellihide.h"

static GtkWidget *window;
static GMainLoop *loop;

static gboolean
draw (GtkWidget *widget,
      cairo_t   *cr)
{
	cairo_set_source_rgb (cr, 0, 0, 1);
	cairo_paint (cr);
	return FALSE;
}

static void
mapped (GtkWidget *widget,
	gpointer   data)
{
	printf ("ready intellihide=%s\n", wayland_intellihide_available () ? "yes" : "no");
	fflush (stdout);
}

static gboolean
command (GIOChannel   *channel,
	 GIOCondition  condition,
	 gpointer      data)
{
	gchar *line = NULL;

	if (g_io_channel_read_line (channel, &line, NULL, NULL, NULL) != G_IO_STATUS_NORMAL) {
		g_main_loop_quit (loop);
		return FALSE;
	}
	g_strstrip (line);
	if (strcmp (line, "ask") == 0) {
		wayland_intellihide_update (GTK_WINDOW (window), TRUE);
		printf ("asked\n");
	} else if (strcmp (line, "stop") == 0) {
		wayland_intellihide_update (GTK_WINDOW (window), FALSE);
		printf ("stopped\n");
	} else if (strcmp (line, "quit") == 0) {
		g_main_loop_quit (loop);
	}
	fflush (stdout);
	g_free (line);
	return TRUE;
}

int
main (int    argc,
      char **argv)
{
	GIOChannel *input;

	gtk_init (&argc, &argv);
	if (!GDK_IS_WAYLAND_DISPLAY (gdk_display_get_default ())) {
		fprintf (stderr, "not on Wayland\n");
		return 1;
	}

	window = gtk_window_new (GTK_WINDOW_TOPLEVEL);
	gtk_widget_set_app_paintable (window, TRUE);
	gtk_widget_set_size_request (window, -1, 28);
	gtk_layer_init_for_window (GTK_WINDOW (window));
	gtk_layer_set_layer (GTK_WINDOW (window), GTK_LAYER_SHELL_LAYER_TOP);
	gtk_layer_set_namespace (GTK_WINDOW (window), "panel");
	gtk_layer_set_anchor (GTK_WINDOW (window), GTK_LAYER_SHELL_EDGE_BOTTOM, TRUE);
	gtk_layer_set_anchor (GTK_WINDOW (window), GTK_LAYER_SHELL_EDGE_LEFT, TRUE);
	gtk_layer_set_anchor (GTK_WINDOW (window), GTK_LAYER_SHELL_EDGE_RIGHT, TRUE);
	gtk_layer_set_exclusive_zone (GTK_WINDOW (window), 0);
	g_signal_connect (window, "draw", G_CALLBACK (draw), NULL);
	g_signal_connect_after (window, "map", G_CALLBACK (mapped), NULL);
	gtk_widget_show (window);

	input = g_io_channel_unix_new (0);
	g_io_add_watch (input, G_IO_IN | G_IO_HUP, command, NULL);

	loop = g_main_loop_new (NULL, FALSE);
	g_main_loop_run (loop);
	return 0;
}
