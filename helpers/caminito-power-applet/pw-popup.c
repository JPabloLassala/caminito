/*
 * The Power applet's popup (262). See pw-popup.h.
 */

#include "pw-popup.h"

#include "pw-awake.h"

#define POPUP_WIDTH 320

struct _PwPopup
{
  CaminitoAppletPopup *popup;
  PwPopupActions       actions;
  gpointer             data;
  GtkWidget           *content;
  PwAwake             *awake;
};

static void
on_set_stay_awake (gboolean on, gpointer data)
{
  PwPopup *popup = data;

  popup->actions.set_stay_awake (on, popup->data);
}

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
  static const PwAwakeActions awake_actions = { on_set_stay_awake };
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
  popup->awake = pw_awake_new (&awake_actions, popup);
  /* The power profile (CMN-98) goes above Stay awake and the batteries (CMN-104) below it,
     above what keeps the computer awake. */
  gtk_box_pack_start (GTK_BOX (box), pw_awake_row (popup->awake), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), pw_awake_inhibitors (popup->awake), FALSE, FALSE, 0);
  gtk_box_pack_end (GTK_BOX (box), configure, FALSE, FALSE, 0);
  gtk_widget_show_all (box);
  gtk_container_add (GTK_CONTAINER (window), box);
  popup->content = box;
  g_object_add_weak_pointer (G_OBJECT (box), (gpointer *) &popup->content);
  return popup;
}

void
pw_popup_set_stay_awake (PwPopup *popup, gboolean on)
{
  pw_awake_set_stay_awake (popup->awake, on);
}

void
pw_popup_set_inhibitors (PwPopup *popup, const GPtrArray *inhibitors)
{
  pw_awake_set_inhibitors (popup->awake, inhibitors);
  /* A window doesn't shrink when its content does: asking for the least brings it back to
     what the list needs, at the width it has. */
  gtk_window_resize (GTK_WINDOW (caminito_applet_popup_get_window (popup->popup)), POPUP_WIDTH,
                     1);
}

void
pw_popup_free (PwPopup *popup)
{
  if (popup->content != NULL)
    {
      g_object_remove_weak_pointer (G_OBJECT (popup->content), (gpointer *) &popup->content);
      gtk_widget_destroy (popup->content);
    }
  pw_awake_free (popup->awake);
  g_free (popup);
}
