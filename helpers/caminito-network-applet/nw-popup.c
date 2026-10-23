/*
 * The Network applet's popup (CMN-193). See nw-popup.h.
 */

#include "nw-popup.h"

struct _NwPopup
{
  GtkWidget *stack;
};

/* What the popup is with NetworkManager not running: its icon over the words. */
static GtkWidget *
not_running_page (void)
{
  static const char *const names[] = { "network-offline", "network-offline-symbolic", NULL };
  g_autoptr (GIcon) gicon = g_themed_icon_new_from_names ((char **) names, -1);
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 12);
  GtkWidget *icon = gtk_image_new_from_gicon (gicon, GTK_ICON_SIZE_DIALOG);
  GtkWidget *label = gtk_label_new ("NetworkManager isn't running");

  gtk_image_set_pixel_size (GTK_IMAGE (icon), 64);
  gtk_container_set_border_width (GTK_CONTAINER (box), 24);
  gtk_box_pack_start (GTK_BOX (box), icon, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), label, FALSE, FALSE, 0);
  return box;
}

NwPopup *
nw_popup_new (CaminitoAppletPopup *applet_popup)
{
  NwPopup *popup = g_new0 (NwPopup, 1);
  /* Empty until the tiles and the card (CMN-196) fill it, at the size they'll have. */
  GtkWidget *content = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);

  gtk_widget_set_size_request (content, 360, 120);
  popup->stack = gtk_stack_new ();
  gtk_stack_add_named (GTK_STACK (popup->stack), not_running_page (), "not-running");
  gtk_stack_add_named (GTK_STACK (popup->stack), content, "content");
  gtk_widget_show_all (popup->stack);
  gtk_stack_set_visible_child_name (GTK_STACK (popup->stack), "not-running");
  gtk_container_add (GTK_CONTAINER (caminito_applet_popup_get_window (applet_popup)),
                     popup->stack);
  return popup;
}

void
nw_popup_free (NwPopup *popup)
{
  g_free (popup);
}

void
nw_popup_set_running (NwPopup *popup, gboolean running)
{
  gtk_stack_set_visible_child_name (GTK_STACK (popup->stack),
                                    running ? "content" : "not-running");
}
