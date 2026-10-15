/*
 * The Bluetooth popup's content (260), from snapshots built by hand: what each state
 * lists, and what its controls call. Without a display, GTK can't start, and it's skipped.
 */

#include "bt-popup.h"

#define HCI0 "/org/bluez/hci0"
#define HEADPHONES HCI0 "/dev_11"
#define MOUSE HCI0 "/dev_22"

typedef struct
{
  GtkWidget           *anchor;
  CaminitoAppletPopup *popup;
  BtPopup             *content;
  GString             *calls;
} Fixture;

static void
on_set_powered (gboolean on, gpointer data)
{
  g_string_append_printf (((Fixture *) data)->calls, "powered %d;", on);
}

static void
on_set_connected (const char *path, gboolean connected, gpointer data)
{
  g_string_append_printf (((Fixture *) data)->calls, "connect %s %d;", path, connected);
}

static void
on_add_device (gpointer data)
{
  g_string_append (((Fixture *) data)->calls, "add;");
}

static void
on_configure (gpointer data)
{
  g_string_append (((Fixture *) data)->calls, "configure;");
}

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  static const BtPopupActions actions = {
    on_set_powered, on_set_connected, on_add_device, on_configure,
  };

  fixture->anchor = g_object_ref_sink (gtk_button_new ());
  fixture->popup = caminito_applet_popup_new (fixture->anchor);
  fixture->calls = g_string_new (NULL);
  fixture->content = bt_popup_new (fixture->popup, &actions, fixture);
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  bt_popup_free (fixture->content);
  /* The popup goes with its anchor. */
  g_object_unref (fixture->anchor);
  g_string_free (fixture->calls, TRUE);
}

static BtSnapshot *
snapshot_with_devices (gboolean powered)
{
  BtSnapshot *snapshot = bt_snapshot_new ();

  bt_snapshot_add_adapter (snapshot, HCI0, powered);
  bt_snapshot_add_device (snapshot, HEADPHONES, HCI0, "WH-1000XM4", "38:18:4C:12:9A:01",
                          "audio-headphones", TRUE, TRUE, 80);
  bt_snapshot_add_device (snapshot, MOUSE, HCI0, "MX Master 3", "D4:1B:70:88:05:13",
                          "input-mouse", TRUE, FALSE, 12);
  bt_snapshot_add_device (snapshot, HCI0 "/dev_33", HCI0, "Nearby TV", "A8:23:FE:10:44:5D",
                          "video-display", FALSE, FALSE, -1);
  return snapshot;
}

typedef struct
{
  const char *text;
  GtkWidget  *found;
  GPtrArray  *all;
} Search;

static void
visit (GtkWidget *widget, gpointer data)
{
  Search *search = data;
  const char *text = NULL;

  if (GTK_IS_LABEL (widget))
    text = gtk_label_get_text (GTK_LABEL (widget));
  if (text != NULL)
    {
      g_ptr_array_add (search->all, (gpointer) text);
      if (search->text != NULL && search->found == NULL && g_str_equal (text, search->text))
        search->found = widget;
    }
  if (GTK_IS_CONTAINER (widget))
    gtk_container_forall (GTK_CONTAINER (widget), visit, search);
}

static GtkWidget *
content (Fixture *fixture)
{
  return gtk_bin_get_child (GTK_BIN (caminito_applet_popup_get_window (fixture->popup)));
}

/* The label with this text, or NULL. */
static GtkWidget *
find_label (Fixture *fixture, const char *text)
{
  Search search = { text, NULL, g_ptr_array_new () };

  visit (content (fixture), &search);
  g_ptr_array_unref (search.all);
  return search.found;
}

/* The button around the label with this text. */
static GtkWidget *
find_button (Fixture *fixture, const char *text)
{
  GtkWidget *label = find_label (fixture, text);

  return label != NULL ? gtk_widget_get_ancestor (label, GTK_TYPE_BUTTON) : NULL;
}

