/*
 * The Power applet's popup (262). See pw-popup.h.
 *
 * The table of batteries (263) is built again when what it shows changes, from the rows
 * pw_state_rows() makes; what the user has open (a battery's ⋮ menu) and what is under way
 * (the call to UPower for a limit, its error) is kept here between builds.
 */

#include "pw-popup.h"

#define POPUP_WIDTH   320
#define ROW_ICON_SIZE 24
#define BAR_WIDTH     56

struct _PwPopup
{
  CaminitoAppletPopup *popup;
  PwPopupActions       actions;
  gpointer             data;
  GtkWidget           *content;
  /* The table, in the box above the footer, and the rows it was drawn from. */
  GtkWidget           *batteries;
  GPtrArray           *rows;
  /* The battery whose ⋮ menu is open, so that a rebuild opens it again. */
  char                *open_path;
  GtkWidget           *open_button;
  /* Battery paths: the limit switches with a call under way; the position the user put a
     switch in, which it keeps until UPower says the same; and the last refusal. */
  GHashTable          *busy;
  GHashTable          *requested;
  GHashTable          *errors;
};

/* What the user asked of a limit switch, as the table keeps it. */
enum
{
  REQUESTED_NONE = 0,
  REQUESTED_OFF,
  REQUESTED_ON,
};

typedef struct
{
  PwPopup *popup;
  char    *path;
} RowData;

static void
row_data_free (gpointer data, GClosure *closure G_GNUC_UNUSED)
{
  RowData *row = data;

  g_free (row->path);
  g_free (row);
}

static RowData *
row_data_new (PwPopup *popup, const char *path)
{
  RowData *row = g_new0 (RowData, 1);

  row->popup = popup;
  row->path = g_strdup (path);
  return row;
}

/* A window doesn't shrink when its content does: asking for the least brings it back to
   what the content needs, at the width it has. */
