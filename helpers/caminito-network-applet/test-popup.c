/*
 * The popup's content (CMN-193, CMN-196, CMN-200): with NetworkManager not running it says
 * so, and once NetworkManager runs it shows the tiles, the card and the Networks tab, drawn
 * from statuses built by hand, and calls back for what the user clicks. Without a display,
 * GTK can't start, and it's skipped.
 */

#include "nw-popup.h"

#define NOT_RUNNING "NetworkManager isn't running"

typedef struct
{
  GtkWidget           *anchor;
  CaminitoAppletPopup *applet_popup;
  NwPopup             *popup;
  /* What the popup called back, as "wifi off", "disconnect", "configure"… */
  GString             *calls;
} Fixture;

static const char *const tile_names[] = { "wifi", "wired", "vpn", "airplane" };

static void
on_set_tile (NwTile tile, gboolean on, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "%s %s;", tile_names[tile], on ? "on" : "off");
}

static void
on_disconnect (gpointer data)
{
  Fixture *fixture = data;

  g_string_append (fixture->calls, "disconnect;");
}

static void
on_configure (gpointer data)
{
  Fixture *fixture = data;

  g_string_append (fixture->calls, "configure;");
}

static void
on_join (const char *key, const char *password, gpointer data)
{
  Fixture *fixture = data;

  g_string_append_printf (fixture->calls, "join %s%s%s;", key, password ? " " : "",
                          password ? password : "");
}

static void
on_create_wifi (gpointer data)
{
  Fixture *fixture = data;

  g_string_append (fixture->calls, "create wifi;");
}

static const NwPopupActions actions = {
  .set_tile = on_set_tile,
  .disconnect = on_disconnect,
  .configure = on_configure,
  .join = on_join,
  .create_wifi = on_create_wifi,
};

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  fixture->anchor = g_object_ref_sink (gtk_button_new ());
  fixture->applet_popup = caminito_applet_popup_new (fixture->anchor);
  fixture->popup = nw_popup_new (fixture->applet_popup, &actions, fixture);
  fixture->calls = g_string_new (NULL);
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  nw_popup_free (fixture->popup);
  /* The applet's popup lives as long as the applet: its button here. */
  gtk_widget_destroy (fixture->anchor);
  g_object_unref (fixture->anchor);
  g_string_free (fixture->calls, TRUE);
}

typedef struct
{
  const char *text;
  GType       type;
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
  else if (search->text == NULL && G_TYPE_CHECK_INSTANCE_TYPE (widget, search->type))
    search->found = widget;
  if (GTK_IS_CONTAINER (widget))
    gtk_container_forall (GTK_CONTAINER (widget), visit, search);
}

static GtkWidget *
window_of (Fixture *fixture)
{
  return caminito_applet_popup_get_window (fixture->applet_popup);
}

/* The label with text under widget, NULL with none. */
static GtkWidget *
label_under (GtkWidget *widget, const char *text)
{
  Search search = { text, G_TYPE_INVALID, NULL };

  visit (widget, &search);
  return search.found;
}

static GtkWidget *
find_label (Fixture *fixture, const char *text)
{
  return label_under (window_of (fixture), text);
}

/* The first widget of the type. */
static GtkWidget *
find_type (Fixture *fixture, GType type)
{
  Search search = { NULL, type, NULL };

  visit (window_of (fixture), &search);
  g_assert_nonnull (search.found);
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

      /* A list's filter hides a row with its child visibility. */
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
  return widget_shows (fixture, find_label (fixture, text));
}

/* The button around the label with text: a tile's, by its name, or the card's. */
static GtkWidget *
button_of (Fixture *fixture, const char *text)
{
  GtkWidget *widget = find_label (fixture, text);

  g_assert_nonnull (widget);
  while (widget != NULL && !GTK_IS_BUTTON (widget))
    widget = gtk_widget_get_parent (widget);
  g_assert_nonnull (widget);
  return widget;
}