/* Every label's text, in order, joined with "|". */
static char *
texts (Fixture *fixture)
{
  Search search = { NULL, NULL, g_ptr_array_new () };
  GString *joined = g_string_new (NULL);

  visit (content (fixture), &search);
  for (guint i = 0; i < search.all->len; i++)
    g_string_append_printf (joined, "%s%s", i > 0 ? "|" : "",
                            (const char *) g_ptr_array_index (search.all, i));
  g_ptr_array_unref (search.all);
  return g_string_free (joined, FALSE);
}

static GtkWidget *
find_switch (GtkWidget *widget)
{
  GtkWidget *found = NULL;
  GList *children;

  if (GTK_IS_SWITCH (widget))
    return widget;
  if (!GTK_IS_CONTAINER (widget))
    return NULL;
  children = gtk_container_get_children (GTK_CONTAINER (widget));
  for (GList *l = children; l != NULL && found == NULL; l = l->next)
    found = find_switch (l->data);
  g_list_free (children);
  return found;
}

static void
test_lists_connected_then_available (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (BtSnapshot) snapshot = snapshot_with_devices (TRUE);
  g_autofree char *all = NULL;

  bt_popup_update (fixture->content, snapshot);
  all = texts (fixture);
  /* The header, each section with its devices, each row with its battery, its details and
     its button, and no device that isn't paired. */
  g_assert_cmpstr (all, ==,
                   "Bluetooth|Connected|WH-1000XM4|80%|Headphones · 38:18:4C:12:9A:01|Disconnect"
                   "|Available|MX Master 3|12%|Mouse · D4:1B:70:88:05:13|Connect"
                   "|Add New Device…");
  g_assert_true (gtk_switch_get_active (GTK_SWITCH (find_switch (content (fixture)))));
}

static void
test_low_battery_is_red (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (BtSnapshot) snapshot = snapshot_with_devices (TRUE);
  GtkStyleContext *low, *fine;

  bt_popup_update (fixture->content, snapshot);
  low = gtk_widget_get_style_context (find_label (fixture, "12%"));
  fine = gtk_widget_get_style_context (find_label (fixture, "80%"));
  g_assert_true (gtk_style_context_has_class (low, "caminito-bluetooth-low"));
  g_assert_false (gtk_style_context_has_class (fine, "caminito-bluetooth-low"));
}

static void
test_off_says_so_in_place_of_the_list (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (BtSnapshot) snapshot = snapshot_with_devices (FALSE);
  g_autofree char *all = NULL;

  bt_popup_update (fixture->content, snapshot);
  all = texts (fixture);
  g_assert_cmpstr (all, ==, "Bluetooth|Bluetooth is off.|Add New Device…");
  g_assert_false (gtk_switch_get_active (GTK_SWITCH (find_switch (content (fixture)))));
  g_assert_false (gtk_widget_get_sensitive (find_button (fixture, "Add New Device…")));
}

static void
test_no_paired_device (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (BtSnapshot) snapshot = bt_snapshot_new ();
  g_autofree char *all = NULL;

  bt_snapshot_add_adapter (snapshot, HCI0, TRUE);
  bt_popup_update (fixture->content, snapshot);
  all = texts (fixture);
  g_assert_cmpstr (all, ==, "Bluetooth|No paired devices.|Add New Device…");
}

static void
test_switch_is_for_bluetooth (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (BtSnapshot) snapshot = snapshot_with_devices (FALSE);

  bt_popup_update (fixture->content, snapshot);
  gtk_switch_set_active (GTK_SWITCH (find_switch (content (fixture))), TRUE);
  g_assert_cmpstr (fixture->calls->str, ==, "powered 1;");
}

/* Connect and Disconnect, from the row's own button, which is busy till the answer. */
static void
test_connect_and_disconnect_call_back (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (BtSnapshot) snapshot = snapshot_with_devices (TRUE);

  bt_popup_update (fixture->content, snapshot);
  gtk_button_clicked (GTK_BUTTON (find_button (fixture, "Connect")));
  g_assert_cmpstr (fixture->calls->str, ==, "connect " MOUSE " 1;");
  /* Drawn again, and busy: it can't be pressed twice. */
  g_assert_null (find_button (fixture, "Connect"));
  g_assert_false (gtk_widget_get_sensitive (find_button (fixture, "Connecting…")));

  bt_popup_connect_done (fixture->content, MOUSE, NULL);
  g_assert_true (gtk_widget_get_sensitive (find_button (fixture, "Connect")));

  g_string_truncate (fixture->calls, 0);
  gtk_button_clicked (GTK_BUTTON (find_button (fixture, "Disconnect")));
  g_assert_cmpstr (fixture->calls->str, ==, "connect " HEADPHONES " 0;");
}

