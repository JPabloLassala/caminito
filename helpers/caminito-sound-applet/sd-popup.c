/*
 * The Sound applet's popup (CMN-213). See sd-popup.h.
 */

#include "sd-popup.h"

/* The prototype's popup's width. */
#define WIDTH 372

struct _SdPopup
{
  GtkWidget *no_server;
};

SdPopup *
sd_popup_new (CaminitoAppletPopup *applet_popup)
{
  SdPopup *popup = g_new0 (SdPopup, 1);
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);

  gtk_widget_set_size_request (box, WIDTH, -1);
  g_object_set (box, "margin", 12, NULL);
  popup->no_server = gtk_label_new ("No sound server");
  gtk_widget_set_no_show_all (popup->no_server, TRUE);
  gtk_box_pack_start (GTK_BOX (box), popup->no_server, FALSE, FALSE, 0);
  gtk_container_add (GTK_CONTAINER (caminito_applet_popup_get_window (applet_popup)), box);
  gtk_widget_show (box);
  return popup;
}

void
sd_popup_update (SdPopup *popup, const SdStatus *status)
{
  gtk_widget_set_visible (popup->no_server, !status->running);
}

void
sd_popup_free (SdPopup *popup)
{
  g_free (popup);
}