static NwStatus
office (void)
{
  return (NwStatus) {
    .running = TRUE,
    .phase = NW_PHASE_CONNECTED,
    .kind = NW_KIND_WIRED,
    .name = "Office",
    .strength = -1,
    .address = "192.168.1.20",
    .speed = 1000,
    .wifi_enabled = TRUE,
    .wired = NW_WIRED_CONNECTED,
    .last_vpn = "Work",
  };
}

static void
test_not_running_says_so (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  NwStatus status = office ();
  NwStatus not_running = { .running = FALSE };

  /* Until NetworkManager's reader answers, it isn't running. */
  g_assert_true (shows (fixture, NOT_RUNNING));
  nw_popup_update (fixture->popup, &status);
  g_assert_false (shows (fixture, NOT_RUNNING));
  g_assert_true (shows (fixture, "Wi-Fi"));
  nw_popup_update (fixture->popup, &not_running);
  g_assert_true (shows (fixture, NOT_RUNNING));
  g_assert_false (shows (fixture, "Wi-Fi"));
}

static void
assert_tile (Fixture *fixture, const char *name, gboolean on, gboolean sensitive,
             const char *text)
{
  GtkWidget *button = button_of (fixture, name);

  g_assert_cmpint (gtk_toggle_button_get_active (GTK_TOGGLE_BUTTON (button)), ==, on);
  g_assert_cmpint (gtk_widget_get_sensitive (button), ==, sensitive);
  g_assert_true (widget_shows (fixture, label_under (button, text)));
}

static void
test_tiles (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  NwStatus status = office ();

  nw_popup_update (fixture->popup, &status);
  assert_tile (fixture, "Wi-Fi", TRUE, TRUE, "On");
  assert_tile (fixture, "Wired", TRUE, TRUE, "Connected");
  assert_tile (fixture, "VPN", FALSE, TRUE, "Off");
  assert_tile (fixture, "Airplane mode", FALSE, TRUE, "Off");
  status.vpn = "Work";
  status.wifi_enabled = FALSE;
  status.airplane = TRUE;
  nw_popup_update (fixture->popup, &status);
  assert_tile (fixture, "Wi-Fi", FALSE, FALSE, "Off");
  assert_tile (fixture, "VPN", TRUE, TRUE, "Work");
  assert_tile (fixture, "Airplane mode", TRUE, TRUE, "On");
}

static void
test_tile_click (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  NwStatus status = office ();
  GtkWidget *wifi;

  nw_popup_update (fixture->popup, &status);
  wifi = button_of (fixture, "Wi-Fi");
  gtk_button_clicked (GTK_BUTTON (wifi));
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "VPN")));
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "Airplane mode")));
  g_assert_cmpstr (fixture->calls->str, ==, "wifi off;vpn on;airplane on;");
  /* The tile stays as NetworkManager has it until NetworkManager follows. */
  g_assert_true (gtk_toggle_button_get_active (GTK_TOGGLE_BUTTON (wifi)));
  status.wifi_enabled = FALSE;
  nw_popup_update (fixture->popup, &status);
  g_assert_false (gtk_toggle_button_get_active (GTK_TOGGLE_BUTTON (wifi)));
  /* An update draws the tile without calling back. */
  g_assert_cmpstr (fixture->calls->str, ==, "wifi off;vpn on;airplane on;");
}

static void
test_card (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  NwStatus status = office ();
  GtkWidget *spinner = NULL;

  nw_popup_update (fixture->popup, &status);
  g_assert_true (shows (fixture, "Office"));
  g_assert_true (shows (fixture, "Connected · 1 Gb/s"));
  g_assert_true (shows (fixture, "192.168.1.20"));
  g_assert_true (shows (fixture, "Disconnect"));
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "Disconnect")));
  g_assert_cmpstr (fixture->calls->str, ==, "disconnect;");
  spinner = find_type (fixture, GTK_TYPE_SPINNER);
  g_assert_false (widget_shows (fixture, spinner));

  status = (NwStatus) { .running = TRUE, .phase = NW_PHASE_CONNECTING, .kind = NW_KIND_WIFI,
                        .name = "Home", .strength = 70, .wifi_enabled = TRUE };
  nw_popup_update (fixture->popup, &status);
  g_assert_true (shows (fixture, "Connecting to Home…"));
  g_assert_true (widget_shows (fixture, spinner));
  g_assert_false (shows (fixture, "Disconnect"));
  g_assert_false (shows (fixture, "192.168.1.20"));

  status = (NwStatus) { .running = TRUE, .strength = -1, .wifi_enabled = TRUE };
  nw_popup_update (fixture->popup, &status);
  g_assert_true (shows (fixture, "Not connected"));
  g_assert_false (widget_shows (fixture, spinner));
  g_assert_false (shows (fixture, "Disconnect"));
}

