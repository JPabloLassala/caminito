/*
 * The Notification center (CMN-260): the history drawn from entries built by hand, newest
 * first, and what a click on an entry, its close button, its actions and "Clear all" call
 * back. Without a display, GTK can't start, and it's skipped.
 */

#include "nt-popup.h"

#define NOW         1700000000
#define NO_ENTRIES  "No notifications"
#define CLEAR_ALL   "Clear all"

typedef struct
{
  GtkWidget           *anchor;
  CaminitoAppletPopup *applet_popup;
  NtPopup             *popup;
  /* What the popup called back, as "invoke 7 default;", "dismiss 3;", "clear all;". */
  GString             *calls;
  GPtrArray           *entries;
} Fixture;

static void
on_invoke (guint32 id, const char *key, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "invoke %u %s;", id, key);
}

static void
on_dismiss (guint32 id, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "dismiss %u;", id);
}

static void
on_clear_all (gpointer data)
{
  Fixture *fixture = data;

  g_string_append (fixture->calls, "clear all;");
}

static const NtPopupActions actions = {
  .invoke = on_invoke,
  .dismiss = on_dismiss,
  .clear_all = on_clear_all,
};

/* An entry from Mail, which arrived ago seconds before NOW, with the actions given as
   "key", "label" pairs, NULL-terminated. */
static NtEntry *
entry (guint32 id, const char *summary, const char *body, gint64 ago, gboolean has_default, ...)
{
  NtEntry *entry = g_new0 (NtEntry, 1);
  const char *key;
  va_list args;

  entry->id = id;
  entry->app = g_strdup ("org.gnome.Evolution");
  entry->app_name = g_strdup ("Mail");
  entry->app_icon = g_strdup ("mail-unread");
  entry->summary = g_strdup (summary);
  entry->body = g_strdup (body);
  entry->has_default = has_default;
  entry->time = NOW - ago;
  entry->actions = g_ptr_array_new ();
  va_start (args, has_default);
  while ((key = va_arg (args, const char *)) != NULL)
    {
      NtAction *action = g_new0 (NtAction, 1);

      action->key = g_strdup (key);
      action->label = g_strdup (va_arg (args, const char *));
      g_ptr_array_add (entry->actions, action);
    }
  va_end (args);
  /* nt_entry_free frees the actions as the array's own. */
  g_ptr_array_set_free_func (entry->actions, NULL);
  return entry;
}

static void
entry_free (gpointer data)
{
  NtEntry *entry = data;

  for (guint i = 0; i < entry->actions->len; i++)
    {
      NtAction *action = g_ptr_array_index (entry->actions, i);

      g_free (action->key);
      g_free (action->label);
      g_free (action);
    }
  g_ptr_array_set_size (entry->actions, 0);
  nt_entry_free (entry);
}

/* Lunch?, newest, with a default action and Reply and Archive; Invoice, without either. */
static void
two_entries (Fixture *fixture)
{
  g_ptr_array_add (fixture->entries,
                   entry (7, "Lunch?", "From <b>Ana</b>", 5 * 60, TRUE, "reply", "Reply",
                          "archive", "Archive", NULL));
  g_ptr_array_add (fixture->entries,
                   entry (3, "Invoice", "Due on Friday", 3 * 60 * 60, FALSE, NULL));
}

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  fixture->anchor = g_object_ref_sink (gtk_button_new ());
  fixture->applet_popup = caminito_applet_popup_new (fixture->anchor);
  fixture->popup = nt_popup_new (fixture->applet_popup, &actions, fixture);
  fixture->calls = g_string_new (NULL);
  fixture->entries = g_ptr_array_new_with_free_func (entry_free);
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  nt_popup_free (fixture->popup);
  /* The applet's popup lives as long as the applet: its button here. */
  gtk_widget_destroy (fixture->anchor);
  g_object_unref (fixture->anchor);
  g_string_free (fixture->calls, TRUE);
  g_ptr_array_unref (fixture->entries);
}

typedef struct
{
  const char *text;
  const char *name;
  GtkWidget  *found;
} Search;

static void
visit (GtkWidget *widget, gpointer data)
{
  Search *search = data;

  if (search->found != NULL)
    return;
  if (search->text != NULL && GTK_IS_LABEL (widget)
      && g_strcmp0 (gtk_label_get_text (GTK_LABEL (widget)), search->text) == 0)
    search->found = widget;
  else if (search->name != NULL && g_strcmp0 (gtk_widget_get_name (widget), search->name) == 0)
    search->found = widget;
  if (GTK_IS_CONTAINER (widget))
    gtk_container_forall (GTK_CONTAINER (widget), visit, search);
}

static GtkWidget *
window_of (Fixture *fixture)
{
  return caminito_applet_popup_get_window (fixture->applet_popup);
}