static void
fit_window (PwPopup *popup)
{
  gtk_window_resize (GTK_WINDOW (caminito_applet_popup_get_window (popup->popup)),
                     POPUP_WIDTH, 1);
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
  /* The warning colour, as in the Bluetooth applet's low batteries. */
  gtk_css_provider_load_from_data (provider,
                                   ".caminito-power-low { color: #c01c28; }"
                                   "levelbar.caminito-power-low block.filled {"
                                   " background-image: none; background-color: #c01c28; }"
                                   ".caminito-power-error { color: #c01c28; }",
                                   -1, NULL);
  gtk_style_context_add_provider_for_screen (gdk_screen_get_default (),
                                             GTK_STYLE_PROVIDER (provider),
                                             GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
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

static void
on_configure (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  PwPopup *popup = data;

  caminito_applet_popup_hide (popup->popup);
  popup->actions.configure (popup->data);
}

static gboolean
on_limit_state_set (GtkSwitch *toggle, gboolean on, gpointer data)
{
  RowData *row = data;
  PwPopup *popup = row->popup;
  /* The call's answer rebuilds the table, which frees this row. */
  g_autofree char *path = g_strdup (row->path);

  g_hash_table_add (popup->busy, g_strdup (path));
  g_hash_table_insert (popup->requested, g_strdup (path), GINT_TO_POINTER (on ? REQUESTED_ON : REQUESTED_OFF));
  g_hash_table_remove (popup->errors, path);
  /* Not pressed again until UPower, or polkit, has answered. */
  gtk_widget_set_sensitive (GTK_WIDGET (toggle), FALSE);
  popup->actions.set_limit (path, on, popup->data);
  /* The switch goes where it was put, and goes back if UPower won't. */
  return FALSE;
}

static void
on_menu_toggled (GtkToggleButton *button, gpointer data)
{
  RowData *row = data;
  PwPopup *popup = row->popup;

  if (gtk_toggle_button_get_active (button))
    {
      /* One menu at a time: the other's button isn't pressed any more. */
      GtkWidget *other = popup->open_button;

      popup->open_button = GTK_WIDGET (button);
      g_free (popup->open_path);
      popup->open_path = g_strdup (row->path);
      if (other != NULL && other != GTK_WIDGET (button))
        gtk_toggle_button_set_active (GTK_TOGGLE_BUTTON (other), FALSE);
    }
  else if (popup->open_button == GTK_WIDGET (button))
    {
      popup->open_button = NULL;
      g_clear_pointer (&popup->open_path, g_free);
    }
}

static GtkWidget *
menu_new (PwPopup *popup, const PwRow *row)
{
  GtkWidget *button = gtk_menu_button_new ();
  GtkWidget *menu = gtk_popover_new (button);
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 8);
  const char *error = g_hash_table_lookup (popup->errors, row->path);

  /* A glyph that every font has, where a themed icon would be one the theme may lack. */
  gtk_container_add (GTK_CONTAINER (button), gtk_label_new ("⋮"));
  gtk_button_set_relief (GTK_BUTTON (button), GTK_RELIEF_NONE);
  gtk_widget_set_valign (button, GTK_ALIGN_CENTER);
  gtk_widget_set_tooltip_text (button, "Health and charge limit");
  gtk_menu_button_set_popover (GTK_MENU_BUTTON (button), menu);
  /* A modal popover takes GTK's grab, which closes the applet's popup around it. A click
     on the button, or Escape, closes it, and so does the popup. */
  gtk_popover_set_modal (GTK_POPOVER (menu), FALSE);
  gtk_popover_set_position (GTK_POPOVER (menu), GTK_POS_BOTTOM);
  g_signal_connect_data (button, "toggled", G_CALLBACK (on_menu_toggled),
                         row_data_new (popup, row->path), row_data_free, 0);

  gtk_container_set_border_width (GTK_CONTAINER (box), 10);
  gtk_box_pack_start (GTK_BOX (box), label_new (row->health, FALSE), FALSE, FALSE, 0);
  if (row->limit_supported)
    {
      GtkWidget *line = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 12);
      g_autofree char *text = g_strdup_printf ("Limit charging to %u%%", row->limit_end);
      GtkWidget *toggle = gtk_switch_new ();
      int requested;

      requested = GPOINTER_TO_INT (g_hash_table_lookup (popup->requested, row->path));
      gtk_switch_set_active (GTK_SWITCH (toggle),
                             requested != REQUESTED_NONE ? requested == REQUESTED_ON
                                                    : row->limit_enabled);
      gtk_widget_set_valign (toggle, GTK_ALIGN_CENTER);
      gtk_widget_set_sensitive (toggle, !g_hash_table_contains (popup->busy, row->path));
      g_signal_connect_data (toggle, "state-set", G_CALLBACK (on_limit_state_set),
                             row_data_new (popup, row->path), row_data_free, 0);
      gtk_box_pack_start (GTK_BOX (line), label_new (text, FALSE), TRUE, TRUE, 0);
      gtk_box_pack_end (GTK_BOX (line), toggle, FALSE, FALSE, 0);
      gtk_box_pack_start (GTK_BOX (box), line, FALSE, FALSE, 0);
    }
  if (error != NULL)
    {
      GtkWidget *failure = label_new (error, FALSE);

      gtk_label_set_line_wrap (GTK_LABEL (failure), TRUE);
      gtk_label_set_max_width_chars (GTK_LABEL (failure), 32);
      gtk_style_context_add_class (gtk_widget_get_style_context (failure),
                                   "caminito-power-error");
      gtk_box_pack_start (GTK_BOX (box), failure, FALSE, FALSE, 0);
    }
  gtk_widget_show_all (box);
  gtk_container_add (GTK_CONTAINER (menu), box);
  return button;
}

static GtkWidget *
icon_new (char **names)
{
  g_autoptr (GIcon) icon = g_themed_icon_new_from_names (names, -1);
  GtkWidget *image = gtk_image_new_from_gicon (icon, GTK_ICON_SIZE_LARGE_TOOLBAR);

  gtk_image_set_pixel_size (GTK_IMAGE (image), ROW_ICON_SIZE);
  return image;
}

/* One row of the table: icon, name with its state under it, bar, percentage, and for a
   laptop's battery the ⋮. */