static void
test_failed_connect_shows_the_error (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (BtSnapshot) snapshot = snapshot_with_devices (TRUE);

  bt_popup_update (fixture->content, snapshot);
  gtk_button_clicked (GTK_BUTTON (find_button (fixture, "Connect")));
  bt_popup_connect_done (fixture->content, MOUSE, "Host is down");
  g_assert_nonnull (find_label (fixture, "Host is down"));
  g_assert_true (gtk_widget_get_sensitive (find_button (fixture, "Connect")));
  /* The next try starts clean. */
  gtk_button_clicked (GTK_BUTTON (find_button (fixture, "Connect")));
  g_assert_null (find_label (fixture, "Host is down"));
}

/* A row open stays open through the changes BlueZ sends (a battery level, say). */
static void
test_expanded_row_survives_an_update (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (BtSnapshot) snapshot = snapshot_with_devices (TRUE);
  g_autoptr (BtSnapshot) later = snapshot_with_devices (TRUE);
  GtkWidget *revealer;

  bt_popup_update (fixture->content, snapshot);
  revealer = gtk_widget_get_ancestor (find_label (fixture, "Mouse · D4:1B:70:88:05:13"),
                                      GTK_TYPE_REVEALER);
  g_assert_false (gtk_revealer_get_reveal_child (GTK_REVEALER (revealer)));
  gtk_button_clicked (GTK_BUTTON (find_button (fixture, "MX Master 3")));
  g_assert_true (gtk_revealer_get_reveal_child (GTK_REVEALER (revealer)));

  bt_popup_update (fixture->content, later);
  revealer = gtk_widget_get_ancestor (find_label (fixture, "Mouse · D4:1B:70:88:05:13"),
                                      GTK_TYPE_REVEALER);
  g_assert_true (gtk_revealer_get_reveal_child (GTK_REVEALER (revealer)));
  gtk_button_clicked (GTK_BUTTON (find_button (fixture, "MX Master 3")));
  g_assert_false (gtk_revealer_get_reveal_child (GTK_REVEALER (revealer)));
}

static void
test_footer_runs_the_program_and_closes (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (BtSnapshot) snapshot = snapshot_with_devices (TRUE);

  bt_popup_update (fixture->content, snapshot);
  gtk_button_clicked (GTK_BUTTON (find_button (fixture, "Add New Device…")));
  g_assert_cmpstr (fixture->calls->str, ==, "add;");
  g_assert_false (caminito_applet_popup_is_shown (fixture->popup));
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
  g_test_add ("/popup/lists-connected-then-available-with-details", Fixture, NULL, setup, test_lists_connected_then_available, teardown);
  g_test_add ("/popup/a-battery-below-20-is-red", Fixture, NULL, setup, test_low_battery_is_red, teardown);
  g_test_add ("/popup/off-says-so-in-place-of-the-list", Fixture, NULL, setup, test_off_says_so_in_place_of_the_list, teardown);
  g_test_add ("/popup/no-paired-device-says-so", Fixture, NULL, setup, test_no_paired_device, teardown);
  g_test_add ("/popup/the-switch-is-for-bluetooth", Fixture, NULL, setup, test_switch_is_for_bluetooth, teardown);
  g_test_add ("/popup/connect-and-disconnect-call-back", Fixture, NULL, setup, test_connect_and_disconnect_call_back, teardown);
  g_test_add ("/popup/a-failed-connect-shows-the-error", Fixture, NULL, setup, test_failed_connect_shows_the_error, teardown);
  g_test_add ("/popup/an-open-row-survives-an-update", Fixture, NULL, setup, test_expanded_row_survives_an_update, teardown);
  g_test_add ("/popup/the-footer-runs-the-program-and-closes", Fixture, NULL, setup, test_footer_runs_the_program_and_closes, teardown);
  return g_test_run ();
}