static void
test_tab_is_kept (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  NwStatus status = office ();
  NwStatus not_running = { .running = FALSE };
  GtkWidget *switcher;
  GtkStack *tabs;
  g_autofree char *title = NULL;

  nw_popup_update (fixture->popup, &status);
  switcher = find_type (fixture, GTK_TYPE_STACK_SWITCHER);
  tabs = gtk_stack_switcher_get_stack (GTK_STACK_SWITCHER (switcher));
  g_assert_true (widget_shows (fixture, switcher));
  g_assert_cmpstr (gtk_stack_get_visible_child_name (tabs), ==, "networks");
  gtk_container_child_get (GTK_CONTAINER (tabs), gtk_stack_get_child_by_name (tabs, "details"),
                           "title", &title, NULL);
  g_assert_cmpstr (title, ==, "Details");
  gtk_stack_set_visible_child_name (tabs, "details");
  caminito_applet_popup_hide (fixture->applet_popup);
  nw_popup_update (fixture->popup, &not_running);
  nw_popup_update (fixture->popup, &status);
  g_assert_cmpstr (gtk_stack_get_visible_child_name (tabs), ==, "details");
}

static void
test_configure (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  NwStatus status = office ();

  nw_popup_update (fixture->popup, &status);
  g_assert_true (shows (fixture, "Configure network connections…"));
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "Configure network connections…")));
  g_assert_cmpstr (fixture->calls->str, ==, "configure;");
  g_assert_false (caminito_applet_popup_is_shown (fixture->applet_popup));
}

/* The Networks tab's rows, in nw_networks_sort()'s order: Work, a VPN; Library, saved;
   Cafe, open; Old, WEP; eduroam, Enterprise. */
static NwNetwork *
networks (void)
{
  static NwNetwork networks[] = {
    { .key = "uuid:work", .name = "Work", .kind = NW_NETWORK_VPN, .strength = -1,
      .saved = TRUE },
    { .key = "ssid:Library", .name = "Library", .kind = NW_NETWORK_WIFI,
      .security = NW_SECURITY_PSK, .strength = 80, .saved = TRUE },
    { .key = "ssid:eduroam", .name = "eduroam", .kind = NW_NETWORK_WIFI,
      .security = NW_SECURITY_ENTERPRISE, .strength = 60 },
    { .key = "ssid:Cafe", .name = "Cafe", .kind = NW_NETWORK_WIFI, .strength = 40 },
    { .key = "ssid:Old", .name = "Old", .kind = NW_NETWORK_WIFI, .security = NW_SECURITY_WEP,
      .strength = 20 },
  };

  for (guint i = 0; i < G_N_ELEMENTS (networks); i++)
    networks[i].connecting = networks[i].active = FALSE;
  return networks;
}

static NwStatus
with_networks (void)
{
  NwStatus status = office ();

  status.networks = networks ();
  status.n_networks = 5;
  return status;
}

static GtkListBox *
list_of (Fixture *fixture)
{
  return GTK_LIST_BOX (find_type (fixture, GTK_TYPE_LIST_BOX));
}

