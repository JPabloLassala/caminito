/*
 * The Network applet's popup (CMN-193, CMN-196). See nw-popup.h.
 *
 * The widgets are built once and updated in place, so that the tab chosen last stays
 * chosen while the panel runs.
 */

#include "nw-popup.h"

#define POPUP_WIDTH     380
#define CARD_ICON_SIZE  48
/* Room for the Networks and Details tabs, empty until their tickets (CMN-200, CMN-201). */
#define TABS_HEIGHT     160

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

static GtkWidget *
tabs_new (void)
{
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  GtkWidget *stack = gtk_stack_new ();
  GtkWidget *switcher = gtk_stack_switcher_new ();

  gtk_stack_add_titled (GTK_STACK (stack), gtk_box_new (GTK_ORIENTATION_VERTICAL, 0),
                        "networks", "Networks");
  gtk_stack_add_titled (GTK_STACK (stack), gtk_box_new (GTK_ORIENTATION_VERTICAL, 0),
                        "details", "Details");
  gtk_widget_set_size_request (stack, -1, TABS_HEIGHT);
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
  gtk_box_pack_start (GTK_BOX (content), tabs_new (), TRUE, TRUE, 0);
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
}

void
nw_popup_free (NwPopup *popup)
{
  g_free (popup);
}
