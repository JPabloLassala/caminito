/*
 * The Bluetooth applet's popup (260). See bt-popup.h.
 *
 * The window's content is built again on every change, from a copy of what bt-state.c
 * lists, and what the user has open (the expanded rows) and what is under way (calls to
 * BlueZ, their errors) is kept here between builds.
 */

#include "bt-popup.h"

#define POPUP_WIDTH   320
#define ROW_ICON_SIZE 22
/* A switch that BlueZ hasn't followed, because turning on failed, goes back to the state
   BlueZ has. It's longer than the retries for an adapter that's still being unblocked. */
#define SWITCH_RESYNC_MS 3500

struct _BtPopup
{
  CaminitoAppletPopup *popup;
  BtPopupActions       actions;
  gpointer             data;
  GtkWidget           *content;
  /* What BlueZ had at the last update, which a rebuild draws again. */
  BtSnapshot          *snapshot;
  gboolean             powered;
  /* Device paths: the rows that are open, and the ones with a call under way. */
  GHashTable          *expanded;
  GHashTable          *busy;
  /* Device path to the message of its last failed call, shown in its details. */
  GHashTable          *errors;
  guint                resync;
};

typedef struct
{
  BtPopup  *popup;
  char     *path;
  gboolean  connected;
} RowData;

static void rebuild (BtPopup *popup);

/* A window doesn't shrink when its content does, so a popup that has listed more devices,
   or had a row open, would stay as tall as it was. Asking for the least brings it back to
   what the content needs, at the width it has. */
static void
fit_window (BtPopup *popup)
{
  gtk_window_resize (GTK_WINDOW (caminito_applet_popup_get_window (popup->popup)),
                     POPUP_WIDTH, 1);
}

static void
on_revealed (GObject *revealer G_GNUC_UNUSED, GParamSpec *pspec G_GNUC_UNUSED, gpointer data)
{
  fit_window (data);
}

