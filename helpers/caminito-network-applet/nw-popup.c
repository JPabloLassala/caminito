/*
 * The Network applet's popup (CMN-193, CMN-196, CMN-200). See nw-popup.h.
 *
 * The widgets are built once and updated in place, so that the tab chosen last stays
 * chosen while the panel runs. The Networks tab's rows are built again on each update, and
 * the network selected, known by its key, is selected again, with the password typed for
 * it kept.
 */

#include "nw-popup.h"

#define POPUP_WIDTH     380
#define CARD_ICON_SIZE  48
/* The Networks tab's list: seven rows, and a scroll bar for the rest. */
#define LIST_HEIGHT     250

static const struct
{
  const char       *name;
  const char *const icons[4];
} tile_kinds[] = {
  [NW_TILE_WIFI] = { "Wi-Fi",
                     { "network-wireless-symbolic", "network-wireless", NULL } },
  [NW_TILE_WIRED] = { "Wired", { "network-wired-symbolic", "network-wired", NULL } },
  [NW_TILE_VPN] = { "VPN",
                    { "network-vpn-symbolic", "network-vpn", "changes-prevent-symbolic", NULL } },
  [NW_TILE_AIRPLANE] = { "Airplane mode",
                         { "airplane-mode-symbolic", "airplane-mode", "network-flightmode-on",
                           NULL } },
};

G_STATIC_ASSERT (G_N_ELEMENTS (tile_kinds) == NW_N_TILES);

typedef struct
{
  NwPopup   *popup;
  NwTile     kind;
  GtkWidget *button;
  GtkWidget *text;
  /* What NetworkManager has, which a click doesn't change. */
  gboolean   on;
} Tile;

struct _NwPopup
{
  CaminitoAppletPopup *applet_popup;
  NwPopupActions       actions;
  gpointer             data;
  GtkWidget           *stack;
  Tile                 tiles[NW_N_TILES];
  GtkWidget           *card_icon;
  GtkWidget           *card_title;
  GtkWidget           *card_detail;
  GtkWidget           *card_address;
  GtkWidget           *spinner;
  GtkWidget           *disconnect;

  /* The Networks tab. */
  GtkWidget           *search;
  GtkWidget           *list;
  /* Under the list: a stack of a page for each NwBar but NW_BAR_NONE. */
  GtkWidget           *bar;
  NwBar                bar_kind;
  GtkWidget           *password;
  GtkWidget           *password_join;
  GtkWidget           *join;
  GtkWidget           *bar_spinner;
  GtkWidget           *connecting;
  GtkWidget           *hidden;
  /* The key of the network selected, NULL with none. */
  char                *selected;
  /* The rows are being built again: their selection isn't the user's. */
  gboolean             rebuilding;
};

