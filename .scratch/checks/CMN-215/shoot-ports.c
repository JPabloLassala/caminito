/*
 * CMN-215's shot of the port dropdown, which null devices can't have: the Sound popup drawn
 * from a status built by hand, as test-popup's are, with Built-in Audio the Default output on
 * its Speakers port and Headphones plugged in, Line Out unplugged and left out, the list and
 * the dropdown open.
 *
 *   shoot-ports <png>
 *
 * The popup's window is shown off every output and drawn with gtk_widget_draw(): an
 * offscreen window would take its content's minimum height (CMN-210).
 */

#include "sd-popup.h"

#define PERCENT(p) ((pa_volume_t) (((guint64) (p) * PA_VOLUME_NORM + 50) / 100))

static void
nothing (void)
{
}

static const SdPopupActions actions = {
  .set_volume = (void *) nothing,
  .set_muted = (void *) nothing,
  .set_default = (void *) nothing,
  .set_port = (void *) nothing,
  .open_settings = (void *) nothing,
};

static void
pump_for (int ms)
{
  gint64 end = g_get_monotonic_time () + ms * 1000;

  while (g_get_monotonic_time () < end)
    {
      while (gtk_events_pending ())
        gtk_main_iteration ();
      g_usleep (2000);
    }
}

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

int
main (int argc, char **argv)
{
  SdPort ports[] = { { "analog-output-speaker", "Speakers" },
                     { "analog-output-headphones", "Headphones" } };
  SdDevice outputs[] = {
    { .name = "alsa", .description = "Built-in Audio", .form = SD_FORM_SPEAKER,
      .volume = PERCENT (60), .ports = ports, .n_ports = 2,
      .active_port = "analog-output-speaker", .active_port_description = "Speakers" },
    { .name = "hdmi", .description = "DELL U2720Q", .form = SD_FORM_DISPLAY,
      .volume = PERCENT (70), .active_port = "hdmi-output-0",
      .active_port_description = "DisplayPort 1" },
    { .name = "easyeffects", .description = "Easy Effects Sink", .form = SD_FORM_SPEAKER,
      .virtual_device = TRUE, .volume = PERCENT (100) },
  };
  SdDevice inputs[] = {
    { .name = "mic", .description = "Internal Microphone", .form = SD_FORM_MICROPHONE,
      .volume = PERCENT (40) },
  };
  SdStatus status = {
    .running = TRUE, .has_output = TRUE, .output_name = "Built-in Audio",
    .volume = PERCENT (60), .has_input = TRUE,
    .outputs = outputs, .n_outputs = G_N_ELEMENTS (outputs),
    .inputs = inputs, .n_inputs = G_N_ELEMENTS (inputs),
    .default_output = "alsa", .default_input = "mic",
  };
  GtkWidget *panel, *anchor, *window;
  CaminitoAppletPopup *applet_popup;
  SdPopup *popup;
  cairo_surface_t *surface;
  cairo_t *cr;
  g_autoptr (GdkPixbuf) pixbuf = NULL;
  g_autoptr (GError) error = NULL;

  gtk_init (&argc, &argv);
  if (argc != 2)
    {
      g_printerr ("usage: shoot-ports PNG\n");
      return 2;
    }
  g_object_set (gtk_settings_get_default (), "gtk-enable-animations", FALSE, NULL);
  panel = gtk_window_new (GTK_WINDOW_POPUP);
  gtk_window_move (GTK_WINDOW (panel), -10000, -10000);
  anchor = gtk_button_new ();
  gtk_container_add (GTK_CONTAINER (panel), anchor);
  gtk_widget_show_all (panel);
  applet_popup = caminito_applet_popup_new (anchor);
  popup = sd_popup_new (applet_popup, &actions, NULL);
  sd_popup_update (popup, &status);
  window = caminito_applet_popup_get_window (applet_popup);
  gtk_window_set_transient_for (GTK_WINDOW (window), GTK_WINDOW (panel));
  gtk_window_move (GTK_WINDOW (window), -10000, -10000);
  gtk_widget_show (window);
  gtk_button_clicked (GTK_BUTTON (find (window, "output-pill")));
  gtk_button_clicked (GTK_BUTTON (find (window, "output-ports-button")));
  pump_for (600);

  surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, gtk_widget_get_allocated_width (window),
                                        gtk_widget_get_allocated_height (window));
  cr = cairo_create (surface);
  gtk_widget_draw (window, cr);
  cairo_destroy (cr);
  pixbuf = gdk_pixbuf_get_from_surface (surface, 0, 0, cairo_image_surface_get_width (surface),
                                        cairo_image_surface_get_height (surface));
  cairo_surface_destroy (surface);
  if (!gdk_pixbuf_save (pixbuf, argv[1], "png", &error, NULL))
    {
      g_printerr ("can't save %s: %s\n", argv[1], error->message);
      return 1;
    }
  sd_popup_free (popup);
  gtk_widget_destroy (panel);
  return 0;
}