static void
add_styles (void)
{
  static gboolean added;
  g_autoptr (GtkCssProvider) provider = NULL;

  if (added)
    return;
  added = TRUE;
  provider = gtk_css_provider_new ();
  /* Red for a battery below 20%, as in the settings window. */
  gtk_css_provider_load_from_data (provider,
                                   ".caminito-bluetooth-low { color: #c01c28; }"
                                   ".caminito-bluetooth-error { color: #c01c28; }",
                                   -1, NULL);
  gtk_style_context_add_provider_for_screen (gdk_screen_get_default (),
                                             GTK_STYLE_PROVIDER (provider),
                                             GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
}

static void
row_data_free (gpointer data, GClosure *closure G_GNUC_UNUSED)
{
  RowData *row = data;

  g_free (row->path);
  g_free (row);
}

static GtkWidget *
label_new (const char *text, gboolean dim)
{
  GtkWidget *label = gtk_label_new (text);

  gtk_label_set_xalign (GTK_LABEL (label), 0);
  if (dim)
    gtk_style_context_add_class (gtk_widget_get_style_context (label), GTK_STYLE_CLASS_DIM_LABEL);
  return label;
}

static GtkWidget *
icon_new (char **names, GtkIconSize size, int pixel_size)
{
  g_autoptr (GIcon) icon = g_themed_icon_new_from_names (names, -1);
  GtkWidget *image = gtk_image_new_from_gicon (icon, size);

  if (pixel_size > 0)
    gtk_image_set_pixel_size (GTK_IMAGE (image), pixel_size);
  return image;
}

static void
on_row_clicked (GtkButton *button, gpointer data)
{
  RowData *row = data;
  GtkWidget *revealer = g_object_get_data (G_OBJECT (button), "revealer");

  /* Opened or closed in place: building the rows again would cut its animation. */
  if (g_hash_table_remove (row->popup->expanded, row->path))
    gtk_revealer_set_reveal_child (GTK_REVEALER (revealer), FALSE);
  else
    {
      g_hash_table_add (row->popup->expanded, g_strdup (row->path));
      gtk_revealer_set_reveal_child (GTK_REVEALER (revealer), TRUE);
    }
}

static void
on_connect_clicked (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  RowData *row = data;
  BtPopup *popup = row->popup;
  /* The row's path outlives the rebuild below. */
  g_autofree char *path = g_strdup (row->path);
  gboolean connect = !row->connected;

  g_hash_table_add (popup->busy, g_strdup (path));
  g_hash_table_remove (popup->errors, path);
  popup->actions.set_connected (path, connect, popup->data);
  rebuild (popup);
}

static GtkWidget *
row_new (BtPopup *popup, const BtDevice *device)
{
  g_auto (GStrv) names = bt_state_device_icon_names (device->icon);
  gboolean open = g_hash_table_contains (popup->expanded, device->path);
  gboolean busy = g_hash_table_contains (popup->busy, device->path);
  const char *error = g_hash_table_lookup (popup->errors, device->path);
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *header = gtk_button_new ();
  GtkWidget *line = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *name = label_new (device->alias, FALSE);
  GtkWidget *revealer = gtk_revealer_new ();
  GtkWidget *details = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  GtkWidget *action;
  g_autofree char *details_line = g_strdup_printf ("%s · %s", bt_state_device_kind (device->icon),
                                           device->address);
  RowData *row = g_new0 (RowData, 1);

  row->popup = popup;
  row->path = g_strdup (device->path);
  row->connected = device->connected;

  gtk_button_set_relief (GTK_BUTTON (header), GTK_RELIEF_NONE);
  gtk_label_set_ellipsize (GTK_LABEL (name), PANGO_ELLIPSIZE_END);
  gtk_widget_set_hexpand (name, TRUE);
  gtk_box_pack_start (GTK_BOX (line), icon_new (names, GTK_ICON_SIZE_LARGE_TOOLBAR, ROW_ICON_SIZE),
                      FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (line), name, TRUE, TRUE, 0);
  if (device->battery >= 0)
    {
      g_autofree char *percent = g_strdup_printf ("%d%%", device->battery);
      GtkWidget *battery = label_new (percent, !bt_state_battery_low (device->battery));

      if (bt_state_battery_low (device->battery))
        gtk_style_context_add_class (gtk_widget_get_style_context (battery),
                                     "caminito-bluetooth-low");
      gtk_box_pack_end (GTK_BOX (line), battery, FALSE, FALSE, 0);
    }
  gtk_container_add (GTK_CONTAINER (header), line);
  /* The row's data is the Connect button's, which goes with the row. */
  g_signal_connect_data (header, "clicked", G_CALLBACK (on_row_clicked), row, NULL, 0);

  gtk_box_pack_start (GTK_BOX (details), label_new (details_line, TRUE), FALSE, FALSE, 0);
  if (error != NULL)
    {
      GtkWidget *failure = label_new (error, FALSE);

      gtk_label_set_line_wrap (GTK_LABEL (failure), TRUE);
      gtk_label_set_max_width_chars (GTK_LABEL (failure), 36);
      gtk_style_context_add_class (gtk_widget_get_style_context (failure),
                                   "caminito-bluetooth-error");
      gtk_box_pack_start (GTK_BOX (details), failure, FALSE, FALSE, 0);
    }
  action = gtk_button_new_with_label (busy ? (device->connected ? "Disconnecting…" : "Connecting…")
                                           : (device->connected ? "Disconnect" : "Connect"));
  gtk_widget_set_sensitive (action, !busy);
  gtk_widget_set_halign (action, GTK_ALIGN_START);
  g_signal_connect_data (action, "clicked", G_CALLBACK (on_connect_clicked), row, row_data_free,
                         0);
  gtk_box_pack_start (GTK_BOX (details), action, FALSE, FALSE, 0);
  gtk_widget_set_margin_start (details, 38);
  gtk_widget_set_margin_end (details, 6);
  gtk_widget_set_margin_top (details, 2);
  gtk_widget_set_margin_bottom (details, 6);
  gtk_container_add (GTK_CONTAINER (revealer), details);
  /* A row that was open stays open as drawn, with no animation of its own. */
  gtk_revealer_set_transition_duration (GTK_REVEALER (revealer), 0);
  gtk_revealer_set_reveal_child (GTK_REVEALER (revealer), open);
  gtk_revealer_set_transition_duration (GTK_REVEALER (revealer), 150);
  g_object_set_data (G_OBJECT (header), "revealer", revealer);
  /* The row has closed all the way: the window may shrink. */
  g_signal_connect (revealer, "notify::child-revealed", G_CALLBACK (on_revealed), popup);

  gtk_box_pack_start (GTK_BOX (box), header, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), revealer, FALSE, FALSE, 0);
  return box;
}