static void
add_styles (void)
{
  static gboolean added;
  g_autoptr (GtkCssProvider) provider = NULL;

  if (added)
    return;
  added = TRUE;
  provider = gtk_css_provider_new ();
  /* The tiles and the card as the prototype has them: rounded, on a grey a little darker
     than the popup's, and a tile that's on in the theme's selection colour. */
  gtk_css_provider_load_from_data (
    provider,
    ".caminito-network-tile, .caminito-network-card { padding: 8px 10px; border-radius: 10px;"
    " border: none; box-shadow: none; background-image: none;"
    " background-color: rgba(127, 127, 127, 0.12); }"
    ".caminito-network-tile:hover { background-color: rgba(127, 127, 127, 0.2); }"
    ".caminito-network-tile:checked { background-color: @theme_selected_bg_color;"
    " color: @theme_selected_fg_color; }"
    ".caminito-network-tile:checked label { color: @theme_selected_fg_color; }"
    ".caminito-network-title { font-weight: bold; }"
    ".caminito-network-card-title { font-weight: bold; font-size: 115%; }",
    -1, NULL);
  gtk_style_context_add_provider_for_screen (gdk_screen_get_default (),
                                             GTK_STYLE_PROVIDER (provider),
                                             GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
}

static void
add_class (GtkWidget *widget, const char *name)
{
  gtk_style_context_add_class (gtk_widget_get_style_context (widget), name);
}

static GtkWidget *
label_new (const char *text, const char *style)
{
  GtkWidget *label = gtk_label_new (text);

  gtk_label_set_xalign (GTK_LABEL (label), 0);
  gtk_label_set_ellipsize (GTK_LABEL (label), PANGO_ELLIPSIZE_END);
  if (style != NULL)
    add_class (label, style);
  return label;
}

static void
set_icon (GtkWidget *image, const char *const *names, GtkIconSize size)
{
  g_autoptr (GIcon) icon = g_themed_icon_new_from_names ((char **) names, -1);

  gtk_image_set_from_gicon (GTK_IMAGE (image), icon, size);
}

/* What the popup is with NetworkManager not running: the panel's icon over its tooltip. */
static GtkWidget *
not_running_page (void)
{
  const NwStatus not_running = { .running = FALSE };
  g_autofree char *text = nw_state_tooltip (&not_running);
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 12);
  GtkWidget *icon = gtk_image_new ();

  set_icon (icon, nw_state_icon_names (&not_running), GTK_ICON_SIZE_DIALOG);
  gtk_image_set_pixel_size (GTK_IMAGE (icon), 64);
  gtk_container_set_border_width (GTK_CONTAINER (box), 24);
  gtk_box_pack_start (GTK_BOX (box), icon, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), gtk_label_new (text), FALSE, FALSE, 0);
  return box;
}

static void on_tile_toggled (GtkToggleButton *button, gpointer data);

/* Draws the tile as NetworkManager has it, without calling back. */
static void
sync_tile (Tile *tile)
{
  g_signal_handlers_block_by_func (tile->button, on_tile_toggled, tile);
  gtk_toggle_button_set_active (GTK_TOGGLE_BUTTON (tile->button), tile->on);
  g_signal_handlers_unblock_by_func (tile->button, on_tile_toggled, tile);
}

static void
on_tile_toggled (GtkToggleButton *button G_GNUC_UNUSED, gpointer data)
{
  Tile *tile = data;
  NwPopup *popup = tile->popup;

  /* Back as it was: it moves when NetworkManager has done it, and stays if it refuses. */
  sync_tile (tile);
  popup->actions.set_tile (tile->kind, !tile->on, popup->data);
}

static GtkWidget *
tile_new (NwPopup *popup, NwTile kind)
{
  Tile *tile = &popup->tiles[kind];
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 10);
  GtkWidget *labels = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *icon = gtk_image_new ();

  tile->popup = popup;
  tile->kind = kind;
  tile->button = gtk_toggle_button_new ();
  add_class (tile->button, "caminito-network-tile");
  set_icon (icon, tile_kinds[kind].icons, GTK_ICON_SIZE_LARGE_TOOLBAR);
  tile->text = label_new ("Off", GTK_STYLE_CLASS_DIM_LABEL);
  gtk_box_pack_start (GTK_BOX (labels), label_new (tile_kinds[kind].name,
                                                   "caminito-network-title"),
                      FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (labels), tile->text, FALSE, FALSE, 0);
  gtk_widget_set_valign (labels, GTK_ALIGN_CENTER);
  gtk_box_pack_start (GTK_BOX (box), icon, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), labels, TRUE, TRUE, 0);
  gtk_container_add (GTK_CONTAINER (tile->button), box);
  g_signal_connect (tile->button, "toggled", G_CALLBACK (on_tile_toggled), tile);
  return tile->button;
}