static void
attach_row (PwPopup *popup, GtkWidget *table, int line, const PwRow *row)
{
  GtkWidget *names = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *name = label_new (row->name, FALSE);
  GtkWidget *state = label_new (row->state, TRUE);
  GtkWidget *bar = gtk_level_bar_new_for_interval (0, 1);
  GtkWidget *percentage = label_new (row->percentage, FALSE);

  gtk_label_set_ellipsize (GTK_LABEL (name), PANGO_ELLIPSIZE_END);
  gtk_label_set_ellipsize (GTK_LABEL (state), PANGO_ELLIPSIZE_END);
  gtk_box_pack_start (GTK_BOX (names), name, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (names), state, FALSE, FALSE, 0);
  gtk_widget_set_hexpand (names, TRUE);
  gtk_widget_set_valign (names, GTK_ALIGN_CENTER);

  /* GTK colours a bar by its own steps, 25% and 75%, which would make 73% and 80% look
     unlike one another: it has the one colour, and the warning one below 20%, like the
     percentage. */
  gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (bar), GTK_LEVEL_BAR_OFFSET_LOW);
  gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (bar), GTK_LEVEL_BAR_OFFSET_HIGH);
  gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (bar), GTK_LEVEL_BAR_OFFSET_FULL);
  gtk_level_bar_set_value (GTK_LEVEL_BAR (bar), row->fraction);
  if (row->low)
    gtk_style_context_add_class (gtk_widget_get_style_context (bar), "caminito-power-low");
  gtk_widget_set_size_request (bar, BAR_WIDTH, -1);
  gtk_widget_set_valign (bar, GTK_ALIGN_CENTER);

  gtk_label_set_xalign (GTK_LABEL (percentage), 1);
  /* Wide enough for "100%", so that the bars all end in the same place. */
  gtk_label_set_width_chars (GTK_LABEL (percentage), 4);
  if (row->low)
    gtk_style_context_add_class (gtk_widget_get_style_context (percentage), "caminito-power-low");

  gtk_grid_attach (GTK_GRID (table), icon_new (row->icon_names), 0, line, 1, 1);
  gtk_grid_attach (GTK_GRID (table), names, 1, line, 1, 1);
  gtk_grid_attach (GTK_GRID (table), bar, 2, line, 1, 1);
  gtk_grid_attach (GTK_GRID (table), percentage, 3, line, 1, 1);
  if (row->laptop)
    {
      GtkWidget *menu = menu_new (popup, row);

      gtk_grid_attach (GTK_GRID (table), menu, 4, line, 1, 1);
      if (g_strcmp0 (popup->open_path, row->path) == 0)
        popup->open_button = menu;
    }
}

static void
destroy_child (GtkWidget *child, gpointer data G_GNUC_UNUSED)
{
  gtk_widget_destroy (child);
}

static void
rebuild (PwPopup *popup)
{
  GtkWidget *table;

  /* The menu that was open goes with its row: it opens again on the new one. */
  popup->open_button = NULL;
  gtk_container_foreach (GTK_CONTAINER (popup->batteries), destroy_child, NULL);
  if (popup->rows->len == 0)
    {
      /* No battery and no peripheral with one: no table, and no line under it. */
      g_clear_pointer (&popup->open_path, g_free);
      return;
    }
  table = gtk_grid_new ();
  gtk_grid_set_column_spacing (GTK_GRID (table), 10);
  gtk_grid_set_row_spacing (GTK_GRID (table), 6);
  for (guint i = 0; i < popup->rows->len; i++)
    attach_row (popup, table, i, g_ptr_array_index (popup->rows, i));
  gtk_box_pack_start (GTK_BOX (popup->batteries), table, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (popup->batteries),
                      gtk_separator_new (GTK_ORIENTATION_HORIZONTAL), FALSE, FALSE, 2);
  gtk_widget_show_all (popup->batteries);
  if (popup->open_button != NULL)
    gtk_toggle_button_set_active (GTK_TOGGLE_BUTTON (popup->open_button), TRUE);
  else
    g_clear_pointer (&popup->open_path, g_free);
  if (gtk_widget_get_mapped (caminito_applet_popup_get_window (popup->popup)))
    fit_window (popup);
}

static gboolean
rows_equal (GPtrArray *a, GPtrArray *b)
{
  if (a->len != b->len)
    return FALSE;
  for (guint i = 0; i < a->len; i++)
    if (!pw_row_equal (g_ptr_array_index (a, i), g_ptr_array_index (b, i)))
      return FALSE;
  return TRUE;
}

static const PwRow *
row_for (PwPopup *popup, const char *path)
{
  for (guint i = 0; i < popup->rows->len; i++)
    {
      const PwRow *row = g_ptr_array_index (popup->rows, i);

      if (g_str_equal (row->path, path))
        return row;
    }
  return NULL;
}