/* A dim line in place of the list. */
static GtkWidget *
empty_note_new (const char *text)
{
  GtkWidget *note = label_new (text, TRUE);

  gtk_widget_set_halign (note, GTK_ALIGN_CENTER);
  gtk_widget_set_margin_top (note, 24);
  gtk_widget_set_margin_bottom (note, 24);
  return note;
}

static void
add_section (BtPopup *popup, GtkWidget *box, const char *title, GPtrArray *devices)
{
  GtkWidget *heading;
  g_autofree char *markup = NULL;

  if (devices->len == 0)
    return;
  markup = g_markup_printf_escaped ("<b>%s</b>", title);
  heading = label_new (NULL, TRUE);
  gtk_label_set_markup (GTK_LABEL (heading), markup);
  gtk_widget_set_margin_top (heading, 6);
  gtk_widget_set_margin_start (heading, 6);
  gtk_box_pack_start (GTK_BOX (box), heading, FALSE, FALSE, 0);
  for (guint i = 0; i < devices->len; i++)
    gtk_box_pack_start (GTK_BOX (box), row_new (popup, g_ptr_array_index (devices, i)), FALSE,
                        FALSE, 0);
}

static gboolean
resync (gpointer data)
{
  BtPopup *popup = data;

  popup->resync = 0;
  rebuild (popup);
  return G_SOURCE_REMOVE;
}

static gboolean
on_switch_state (GtkSwitch *toggle G_GNUC_UNUSED, gboolean on, gpointer data)
{
  BtPopup *popup = data;

  popup->actions.set_powered (on, popup->data);
  /* The switch follows BlueZ's answer, which rebuilds it; if there's none, it goes back. */
  g_clear_handle_id (&popup->resync, g_source_remove);
  popup->resync = g_timeout_add (SWITCH_RESYNC_MS, resync, popup);
  return FALSE;
}

static void
on_add_device (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  BtPopup *popup = data;

  caminito_applet_popup_hide (popup->popup);
  popup->actions.add_device (popup->data);
}

static void
on_configure (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  BtPopup *popup = data;

  caminito_applet_popup_hide (popup->popup);
  popup->actions.configure (popup->data);
}

static GtkWidget *
header_new (BtPopup *popup)
{
  GtkWidget *header = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *title = gtk_label_new (NULL);
  GtkWidget *toggle = gtk_switch_new ();

  gtk_label_set_markup (GTK_LABEL (title), "<big><b>Bluetooth</b></big>");
  gtk_label_set_xalign (GTK_LABEL (title), 0);
  gtk_widget_set_margin_start (title, 6);
  gtk_switch_set_active (GTK_SWITCH (toggle), popup->powered);
  gtk_widget_set_valign (toggle, GTK_ALIGN_CENTER);
  g_signal_connect (toggle, "state-set", G_CALLBACK (on_switch_state), popup);
  gtk_box_pack_start (GTK_BOX (header), title, TRUE, TRUE, 0);
  gtk_box_pack_end (GTK_BOX (header), toggle, FALSE, FALSE, 0);
  return header;
}

static GtkWidget *
footer_new (BtPopup *popup)
{
  GtkWidget *footer = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 4);
  GtkWidget *add = gtk_button_new_with_label ("Add New Device…");
  const char *configure_names[] = {
    "emblem-system-symbolic", "preferences-system-symbolic", "preferences-system", NULL,
  };
  GtkWidget *configure = gtk_button_new ();

  gtk_button_set_relief (GTK_BUTTON (add), GTK_RELIEF_NONE);
  gtk_widget_set_sensitive (add, popup->powered);
  g_signal_connect (add, "clicked", G_CALLBACK (on_add_device), popup);
  gtk_button_set_relief (GTK_BUTTON (configure), GTK_RELIEF_NONE);
  gtk_button_set_image (GTK_BUTTON (configure),
                        icon_new ((char **) configure_names, GTK_ICON_SIZE_BUTTON, 0));
  gtk_widget_set_tooltip_text (configure, "Configure Bluetooth…");
  g_signal_connect (configure, "clicked", G_CALLBACK (on_configure), popup);
  gtk_box_pack_start (GTK_BOX (footer), add, FALSE, FALSE, 0);
  gtk_box_pack_end (GTK_BOX (footer), configure, FALSE, FALSE, 0);
  return footer;
}