static GtkWidget *
tiles_new (NwPopup *popup)
{
  GtkWidget *grid = gtk_grid_new ();

  gtk_grid_set_row_spacing (GTK_GRID (grid), 8);
  gtk_grid_set_column_spacing (GTK_GRID (grid), 8);
  gtk_grid_set_column_homogeneous (GTK_GRID (grid), TRUE);
  for (NwTile kind = 0; kind < NW_N_TILES; kind++)
    gtk_grid_attach (GTK_GRID (grid), tile_new (popup, kind), kind % 2, kind / 2, 1, 1);
  return grid;
}

static void
on_disconnect (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  NwPopup *popup = data;

  popup->actions.disconnect (popup->data);
}

static GtkWidget *
card_new (NwPopup *popup)
{
  GtkWidget *card = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 12);
  GtkWidget *labels = gtk_box_new (GTK_ORIENTATION_VERTICAL, 2);

  add_class (card, "caminito-network-card");
  popup->card_icon = gtk_image_new ();
  gtk_image_set_pixel_size (GTK_IMAGE (popup->card_icon), CARD_ICON_SIZE);
  popup->card_title = label_new (NULL, "caminito-network-card-title");
  popup->card_detail = label_new (NULL, GTK_STYLE_CLASS_DIM_LABEL);
  popup->card_address = label_new (NULL, GTK_STYLE_CLASS_DIM_LABEL);
  /* Beside Disconnect, "Connected · No internet · 1 Gb/s" doesn't fit on one line, and
     ellipsized it would lose the speed. */
  gtk_label_set_ellipsize (GTK_LABEL (popup->card_detail), PANGO_ELLIPSIZE_NONE);
  gtk_label_set_line_wrap (GTK_LABEL (popup->card_detail), TRUE);
  gtk_label_set_ellipsize (GTK_LABEL (popup->card_address), PANGO_ELLIPSIZE_NONE);
  gtk_label_set_selectable (GTK_LABEL (popup->card_address), TRUE);
  popup->spinner = gtk_spinner_new ();
  popup->disconnect = gtk_button_new_with_label ("Disconnect");
  gtk_widget_set_valign (popup->disconnect, GTK_ALIGN_CENTER);
  g_signal_connect (popup->disconnect, "clicked", G_CALLBACK (on_disconnect), popup);
  gtk_box_pack_start (GTK_BOX (labels), popup->card_title, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (labels), popup->card_detail, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (labels), popup->card_address, FALSE, FALSE, 0);
  gtk_widget_set_valign (labels, GTK_ALIGN_CENTER);
  gtk_box_pack_start (GTK_BOX (card), popup->card_icon, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (card), labels, TRUE, TRUE, 0);
  gtk_box_pack_end (GTK_BOX (card), popup->disconnect, FALSE, FALSE, 0);
  gtk_box_pack_end (GTK_BOX (card), popup->spinner, FALSE, FALSE, 0);
  return card;
}

static const NwNetwork *
network_of (GtkListBoxRow *row)
{
  return row != NULL ? g_object_get_data (G_OBJECT (row), "caminito-network") : NULL;
}

static GtkListBoxRow *
selected_row (NwPopup *popup)
{
  return gtk_list_box_get_selected_row (GTK_LIST_BOX (popup->list));
}

static void
network_free (gpointer data)
{
  NwNetwork *network = data;

  g_free (network->key);
  g_free (network->name);
  g_free (network);
}