/* The rows shown, in order: "Work, Library 80%, …". */
static char *
rows_shown (Fixture *fixture)
{
  GString *rows = g_string_new (NULL);
  GtkListBox *list = list_of (fixture);
  GtkListBoxRow *row;

  for (int i = 0; (row = gtk_list_box_get_row_at_index (list, i)) != NULL; i++)
    {
      NwNetwork *network = networks ();

      if (!widget_shows (fixture, GTK_WIDGET (row)))
        continue;
      for (guint j = 0; j < 5; j++)
        if (label_under (GTK_WIDGET (row), network[j].name) != NULL)
          {
            g_autofree char *strength = g_strdup_printf ("%d%%", network[j].strength);

            g_string_append_printf (rows, "%s%s", rows->len > 0 ? ", " : "", network[j].name);
            if (label_under (GTK_WIDGET (row), strength) != NULL)
              g_string_append_printf (rows, " %s", strength);
          }
    }
  return g_string_free (rows, FALSE);
}

static void
select_row (Fixture *fixture, const char *name)
{
  GtkWidget *widget = find_label (fixture, name);

  while (!GTK_IS_LIST_BOX_ROW (widget))
    widget = gtk_widget_get_parent (widget);
  gtk_list_box_select_row (list_of (fixture), GTK_LIST_BOX_ROW (widget));
}

static void
visit_password (GtkWidget *widget, gpointer data)
{
  GtkWidget **found = data;

  if (*found != NULL)
    return;
  /* The search field is an entry too. */
  if (GTK_IS_ENTRY (widget) && !GTK_IS_SEARCH_ENTRY (widget))
    *found = widget;
  else if (GTK_IS_CONTAINER (widget))
    gtk_container_forall (GTK_CONTAINER (widget), visit_password, found);
}

/* The bar's password field. */
static GtkEntry *
password_entry (Fixture *fixture)
{
  GtkWidget *found = NULL;

  visit_password (window_of (fixture), &found);
  g_assert_nonnull (found);
  return GTK_ENTRY (found);
}

static void
test_the_list (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  NwStatus status = with_networks ();
  g_autofree char *rows = NULL;
  GtkWidget *search = find_type (fixture, GTK_TYPE_SEARCH_ENTRY);

  nw_popup_update (fixture->popup, &status);
  rows = rows_shown (fixture);
  g_assert_cmpstr (rows, ==, "Work, Library 80%, eduroam 60%, Cafe 40%, Old 20%");
  g_clear_pointer (&rows, g_free);
  /* A search, in any case. */
  gtk_entry_set_text (GTK_ENTRY (search), "LIB");
  rows = rows_shown (fixture);
  g_assert_cmpstr (rows, ==, "Library 80%");
  g_clear_pointer (&rows, g_free);
  gtk_entry_set_text (GTK_ENTRY (search), "");
  rows = rows_shown (fixture);
  g_assert_cmpstr (rows, ==, "Work, Library 80%, eduroam 60%, Cafe 40%, Old 20%");
}

/* The bar's text field for a password, NULL where it isn't shown. */
static GtkWidget *
password_shown (Fixture *fixture)
{
  GtkWidget *entry = GTK_WIDGET (password_entry (fixture));

  return entry != NULL && widget_shows (fixture, entry) ? entry : NULL;
}

static void
test_the_bar (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  NwStatus status = with_networks ();
  GtkWidget *password;

  nw_popup_update (fixture->popup, &status);
  /* Nothing selected, nothing to join. */
  g_assert_null (password_shown (fixture));
  g_assert_false (shows (fixture, "Join"));

  select_row (fixture, "Old");
  password = password_shown (fixture);
  g_assert_nonnull (password);
  /* Ready to type in. */
  g_assert_true (gtk_widget_is_focus (password));
  g_assert_true (shows (fixture, "Join"));
  gtk_entry_set_text (GTK_ENTRY (password), "s3cret");
  /* Enter joins. */
  g_signal_emit_by_name (password, "activate");
  g_assert_cmpstr (fixture->calls->str, ==, "join ssid:Old s3cret;");
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "Join")));
  g_assert_cmpstr (fixture->calls->str, ==, "join ssid:Old s3cret;join ssid:Old s3cret;");
  g_string_truncate (fixture->calls, 0);

  /* The password stays while the network stays selected, and goes with another one. */
  nw_popup_update (fixture->popup, &status);
  g_assert_cmpstr (gtk_entry_get_text (GTK_ENTRY (password_shown (fixture))), ==, "s3cret");

  select_row (fixture, "Library");
  g_assert_null (password_shown (fixture));
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "Join Library")));
  select_row (fixture, "Cafe");
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "Join Cafe")));
  select_row (fixture, "Work");
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "Connect Work")));
  g_assert_cmpstr (fixture->calls->str, ==, "join ssid:Library;join ssid:Cafe;join uuid:work;");

  select_row (fixture, "Old");
  g_assert_cmpstr (gtk_entry_get_text (GTK_ENTRY (password_shown (fixture))), ==, "");
}

