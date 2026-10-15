/*
 * The Power applet's popup (262). See pw-popup.h.
 */

#include "pw-popup.h"

#define POPUP_WIDTH 320

struct _PwPopup
{
  CaminitoAppletPopup *popup;
  PwPopupActions       actions;
  gpointer             data;
  GtkWidget           *content;
};

static void
on_configure (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  PwPopup *popup = data;

  caminito_applet_popup_hide (popup->popup);
  popup->actions.configure (popup->data);
}

PwPopup *
pw_popup_new (CaminitoAppletPopup *applet_popup, const PwPopupActions *actions, gpointer data)
{
  PwPopup *popup = g_new0 (PwPopup, 1);
  GtkWidget *window = caminito_applet_popup_get_window (applet_popup);
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 4);
  GtkWidget *configure = gtk_button_new_with_label ("Configure Power Management…");

  popup->popup = applet_popup;
  popup->actions = *actions;
  popup->data = data;

  gtk_container_set_border_width (GTK_CONTAINER (box), 8);
  gtk_widget_set_size_request (box, POPUP_WIDTH, -1);
  gtk_button_set_relief (GTK_BUTTON (configure), GTK_RELIEF_NONE);
  gtk_widget_set_halign (configure, GTK_ALIGN_START);
  g_signal_connect (configure, "clicked", G_CALLBACK (on_configure), popup);
  /* The batteries (CMN-104), the power profile (CMN-98) and Stay awake (CMN-101) go above
     this. */
  gtk_box_pack_end (GTK_BOX (box), configure, FALSE, FALSE, 0);
  gtk_widget_show_all (box);
  gtk_container_add (GTK_CONTAINER (window), box);
  popup->content = box;
  g_object_add_weak_pointer (G_OBJECT (box), (gpointer *) &popup->content);
  return popup;
}

void
pw_popup_free (PwPopup *popup)
{
  if (popup->content != NULL)
    {
      g_object_remove_weak_pointer (G_OBJECT (popup->content), (gpointer *) &popup->content);
      gtk_widget_destroy (popup->content);
    }
  g_free (popup);
}