/* A row: the network's icon, its name, a lock if it's secured, and its signal. */
static GtkWidget *
row_new (const NwNetwork *network)
{
  GtkWidget *row = gtk_list_box_row_new ();
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *icon = gtk_image_new ();
  NwNetwork *copy = g_new (NwNetwork, 1);

  *copy = *network;
  copy->key = g_strdup (network->key);
  copy->name = g_strdup (network->name);
  g_object_set_data_full (G_OBJECT (row), "caminito-network", copy, network_free);
  gtk_container_set_border_width (GTK_CONTAINER (box), 4);
  set_icon (icon, nw_network_icon_names (network), GTK_ICON_SIZE_LARGE_TOOLBAR);
  gtk_box_pack_start (GTK_BOX (box), icon, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), label_new (network->name, NULL), TRUE, TRUE, 0);
  if (network->security != NW_SECURITY_NONE)
    {
      GtkWidget *lock = gtk_image_new ();

      set_icon (lock, nw_state_lock_names (), GTK_ICON_SIZE_MENU);
      gtk_box_pack_start (GTK_BOX (box), lock, FALSE, FALSE, 0);
    }
  if (network->strength >= 0)
    {
      g_autofree char *strength = g_strdup_printf ("%d%%", network->strength);

      gtk_box_pack_start (GTK_BOX (box), label_new (strength, GTK_STYLE_CLASS_DIM_LABEL),
                          FALSE, FALSE, 0);
    }
  gtk_container_add (GTK_CONTAINER (row), box);
  gtk_widget_show_all (row);
  return row;
}

static gboolean
filter_row (GtkListBoxRow *row, gpointer data)
{
  NwPopup *popup = data;

  return nw_network_matches (network_of (row), gtk_entry_get_text (GTK_ENTRY (popup->search)));
}

static void
on_search_changed (GtkEditable *search G_GNUC_UNUSED, gpointer data)
{
  NwPopup *popup = data;

  gtk_list_box_invalidate_filter (GTK_LIST_BOX (popup->list));
}

/* Draws the bar for network, NULL with none selected. */
static void
update_bar (NwPopup *popup, const NwNetwork *network)
{
  g_autofree char *text = NULL;

  popup->bar_kind = network != NULL ? nw_state_bar (network, &text) : NW_BAR_NONE;
  gtk_widget_set_visible (popup->bar, popup->bar_kind != NW_BAR_NONE);
  switch (popup->bar_kind)
    {
    case NW_BAR_PASSWORD:
      gtk_button_set_label (GTK_BUTTON (popup->password_join), text);
      gtk_stack_set_visible_child_name (GTK_STACK (popup->bar), "password");
      break;
    case NW_BAR_JOIN:
    case NW_BAR_EDITOR:
      gtk_button_set_label (GTK_BUTTON (popup->join), text);
      gtk_stack_set_visible_child_name (GTK_STACK (popup->bar), "join");
      break;
    case NW_BAR_CONNECTING:
      gtk_label_set_text (GTK_LABEL (popup->connecting), text);
      gtk_stack_set_visible_child_name (GTK_STACK (popup->bar), "connecting");
      break;
    case NW_BAR_NONE:
      break;
    }
  if (popup->bar_kind == NW_BAR_CONNECTING)
    gtk_spinner_start (GTK_SPINNER (popup->bar_spinner));
  else
    gtk_spinner_stop (GTK_SPINNER (popup->bar_spinner));
  /* A password is for the network it was typed for, while it's asked for. */
  if (popup->bar_kind != NW_BAR_PASSWORD)
    gtk_entry_set_text (GTK_ENTRY (popup->password), "");
}

/* Ready to type in, for a network that asks for a password. */
static void
focus_password (NwPopup *popup)
{
  if (popup->bar_kind == NW_BAR_PASSWORD)
    gtk_widget_grab_focus (popup->password);
}

static void
on_row_selected (GtkListBox *list G_GNUC_UNUSED, GtkListBoxRow *row, gpointer data)
{
  NwPopup *popup = data;
  const NwNetwork *network = network_of (row);

  if (popup->rebuilding)
    return;
  if (network == NULL || g_strcmp0 (network->key, popup->selected) != 0)
    gtk_entry_set_text (GTK_ENTRY (popup->password), "");
  g_free (popup->selected);
  popup->selected = network != NULL ? g_strdup (network->key) : NULL;
  update_bar (popup, network);
  focus_password (popup);
}

/* A click focuses its row after it's selected. */
static void
on_row_activated (GtkListBox *list G_GNUC_UNUSED, GtkListBoxRow *row G_GNUC_UNUSED,
                  gpointer data)
{
  focus_password (data);
}