static void
test_connecting (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  NwStatus status = with_networks ();
  GtkWidget *spinner;
  gboolean spinning;

  nw_popup_update (fixture->popup, &status);
  select_row (fixture, "Library");
  status.networks[1].connecting = TRUE;
  nw_popup_update (fixture->popup, &status);
  g_assert_true (shows (fixture, "Connecting to Library…"));
  g_assert_false (shows (fixture, "Join Library"));
  /* Beside it, a spinner, which the card's isn't. */
  spinner = find_label (fixture, "Connecting to Library…");
  {
    Search search = { NULL, GTK_TYPE_SPINNER, NULL };

    visit (gtk_widget_get_parent (spinner), &search);
    spinner = search.found;
  }
  g_assert_true (widget_shows (fixture, spinner));
  g_object_get (spinner, "active", &spinning, NULL);
  g_assert_true (spinning);
  /* Up: nothing left to do. */
  status.networks[1].connecting = FALSE;
  status.networks[1].active = TRUE;
  nw_popup_update (fixture->popup, &status);
  g_assert_false (shows (fixture, "Connecting to Library…"));
  g_assert_false (shows (fixture, "Join Library"));
}

static void
test_editor (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  NwStatus status = with_networks ();

  nw_popup_update (fixture->popup, &status);
  g_assert_true (shows (fixture, "Connect to hidden network…"));
  select_row (fixture, "eduroam");
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "Join eduroam…")));
  g_assert_cmpstr (fixture->calls->str, ==, "create wifi;");
  g_assert_false (caminito_applet_popup_is_shown (fixture->applet_popup));
  gtk_button_clicked (GTK_BUTTON (button_of (fixture, "Connect to hidden network…")));
  g_assert_cmpstr (fixture->calls->str, ==, "create wifi;create wifi;");
  /* Only while Wi-Fi is on. */
  status.wifi_enabled = FALSE;
  nw_popup_update (fixture->popup, &status);
  g_assert_false (shows (fixture, "Connect to hidden network…"));
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
  g_test_add ("/popup/the-tiles-show-the-status", Fixture, NULL, setup, test_tiles, teardown);
  g_test_add ("/popup/a-tile-click-asks-for-the-other-state", Fixture, NULL, setup, test_tile_click, teardown);
  g_test_add ("/popup/the-card-shows-the-connection-coming-up-or-none", Fixture, NULL, setup, test_card, teardown);
  g_test_add ("/popup/the-tab-chosen-last-is-kept", Fixture, NULL, setup, test_tab_is_kept, teardown);
  g_test_add ("/popup/configure-closes-the-popup-and-calls-back", Fixture, NULL, setup, test_configure, teardown);
  g_test_add ("/popup/the-networks-are-listed-and-searched", Fixture, NULL, setup, test_the_list, teardown);
  g_test_add ("/popup/the-bar-joins-the-network-selected", Fixture, NULL, setup, test_the_bar, teardown);
  g_test_add ("/popup/the-bar-says-a-network-is-connecting", Fixture, NULL, setup, test_connecting, teardown);
  g_test_add ("/popup/enterprise-and-hidden-networks-open-the-editor", Fixture, NULL, setup, test_editor, teardown);
  return g_test_run ();
}