/* The label showing text under widget, NULL with none. */
static GtkWidget *
label_under (GtkWidget *widget, const char *text)
{
  Search search = { text, NULL, NULL };

  visit (widget, &search);
  return search.found;
}

/* The widget named name under widget, NULL with none. */
static GtkWidget *
named_under (GtkWidget *widget, const char *name)
{
  Search search = { NULL, name, NULL };

  visit (widget, &search);
  return search.found;
}

/* Whether widget would show: every widget above it in the window is visible and, in a
   stack, the stack's page. */
static gboolean
widget_shows (Fixture *fixture, GtkWidget *found)
{
  GtkWidget *window = window_of (fixture);

  if (found == NULL)
    return FALSE;
  for (GtkWidget *widget = found; widget != window; widget = gtk_widget_get_parent (widget))
    {
      GtkWidget *parent = gtk_widget_get_parent (widget);

      if (!gtk_widget_get_visible (widget) || !gtk_widget_get_child_visible (widget))
        return FALSE;
      if (GTK_IS_STACK (parent) && gtk_stack_get_visible_child (GTK_STACK (parent)) != widget)
        return FALSE;
    }
  return TRUE;
}

static gboolean
shows (Fixture *fixture, const char *text)
{
  return widget_shows (fixture, label_under (window_of (fixture), text));
}

static GtkWidget *
ancestor (GtkWidget *widget, GType type)
{
  while (widget != NULL && !G_TYPE_CHECK_INSTANCE_TYPE (widget, type))
    widget = gtk_widget_get_parent (widget);
  g_assert_nonnull (widget);
  return widget;
}

/* The entry's row, by its summary. */
static GtkWidget *
row_of (Fixture *fixture, const char *summary)
{
  GtkWidget *label = label_under (window_of (fixture), summary);

  g_assert_nonnull (label);
  return ancestor (label, GTK_TYPE_LIST_BOX_ROW);
}

/* The summaries of the rows, in their order, ";"-joined. */
static char *
summaries (Fixture *fixture)
{
  GtkWidget *list = named_under (window_of (fixture), "history");
  GString *joined = g_string_new (NULL);
  GtkListBoxRow *row;

  g_assert_nonnull (list);
  for (int i = 0; (row = gtk_list_box_get_row_at_index (GTK_LIST_BOX (list), i)) != NULL; i++)
    {
      GtkWidget *summary = named_under (GTK_WIDGET (row), "summary");

      g_string_append_printf (joined, "%s%s", i > 0 ? ";" : "",
                              gtk_label_get_text (GTK_LABEL (summary)));
    }
  return g_string_free (joined, FALSE);
}

static GtkWidget *
clear_all_button (Fixture *fixture)
{
  return ancestor (label_under (window_of (fixture), CLEAR_ALL), GTK_TYPE_BUTTON);
}

static void
test_an_empty_history_says_so (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  /* Empty until the first update. */
  g_assert_true (shows (fixture, NO_ENTRIES));
  two_entries (fixture);
  nt_popup_update (fixture->popup, fixture->entries, NOW);
  g_assert_false (shows (fixture, NO_ENTRIES));
  g_ptr_array_set_size (fixture->entries, 0);
  nt_popup_update (fixture->popup, fixture->entries, NOW);
  g_assert_true (shows (fixture, NO_ENTRIES));
  g_assert_false (gtk_widget_get_sensitive (clear_all_button (fixture)));
}

static void
test_the_entries_show_newest_first (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *order = NULL;
  GtkWidget *lunch;

  two_entries (fixture);
  nt_popup_update (fixture->popup, fixture->entries, NOW);
  order = summaries (fixture);
  g_assert_cmpstr (order, ==, "Lunch?;Invoice");
  lunch = row_of (fixture, "Lunch?");
  g_assert_nonnull (label_under (lunch, "Mail"));
  g_assert_nonnull (label_under (lunch, "5 min ago"));
  /* The body's markup is drawn, not shown. */
  g_assert_nonnull (label_under (lunch, "From Ana"));
  g_assert_nonnull (label_under (lunch, "Reply"));
  g_assert_nonnull (label_under (lunch, "Archive"));
  g_assert_nonnull (label_under (row_of (fixture, "Invoice"), "3 h ago"));
  g_assert_null (label_under (row_of (fixture, "Invoice"), "Reply"));
  g_assert_true (gtk_widget_get_sensitive (clear_all_button (fixture)));
}

static void
test_an_update_replaces_the_entries (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *order = NULL;

  two_entries (fixture);
  nt_popup_update (fixture->popup, fixture->entries, NOW);
  g_ptr_array_remove_index (fixture->entries, 0);
  nt_popup_update (fixture->popup, fixture->entries, NOW);
  order = summaries (fixture);
  g_assert_cmpstr (order, ==, "Invoice");
}