static void
join_selected (NwPopup *popup)
{
  const NwNetwork *network = network_of (selected_row (popup));

  if (network == NULL)
    return;
  switch (popup->bar_kind)
    {
    case NW_BAR_PASSWORD:
      popup->actions.join (network->key, gtk_entry_get_text (GTK_ENTRY (popup->password)),
                           popup->data);
      break;
    case NW_BAR_JOIN:
      popup->actions.join (network->key, NULL, popup->data);
      break;
    case NW_BAR_EDITOR:
      caminito_applet_popup_hide (popup->applet_popup);
      popup->actions.create_wifi (popup->data);
      break;
    case NW_BAR_CONNECTING:
    case NW_BAR_NONE:
      break;
    }
}

static void
on_join (GtkWidget *widget G_GNUC_UNUSED, gpointer data)
{
  join_selected (data);
}

static void
on_password_changed (GtkEditable *password, gpointer data)
{
  NwPopup *popup = data;

  gtk_widget_set_sensitive (popup->password_join,
                            *gtk_entry_get_text (GTK_ENTRY (password)) != '\0');
}

static void
on_password_activate (GtkEntry *password, gpointer data)
{
  if (*gtk_entry_get_text (password) != '\0')
    join_selected (data);
}

static void
on_hidden (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  NwPopup *popup = data;

  caminito_applet_popup_hide (popup->applet_popup);
  popup->actions.create_wifi (popup->data);
}

/* The bar under the list: a password and "Join", a button, or a spinner and its line. */
static GtkWidget *
bar_new (NwPopup *popup)
{
  GtkWidget *password_page = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 6);
  GtkWidget *connecting_page = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 8);

  popup->bar = gtk_stack_new ();
  popup->password = gtk_entry_new ();
  gtk_entry_set_visibility (GTK_ENTRY (popup->password), FALSE);
  gtk_entry_set_input_purpose (GTK_ENTRY (popup->password), GTK_INPUT_PURPOSE_PASSWORD);
  gtk_entry_set_placeholder_text (GTK_ENTRY (popup->password), "Password");
  g_signal_connect (popup->password, "changed", G_CALLBACK (on_password_changed), popup);
  g_signal_connect (popup->password, "activate", G_CALLBACK (on_password_activate), popup);
  popup->password_join = gtk_button_new_with_label ("Join");
  gtk_widget_set_sensitive (popup->password_join, FALSE);
  g_signal_connect (popup->password_join, "clicked", G_CALLBACK (on_join), popup);
  gtk_box_pack_start (GTK_BOX (password_page), popup->password, TRUE, TRUE, 0);
  gtk_box_pack_start (GTK_BOX (password_page), popup->password_join, FALSE, FALSE, 0);
  gtk_stack_add_named (GTK_STACK (popup->bar), password_page, "password");

  popup->join = gtk_button_new_with_label ("");
  gtk_widget_set_halign (popup->join, GTK_ALIGN_END);
  g_signal_connect (popup->join, "clicked", G_CALLBACK (on_join), popup);
  gtk_stack_add_named (GTK_STACK (popup->bar), popup->join, "join");

  popup->bar_spinner = gtk_spinner_new ();
  popup->connecting = label_new (NULL, NULL);
  gtk_box_pack_start (GTK_BOX (connecting_page), popup->bar_spinner, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (connecting_page), popup->connecting, TRUE, TRUE, 0);
  gtk_stack_add_named (GTK_STACK (popup->bar), connecting_page, "connecting");
  gtk_widget_show_all (popup->bar);
  /* Hidden until a row is selected. */
  gtk_widget_set_no_show_all (popup->bar, TRUE);
  gtk_widget_hide (popup->bar);
  return popup->bar;
}

