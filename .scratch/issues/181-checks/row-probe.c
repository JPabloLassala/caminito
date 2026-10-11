/* A GTK module that prints the label after "Windowing System" in any
 * toplevel, then quits (181). */
#include <gtk/gtk.h>
#include <stdio.h>
#include <stdlib.h>

static GList *labels;

static void collect (GtkWidget *w, gpointer data)
{
  if (GTK_IS_LABEL (w))
    labels = g_list_append (labels, w);
  if (GTK_IS_CONTAINER (w))
    gtk_container_forall (GTK_CONTAINER (w), collect, NULL);
}

static gboolean probe (gpointer data)
{
  GList *tops = gtk_window_list_toplevels (), *l;
  for (l = tops; l; l = l->next)
    collect (l->data, NULL);
  for (l = labels; l && l->next; l = l->next)
    if (g_strcmp0 (gtk_label_get_text (l->data), "Windowing System") == 0)
      printf ("row=%s\n", gtk_label_get_text (l->next->data));
  fflush (stdout);
  exit (0);
  return G_SOURCE_REMOVE;
}

G_MODULE_EXPORT void gtk_module_init (gint *argc, gchar ***argv)
{
  g_timeout_add (1500, probe, NULL);
}
