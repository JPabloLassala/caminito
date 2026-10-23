/*
 * The popup's content (CMN-193): with NetworkManager not running it says so, and once
 * NetworkManager runs it doesn't. Without a display, GTK can't start, and it's skipped.
 */

#include "nw-popup.h"

#define NOT_RUNNING "NetworkManager isn't running"

typedef struct
{
  GtkWidget           *anchor;
  CaminitoAppletPopup *applet_popup;
  NwPopup             *popup;
} Fixture;

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  fixture->anchor = g_object_ref_sink (gtk_button_new ());
  fixture->applet_popup = caminito_applet_popup_new (fixture->anchor);
  fixture->popup = nw_popup_new (fixture->applet_popup);
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  nw_popup_free (fixture->popup);
  /* The applet's popup lives as long as the applet: its button here. */
  gtk_widget_destroy (fixture->anchor);
  g_object_unref (fixture->anchor);
}

typedef struct
{
  const char *text;
  GtkWidget  *found;
} Search;

static void
visit (GtkWidget *widget, gpointer data)
{
  Search *search = data;

  if (search->found == NULL && GTK_IS_LABEL (widget)
      && g_strcmp0 (gtk_label_get_text (GTK_LABEL (widget)), search->text) == 0)
    search->found = widget;
  if (GTK_IS_CONTAINER (widget))
    gtk_container_forall (GTK_CONTAINER (widget), visit, search);
}

/* Whether a label with text would show: it's there, and every widget above it in the
   window is visible and, in a stack, the stack's page. */
static gboolean
shows (Fixture *fixture, const char *text)
{
  GtkWidget *window = caminito_applet_popup_get_window (fixture->applet_popup);
  Search search = { text, NULL };

  visit (window, &search);
  if (search.found == NULL)
    return FALSE;
  for (GtkWidget *widget = search.found; widget != window; widget = gtk_widget_get_parent (widget))
    {
      GtkWidget *parent = gtk_widget_get_parent (widget);

      if (!gtk_widget_get_visible (widget))
        return FALSE;
      if (GTK_IS_STACK (parent) && gtk_stack_get_visible_child (GTK_STACK (parent)) != widget)
        return FALSE;
    }
  return TRUE;
}

static void
test_not_running_says_so (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  /* Until NetworkManager's reader answers, it isn't running. */
  g_assert_true (shows (fixture, NOT_RUNNING));
  nw_popup_set_running (fixture->popup, TRUE);
  g_assert_false (shows (fixture, NOT_RUNNING));
  nw_popup_set_running (fixture->popup, FALSE);
  g_assert_true (shows (fixture, NOT_RUNNING));
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  if (!gtk_init_check (&argc, &argv))
    {
      g_test_skip ("no display");
      return g_test_run ();
    }
  g_test_add ("/popup/without-networkmanager-it-says-so", Fixture, NULL, setup, test_not_running_says_so, teardown);
  return g_test_run ();
}