/* The search, the list, the bar, and "Connect to hidden network…". */
static GtkWidget *
networks_page_new (NwPopup *popup)
{
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  GtkWidget *scrolled = gtk_scrolled_window_new (NULL, NULL);

  popup->search = gtk_search_entry_new ();
  /* At once, not after GtkSearchEntry's pause: the list is short. */
  g_signal_connect (popup->search, "changed", G_CALLBACK (on_search_changed), popup);
  popup->list = gtk_list_box_new ();
  gtk_list_box_set_filter_func (GTK_LIST_BOX (popup->list), filter_row, popup, NULL);
  g_signal_connect (popup->list, "row-selected", G_CALLBACK (on_row_selected), popup);
  g_signal_connect (popup->list, "row-activated", G_CALLBACK (on_row_activated), popup);
  gtk_scrolled_window_set_policy (GTK_SCROLLED_WINDOW (scrolled), GTK_POLICY_NEVER,
                                  GTK_POLICY_AUTOMATIC);
  gtk_scrolled_window_set_min_content_height (GTK_SCROLLED_WINDOW (scrolled), LIST_HEIGHT);
  gtk_scrolled_window_set_shadow_type (GTK_SCROLLED_WINDOW (scrolled), GTK_SHADOW_IN);
  gtk_container_add (GTK_CONTAINER (scrolled), popup->list);
  popup->hidden = gtk_button_new_with_label ("Connect to hidden network…");
  gtk_button_set_relief (GTK_BUTTON (popup->hidden), GTK_RELIEF_NONE);
  gtk_widget_set_halign (popup->hidden, GTK_ALIGN_START);
  g_signal_connect (popup->hidden, "clicked", G_CALLBACK (on_hidden), popup);
  gtk_box_pack_start (GTK_BOX (box), popup->search, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), scrolled, TRUE, TRUE, 0);
  gtk_box_pack_start (GTK_BOX (box), bar_new (popup), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), popup->hidden, FALSE, FALSE, 0);
  return box;
}

static GtkWidget *
tabs_new (NwPopup *popup)
{
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  GtkWidget *stack = gtk_stack_new ();
  GtkWidget *switcher = gtk_stack_switcher_new ();

  gtk_stack_add_titled (GTK_STACK (stack), networks_page_new (popup), "networks", "Networks");
  /* Empty until its ticket (CMN-201). */
  gtk_stack_add_titled (GTK_STACK (stack), gtk_box_new (GTK_ORIENTATION_VERTICAL, 0),
                        "details", "Details");
  gtk_stack_switcher_set_stack (GTK_STACK_SWITCHER (switcher), GTK_STACK (stack));
  gtk_widget_set_halign (switcher, GTK_ALIGN_CENTER);
  gtk_box_pack_start (GTK_BOX (box), switcher, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), stack, TRUE, TRUE, 0);
  return box;
}

static void
on_configure (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  NwPopup *popup = data;

  caminito_applet_popup_hide (popup->applet_popup);
  popup->actions.configure (popup->data);
}

static GtkWidget *
footer_new (NwPopup *popup)
{
  GtkWidget *configure = gtk_button_new_with_label ("Configure network connections…");

  gtk_button_set_relief (GTK_BUTTON (configure), GTK_RELIEF_NONE);
  gtk_widget_set_halign (configure, GTK_ALIGN_START);
  g_signal_connect (configure, "clicked", G_CALLBACK (on_configure), popup);
  return configure;
}

