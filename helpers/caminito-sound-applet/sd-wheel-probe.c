/*
 * The Sound popup's wheel end to end (CMN-215, as CMN-210's spike did): the popup's content,
 * with twelve outputs, their list open and a short room, so that it scrolls, in an
 * undecorated toplevel of its own. caminito's sound_popup_wheel test runs it under a headless
 * Caminito, on GDK's Wayland backend, and sends it the wheel over the output's slider. It's
 * built for that test, not installed.
 *
 * stdout, a line each:
 *   ready slider=X,Y         the window is mapped, and the output's slider's middle in it
 *   volume=P                 the popup asked for the output's volume P percent
 *   state TAG scale=V vadj=V the answer to "state TAG" on stdin
 *   reset                    the answer to "reset": the slider at 50, scrolled to the top
 *
 * Animations are off, so that a plain wheel's scroll lands at once. It quits on "quit", at
 * the end of stdin, or after 60 s.
 */

#include <stdio.h>
#include <string.h>

#include "sd-popup.h"

#define PERCENT(p) ((pa_volume_t) (((guint64) (p) * PA_VOLUME_NORM + 50) / 100))
#define N_OUTPUTS  12

static GtkWidget *scale;
static GtkAdjustment *vadj;

static void
on_set_volume (SdSide side, pa_volume_t volume, gpointer data G_GNUC_UNUSED)
{
  if (side == SD_OUTPUT)
    printf ("volume=%u\n", sd_state_percent (volume));
}

static void
ignore (void)
{
}

static const SdPopupActions actions = {
  .set_volume = on_set_volume,
  .set_muted = (void *) ignore,
  .set_default = (void *) ignore,
  .set_port = (void *) ignore,
  .open_settings = (void *) ignore,
};

static GtkWidget *
find (GtkWidget *widget, const char *name)
{
  GtkWidget *found = NULL;

  if (g_strcmp0 (gtk_widget_get_name (widget), name) == 0)
    return widget;
  if (GTK_IS_CONTAINER (widget))
    {
      g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (widget));

      for (GList *l = children; l != NULL && found == NULL; l = l->next)
        found = find (l->data, name);
    }
  return found;
}

static gboolean
answer_state (gpointer data)
{
  char *tag = data;

  printf ("state %s scale=%g vadj=%g\n", tag, gtk_range_get_value (GTK_RANGE (scale)),
          gtk_adjustment_get_value (vadj));
  g_free (tag);
  return G_SOURCE_REMOVE;
}

static gboolean
on_stdin (GIOChannel *channel, GIOCondition condition G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
  g_autofree char *line = NULL;
  gsize length;
  GIOStatus status = g_io_channel_read_line (channel, &line, &length, NULL, NULL);

  if (status == G_IO_STATUS_EOF || status == G_IO_STATUS_ERROR)
    {
      gtk_main_quit ();
      return G_SOURCE_REMOVE;
    }
  if (line == NULL)
    return G_SOURCE_CONTINUE;
  g_strchomp (line);
  if (strcmp (line, "quit") == 0)
    gtk_main_quit ();
  else if (strcmp (line, "reset") == 0)
    {
      gtk_range_set_value (GTK_RANGE (scale), 50);
      gtk_adjustment_set_value (vadj, 0);
      printf ("reset\n");
    }
  else if (g_str_has_prefix (line, "state "))
    {
      /* What the compositor sent before the test asked is read by the roundtrip, and
         handled by GDK's source, of a higher priority, before the low idle answers. */
      gdk_display_sync (gdk_display_get_default ());
      g_idle_add_full (G_PRIORITY_LOW, answer_state, g_strdup (line + 6), NULL);
    }
  return G_SOURCE_CONTINUE;
}

static gboolean
on_mapped (GtkWidget *window, GdkEvent *event G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
  int x, y;

  gtk_widget_translate_coordinates (scale, window, gtk_widget_get_allocated_width (scale) / 2,
                                    gtk_widget_get_allocated_height (scale) / 2, &x, &y);
  printf ("ready slider=%d,%d\n", x, y);
  return FALSE;
}

static gboolean
on_timeout (gpointer data G_GNUC_UNUSED)
{
  printf ("timeout\n");
  gtk_main_quit ();
  return G_SOURCE_REMOVE;
}

int
main (int argc, char **argv)
{
  SdDevice outputs[N_OUTPUTS];
  SdStatus status = { .running = TRUE, .outputs = outputs, .n_outputs = N_OUTPUTS,
                      .default_output = "sink0" };
  GtkWidget *anchor, *window, *content;
  CaminitoAppletPopup *applet_popup;
  SdPopup *popup;
  GIOChannel *in;

  setvbuf (stdout, NULL, _IOLBF, 0);
  gtk_init (&argc, &argv);
  g_object_set (gtk_settings_get_default (), "gtk-enable-animations", FALSE, NULL);
  for (guint i = 0; i < N_OUTPUTS; i++)
    outputs[i] = (SdDevice) { .name = g_strdup_printf ("sink%u", i),
                              .description = g_strdup_printf ("Output %u", i),
                              .volume = PERCENT (50) };

  anchor = gtk_button_new ();
  applet_popup = caminito_applet_popup_new (anchor);
  popup = sd_popup_new (applet_popup, &actions, NULL);
  sd_popup_set_volume_settings (popup, 5, FALSE);
  sd_popup_update (popup, &status);
  sd_popup_set_room (popup, 300);
  /* The content in a toplevel of its own: a popup needs a panel to be placed against. */
  content = gtk_bin_get_child (GTK_BIN (caminito_applet_popup_get_window (applet_popup)));
  g_object_ref (content);
  gtk_container_remove (GTK_CONTAINER (caminito_applet_popup_get_window (applet_popup)),
                        content);
  window = gtk_window_new (GTK_WINDOW_TOPLEVEL);
  gtk_window_set_title (GTK_WINDOW (window), "sd-wheel-probe");
  /* No client-side frame or shadow: the window's geometry is its allocation. */
  gtk_window_set_decorated (GTK_WINDOW (window), FALSE);
  gtk_container_add (GTK_CONTAINER (window), content);
  g_object_unref (content);
  scale = find (content, "output-scale");
  vadj = gtk_scrolled_window_get_vadjustment (GTK_SCROLLED_WINDOW (find (content, "scrolled")));
  gtk_button_clicked (GTK_BUTTON (find (content, "output-pill")));

  g_signal_connect (window, "map-event", G_CALLBACK (on_mapped), NULL);
  g_signal_connect (window, "destroy", G_CALLBACK (gtk_main_quit), NULL);
  gtk_widget_show (window);

  in = g_io_channel_unix_new (0);
  g_io_add_watch (in, G_IO_IN | G_IO_HUP | G_IO_ERR, on_stdin, NULL);
  g_timeout_add_seconds (60, on_timeout, NULL);
  gtk_main ();
  return 0;
}