static void
test_a_click_on_an_entry_invokes_its_default_action (Fixture *fixture,
                                                     gconstpointer data G_GNUC_UNUSED)
{
  two_entries (fixture);
  nt_popup_update (fixture->popup, fixture->entries, NOW);
  caminito_applet_popup_show (fixture->applet_popup);
  gtk_widget_activate (row_of (fixture, "Lunch?"));
  g_assert_cmpstr (fixture->calls->str, ==, "invoke 7 default;");
  g_assert_false (caminito_applet_popup_is_shown (fixture->applet_popup));
  /* Invoice has no default action: a click does nothing. */
  gtk_widget_activate (row_of (fixture, "Invoice"));
  g_assert_cmpstr (fixture->calls->str, ==, "invoke 7 default;");
}

static void
test_an_action_button_invokes_its_action (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  two_entries (fixture);
  nt_popup_update (fixture->popup, fixture->entries, NOW);
  gtk_button_clicked (GTK_BUTTON (ancestor (label_under (row_of (fixture, "Lunch?"), "Archive"),
                                            GTK_TYPE_BUTTON)));
  g_assert_cmpstr (fixture->calls->str, ==, "invoke 7 archive;");
  g_assert_false (caminito_applet_popup_is_shown (fixture->applet_popup));
}

static void
test_the_close_button_dismisses_the_entry (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  two_entries (fixture);
  nt_popup_update (fixture->popup, fixture->entries, NOW);
  gtk_button_clicked (GTK_BUTTON (named_under (row_of (fixture, "Invoice"), "dismiss")));
  g_assert_cmpstr (fixture->calls->str, ==, "dismiss 3;");
}

static void
test_clear_all_clears_the_history (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  two_entries (fixture);
  nt_popup_update (fixture->popup, fixture->entries, NOW);
  g_assert_true (shows (fixture, CLEAR_ALL));
  caminito_applet_popup_show (fixture->applet_popup);
  gtk_button_clicked (GTK_BUTTON (clear_all_button (fixture)));
  g_assert_cmpstr (fixture->calls->str, ==, "clear all;");
  /* And closes the center (CMN-285). */
  g_assert_false (caminito_applet_popup_is_shown (fixture->applet_popup));
}

static void
test_an_image_is_shown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  static const guint8 pixels[] = { 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255 };
  NtEntry *photo = entry (9, "Photo", "", 60, FALSE, NULL);
  GtkWidget *image;
  GdkPixbuf *pixbuf;

  photo->image_pixels = g_variant_ref_sink (
    g_variant_new ("(iiibii@ay)", 2, 2, 6, FALSE, 8, 3,
                   g_variant_new_fixed_array (G_VARIANT_TYPE_BYTE, pixels, sizeof pixels, 1)));
  g_ptr_array_add (fixture->entries, photo);
  two_entries (fixture);
  nt_popup_update (fixture->popup, fixture->entries, NOW);
  image = named_under (row_of (fixture, "Photo"), "image");
  g_assert_nonnull (image);
  g_assert_true (gtk_widget_get_visible (image));
  pixbuf = gtk_image_get_pixbuf (GTK_IMAGE (image));
  g_assert_nonnull (pixbuf);
  /* Scaled up to the image's place, keeping its shape. */
  g_assert_cmpint (gdk_pixbuf_get_width (pixbuf), ==, gdk_pixbuf_get_height (pixbuf));
  /* Lunch? has none. */
  g_assert_false (widget_shows (fixture, named_under (row_of (fixture, "Lunch?"), "image")));
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
  g_test_add ("/popup/an-empty-history-says-so", Fixture, NULL, setup, test_an_empty_history_says_so, teardown);
  g_test_add ("/popup/the-entries-show-newest-first", Fixture, NULL, setup, test_the_entries_show_newest_first, teardown);
  g_test_add ("/popup/an-update-replaces-the-entries", Fixture, NULL, setup, test_an_update_replaces_the_entries, teardown);
  g_test_add ("/popup/a-click-on-an-entry-invokes-its-default-action", Fixture, NULL, setup, test_a_click_on_an_entry_invokes_its_default_action, teardown);
  g_test_add ("/popup/an-action-button-invokes-its-action", Fixture, NULL, setup, test_an_action_button_invokes_its_action, teardown);
  g_test_add ("/popup/the-close-button-dismisses-the-entry", Fixture, NULL, setup, test_the_close_button_dismisses_the_entry, teardown);
  g_test_add ("/popup/clear-all-clears-the-history-and-closes-the-center", Fixture, NULL, setup, test_clear_all_clears_the_history, teardown);
  g_test_add ("/popup/an-image-is-shown", Fixture, NULL, setup, test_an_image_is_shown, teardown);
  return g_test_run ();
}