NwPopup *
nw_popup_new (CaminitoAppletPopup *applet_popup, const NwPopupActions *actions,
              gpointer data)
{
  NwPopup *popup = g_new0 (NwPopup, 1);
  GtkWidget *content = gtk_box_new (GTK_ORIENTATION_VERTICAL, 12);

  add_styles ();
  popup->applet_popup = applet_popup;
  popup->actions = *actions;
  popup->data = data;
  gtk_widget_set_size_request (content, POPUP_WIDTH, -1);
  gtk_container_set_border_width (GTK_CONTAINER (content), 12);
  gtk_box_pack_start (GTK_BOX (content), tiles_new (popup), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (content), card_new (popup), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (content), tabs_new (popup), TRUE, TRUE, 0);
  gtk_box_pack_start (GTK_BOX (content), footer_new (popup), FALSE, FALSE, 0);
  popup->stack = gtk_stack_new ();
  gtk_stack_add_named (GTK_STACK (popup->stack), not_running_page (), "not-running");
  gtk_stack_add_named (GTK_STACK (popup->stack), content, "content");
  gtk_widget_show_all (popup->stack);
  gtk_stack_set_visible_child_name (GTK_STACK (popup->stack), "not-running");
  gtk_container_add (GTK_CONTAINER (caminito_applet_popup_get_window (applet_popup)),
                     popup->stack);
  return popup;
}

static void
update_card (NwPopup *popup, const NwStatus *status)
{
  g_autofree char *title = nw_state_card_title (status);
  g_autofree char *detail = nw_state_card_detail (status);
  gboolean connected = status->phase == NW_PHASE_CONNECTED;
  gboolean connecting = status->phase == NW_PHASE_CONNECTING;

  set_icon (popup->card_icon, nw_state_icon_names (status), GTK_ICON_SIZE_DIALOG);
  gtk_label_set_text (GTK_LABEL (popup->card_title), title);
  gtk_label_set_text (GTK_LABEL (popup->card_detail), detail);
  gtk_widget_set_visible (popup->card_detail, detail != NULL);
  gtk_label_set_text (GTK_LABEL (popup->card_address), connected ? status->address : NULL);
  gtk_widget_set_visible (popup->card_address, connected && status->address != NULL);
  gtk_widget_set_visible (popup->disconnect, connected);
  gtk_widget_set_visible (popup->spinner, connecting);
  if (connecting)
    gtk_spinner_start (GTK_SPINNER (popup->spinner));
  else
    gtk_spinner_stop (GTK_SPINNER (popup->spinner));
}

/* The rows built again from status, with the network selected selected again. */
static void
update_networks (NwPopup *popup, const NwStatus *status)
{
  g_autoptr (GList) rows = gtk_container_get_children (GTK_CONTAINER (popup->list));
  GtkWidget *selected = NULL;

  popup->rebuilding = TRUE;
  for (GList *row = rows; row != NULL; row = row->next)
    gtk_widget_destroy (row->data);
  for (guint i = 0; i < status->n_networks; i++)
    {
      GtkWidget *row = row_new (&status->networks[i]);

      gtk_container_add (GTK_CONTAINER (popup->list), row);
      if (g_strcmp0 (status->networks[i].key, popup->selected) == 0)
        selected = row;
    }
  if (selected != NULL)
    gtk_list_box_select_row (GTK_LIST_BOX (popup->list), GTK_LIST_BOX_ROW (selected));
  else
    g_clear_pointer (&popup->selected, g_free);
  popup->rebuilding = FALSE;
  update_bar (popup, network_of (GTK_LIST_BOX_ROW (selected)));
  gtk_widget_set_visible (popup->hidden, nw_state_tile (status, NW_TILE_WIFI).on);
}

void
nw_popup_update (NwPopup *popup, const NwStatus *status)
{
  gtk_stack_set_visible_child_name (GTK_STACK (popup->stack),
                                    status->running ? "content" : "not-running");
  if (!status->running)
    return;
  for (NwTile kind = 0; kind < NW_N_TILES; kind++)
    {
      Tile *tile = &popup->tiles[kind];
      NwTileState state = nw_state_tile (status, kind);

      tile->on = state.on;
      sync_tile (tile);
      gtk_widget_set_sensitive (tile->button, state.sensitive);
      gtk_label_set_text (GTK_LABEL (tile->text), state.text);
    }
  update_card (popup, status);
  update_networks (popup, status);
}

void
nw_popup_free (NwPopup *popup)
{
  g_free (popup->selected);
  g_free (popup);
}