static void
rebuild (BtPopup *popup)
{
  GtkWidget *window = caminito_applet_popup_get_window (popup->popup);
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 4);
  g_autoptr (GPtrArray) connected = bt_state_listed (popup->snapshot, TRUE);
  g_autoptr (GPtrArray) available = bt_state_listed (popup->snapshot, FALSE);

  g_clear_handle_id (&popup->resync, g_source_remove);
  if (popup->content != NULL)
    gtk_widget_destroy (popup->content);
  gtk_container_set_border_width (GTK_CONTAINER (box), 8);
  gtk_widget_set_size_request (box, POPUP_WIDTH, -1);
  gtk_box_pack_start (GTK_BOX (box), header_new (popup), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), gtk_separator_new (GTK_ORIENTATION_HORIZONTAL), FALSE,
                      FALSE, 2);
  if (!popup->powered)
    gtk_box_pack_start (GTK_BOX (box), empty_note_new ("Bluetooth is off."), FALSE, FALSE, 0);
  else if (connected->len == 0 && available->len == 0)
    gtk_box_pack_start (GTK_BOX (box), empty_note_new ("No paired devices."), FALSE, FALSE, 0);
  else
    {
      add_section (popup, box, "Connected", connected);
      add_section (popup, box, "Available", available);
    }
  gtk_box_pack_start (GTK_BOX (box), gtk_separator_new (GTK_ORIENTATION_HORIZONTAL), FALSE,
                      FALSE, 2);
  gtk_box_pack_start (GTK_BOX (box), footer_new (popup), FALSE, FALSE, 0);
  gtk_widget_show_all (box);
  gtk_container_add (GTK_CONTAINER (window), box);
  fit_window (popup);
  popup->content = box;
  g_object_add_weak_pointer (G_OBJECT (box), (gpointer *) &popup->content);
}

void
bt_popup_update (BtPopup *popup, const BtSnapshot *snapshot)
{
  BtIconState state = bt_state_icon (snapshot);

  g_clear_pointer (&popup->snapshot, bt_snapshot_free);
  popup->snapshot = bt_snapshot_copy (snapshot);
  popup->powered = state != BT_ICON_OFF && state != BT_ICON_NONE;
  rebuild (popup);
}

void
bt_popup_connect_done (BtPopup *popup, const char *path, const char *error)
{
  g_hash_table_remove (popup->busy, path);
  if (error != NULL)
    {
      g_hash_table_insert (popup->errors, g_strdup (path), g_strdup (error));
      /* The row is open to show the error, which it would otherwise hide. */
      g_hash_table_add (popup->expanded, g_strdup (path));
    }
  rebuild (popup);
}

BtPopup *
bt_popup_new (CaminitoAppletPopup *applet_popup, const BtPopupActions *actions, gpointer data)
{
  BtPopup *popup = g_new0 (BtPopup, 1);

  add_styles ();
  popup->popup = applet_popup;
  popup->actions = *actions;
  popup->data = data;
  popup->snapshot = bt_snapshot_new ();
  popup->expanded = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  popup->busy = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  popup->errors = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, g_free);
  rebuild (popup);
  return popup;
}

void
bt_popup_free (BtPopup *popup)
{
  g_clear_handle_id (&popup->resync, g_source_remove);
  if (popup->content != NULL)
    {
      g_object_remove_weak_pointer (G_OBJECT (popup->content), (gpointer *) &popup->content);
      gtk_widget_destroy (popup->content);
    }
  bt_snapshot_free (popup->snapshot);
  g_hash_table_unref (popup->expanded);
  g_hash_table_unref (popup->busy);
  g_hash_table_unref (popup->errors);
  g_free (popup);
}