static void
forget_missing (GHashTable *table, PwPopup *popup)
{
  GHashTableIter iter;
  gpointer path;

  g_hash_table_iter_init (&iter, table);
  while (g_hash_table_iter_next (&iter, &path, NULL))
    if (row_for (popup, path) == NULL)
      g_hash_table_iter_remove (&iter);
}

void
pw_popup_update (PwPopup *popup, GPtrArray *devices)
{
  GPtrArray *rows = pw_state_rows (devices);
  GHashTableIter iter;
  gpointer path, want;

  /* A change of what isn't shown, the time left or the energy rate, builds nothing: the
     table would close an open menu a moment after it opened. */
  if (rows_equal (rows, popup->rows))
    {
      g_ptr_array_unref (rows);
      return;
    }
  g_ptr_array_unref (popup->rows);
  popup->rows = rows;
  /* UPower has caught up with a switch where its row says what the user asked for, and
     has no more to say about a battery that went. */
  g_hash_table_iter_init (&iter, popup->requested);
  while (g_hash_table_iter_next (&iter, &path, &want))
    {
      const PwRow *row = row_for (popup, path);

      if (row == NULL || row->limit_enabled == (GPOINTER_TO_INT (want) == REQUESTED_ON))
        g_hash_table_iter_remove (&iter);
    }
  forget_missing (popup->busy, popup);
  forget_missing (popup->errors, popup);
  rebuild (popup);
}

void
pw_popup_limit_done (PwPopup *popup, const char *path, const char *error)
{
  g_hash_table_remove (popup->busy, path);
  if (error != NULL)
    {
      g_hash_table_remove (popup->requested, path);
      g_hash_table_insert (popup->errors, g_strdup (path), g_strdup (error));
      /* The menu is open to show it, which it would otherwise hide. */
      g_free (popup->open_path);
      popup->open_path = g_strdup (path);
    }
  /* Drawn again: the switch goes back to what UPower has after a refusal, and after an
     answer stays where the user put it. */
  rebuild (popup);
}

PwPopup *
pw_popup_new (CaminitoAppletPopup *applet_popup, const PwPopupActions *actions, gpointer data)
{
  PwPopup *popup = g_new0 (PwPopup, 1);
  GtkWidget *window = caminito_applet_popup_get_window (applet_popup);
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 4);
  GtkWidget *configure = gtk_button_new_with_label ("Configure Power Management…");

  add_styles ();
  popup->popup = applet_popup;
  popup->actions = *actions;
  popup->data = data;
  popup->rows = g_ptr_array_new_with_free_func ((GDestroyNotify) pw_row_free);
  popup->busy = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  popup->requested = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  popup->errors = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, g_free);

  gtk_container_set_border_width (GTK_CONTAINER (box), 8);
  gtk_widget_set_size_request (box, POPUP_WIDTH, -1);
  gtk_button_set_relief (GTK_BUTTON (configure), GTK_RELIEF_NONE);
  gtk_widget_set_halign (configure, GTK_ALIGN_START);
  g_signal_connect (configure, "clicked", G_CALLBACK (on_configure), popup);
  /* The power profile (CMN-98) and Stay awake (CMN-101) go above the table. */
  popup->batteries = gtk_box_new (GTK_ORIENTATION_VERTICAL, 4);
  gtk_box_pack_start (GTK_BOX (box), popup->batteries, FALSE, FALSE, 0);
  gtk_box_pack_end (GTK_BOX (box), configure, FALSE, FALSE, 0);
  gtk_widget_show_all (box);
  gtk_container_add (GTK_CONTAINER (window), box);
  popup->content = box;
  g_object_add_weak_pointer (G_OBJECT (box), (gpointer *) &popup->content);
  return popup;
}

void
pw_popup_free (PwPopup *popup)
{
  if (popup->content != NULL)
    {
      g_object_remove_weak_pointer (G_OBJECT (popup->content), (gpointer *) &popup->content);
      gtk_widget_destroy (popup->content);
    }
  g_ptr_array_unref (popup->rows);
  g_hash_table_unref (popup->busy);
  g_hash_table_unref (popup->requested);
  g_hash_table_unref (popup->errors);
  g_free (popup->open_path);
  g_free (popup);
}
