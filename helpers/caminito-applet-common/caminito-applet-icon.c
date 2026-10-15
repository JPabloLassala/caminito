/*
 * The applets' panel icon (ADR 0035). See caminito-applet-icon.h.
 */

#include "caminito-applet-icon.h"

int
caminito_applet_icon_pixel_size (guint panel_size)
{
  /* The sizes icon themes draw, the largest that leaves the panel 2 px each side. */
  static const int sizes[] = { 48, 32, 24, 22, 16 };

  for (guint i = 0; i < G_N_ELEMENTS (sizes); i++)
    if ((int) panel_size - 4 >= sizes[i])
      return sizes[i];
  return 16;
}

static void
on_change_size (MatePanelApplet *applet G_GNUC_UNUSED, guint size, gpointer data)
{
  gtk_image_set_pixel_size (GTK_IMAGE (data), caminito_applet_icon_pixel_size (size));
}

GtkWidget *
caminito_applet_icon_new (MatePanelApplet *applet)
{
  GtkWidget *icon = gtk_image_new ();

  gtk_image_set_pixel_size (GTK_IMAGE (icon),
                            caminito_applet_icon_pixel_size (mate_panel_applet_get_size (applet)));
  g_signal_connect_object (applet, "change-size", G_CALLBACK (on_change_size), icon, 0);
  return icon;
}

void
caminito_applet_icon_set (GtkWidget *icon, const char *const *names)
{
  g_autoptr (GIcon) gicon = NULL;

  /* No default fallbacks: they would shorten "bluetooth-active-symbolic" to
     "bluetooth-active" and so on, before the next of names had its turn. */
  gicon = g_themed_icon_new_from_names ((char **) names, -1);
  gtk_image_set_from_gicon (GTK_IMAGE (icon), gicon, GTK_ICON_SIZE_BUTTON);
}
