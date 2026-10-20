/*
 * The Thermal applet's popup (268): the prototype's Popup C with Popup A's bars
 * (prototype/thermal-applet-ui, --variant 3), in GTK3. See th-popup.h.
 */

#include <math.h>

#include "th-popup.h"

#define POPUP_WIDTH 340
#define BAR_WIDTH   110

static const char *const fan_icon[] = {
  "xsi-fan-symbolic", "sensors-fan-symbolic", "freon-fan-symbolic",
  "emblem-synchronizing-symbolic", NULL,
};

static const char *const pin_icon[] = {
  "view-pin-symbolic", "pin-symbolic", "emblem-important-symbolic", NULL,
};

/* The groups in the order Show all sensors lists them, with their names and icons. */
static const struct
{
  ThGroup            group;
  const char        *name;
  const char *const  icons[3];
} groups[] = {
  { TH_GROUP_CPU, "CPU", { "cpu-symbolic", "computer-symbolic", NULL } },
  { TH_GROUP_GPU, "GPU", { "video-display-symbolic", "computer-symbolic", NULL } },
  { TH_GROUP_STORAGE, "Storage",
    { "drive-harddisk-solidstate-symbolic", "drive-harddisk-symbolic", NULL } },
  { TH_GROUP_WIRELESS, "Wireless", { "network-wireless-symbolic", NULL } },
  { TH_GROUP_BOARD, "Board", { "applications-system-symbolic", "emblem-system-symbolic", NULL } },
  { TH_GROUP_OTHER, "Other", { "dialog-information-symbolic", NULL } },
};

G_STATIC_ASSERT (G_N_ELEMENTS (groups) == TH_GROUP_OTHER + 1);

/* The groups that have a tile; no other fills in. */
static const ThGroup tiled[] = { TH_GROUP_CPU, TH_GROUP_GPU, TH_GROUP_STORAGE };

static const char *const band_classes[] = {
  [TH_BAND_NORMAL] = "caminito-thermal-normal",
  [TH_BAND_WARM] = "caminito-thermal-warm",
  [TH_BAND_HOT] = "caminito-thermal-hot",
};

/* A tile's widgets that change: its reading and that reading's name. */
typedef struct
{
  gboolean   fans;
  /* A temperature tile's; a fan tile has none. */
  ThGroup    group;
  GtkWidget *value;
  GtkWidget *sub;
} Tile;

/* A row's of Show all sensors. A fan's has no pin. */
typedef struct
{
  GtkWidget *name;
  GtkWidget *bar;
  GtkWidget *value;
  GtkWidget *pin;
} Row;

struct _ThPopup
{
  CaminitoAppletPopup *applet_popup;
  GtkWidget           *grid;
  GtkWidget           *expander;
  GPtrArray           *tiles;
  GPtrArray           *temperature_rows;
  GPtrArray           *fan_rows;
  /* Every reading's id in the order shown: the widgets are made again when it changes. */
  char                *shape;
};

static guint
group_index (ThGroup group)
{
  for (guint g = 0; g < G_N_ELEMENTS (groups); g++)
    if (groups[g].group == group)
      return g;
  g_assert_not_reached ();
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
  /* Warm and hot as the prototype has them on a light popup. A normal reading is the
     theme's text colour, its bar too. The pin shows on hover, and stays on a panel
     sensor's row. */
  gtk_css_provider_load_from_data (
    provider,
    ".caminito-thermal-tile { padding: 10px; border-radius: 10px;"
    " background-color: rgba(127, 127, 127, 0.12); }"
    ".caminito-thermal-big { font-weight: bold; font-size: 20px; }"
    ".caminito-thermal-title { font-weight: bold; }"
    "label.caminito-thermal-warm { color: #c88400; }"
    "label.caminito-thermal-hot { color: #d42a2a; }"
    "levelbar.caminito-thermal-bar trough { min-height: 6px; padding: 0; }"
    "levelbar.caminito-thermal-bar block { min-height: 6px; }"
    "levelbar.caminito-thermal-bar block.filled { background-image: none; }"
    "levelbar.caminito-thermal-normal block.filled {"
    " background-color: @theme_fg_color; border-color: @theme_fg_color; }"
    "levelbar.caminito-thermal-warm block.filled {"
    " background-color: #e6a100; border-color: #e6a100; }"
    "levelbar.caminito-thermal-hot block.filled {"
    " background-color: #e03b3b; border-color: #e03b3b; }"
    "levelbar.caminito-thermal-fan block.filled {"
    " background-color: #3584e4; border-color: #3584e4; }"
    "list.caminito-thermal-list, list.caminito-thermal-list row {"
    " background-color: transparent; }"
    "list.caminito-thermal-list row { padding: 1px 0; }"
    ".caminito-thermal-pin { opacity: 0; padding: 0 2px; min-height: 0; min-width: 0; }"
    "row:hover .caminito-thermal-pin { opacity: 0.6; }"
    ".caminito-thermal-pin.caminito-thermal-pinned,"
    " row:hover .caminito-thermal-pin.caminito-thermal-pinned { opacity: 1; }",
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

static void
set_band (GtkWidget *widget, ThBand band)
{
  GtkStyleContext *style = gtk_widget_get_style_context (widget);

  for (guint i = 0; i < G_N_ELEMENTS (band_classes); i++)
    gtk_style_context_remove_class (style, band_classes[i]);
  gtk_style_context_add_class (style, band_classes[band]);
}

static GtkWidget *
icon_new (const char *const *names)
{
  g_autoptr (GIcon) icon = g_themed_icon_new_from_names ((char **) names, -1);

  return gtk_image_new_from_gicon (icon, GTK_ICON_SIZE_MENU);
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

/* An icon and a name in bold, as a tile's or a group's heading. */
static GtkWidget *
heading_new (const char *const *icons, const char *name)
{
  GtkWidget *heading = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 6);

  gtk_box_pack_start (GTK_BOX (heading), icon_new (icons), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (heading), label_new (name, "caminito-thermal-title"), FALSE,
                      FALSE, 0);
  return heading;
}

static GtkWidget *
bar_new (void)
{
  GtkWidget *bar = gtk_level_bar_new_for_interval (0, 1);

  gtk_level_bar_set_mode (GTK_LEVEL_BAR (bar), GTK_LEVEL_BAR_MODE_CONTINUOUS);
  /* The theme's low, high and full colours would cover the band's. */
  gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (bar), GTK_LEVEL_BAR_OFFSET_LOW);
  gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (bar), GTK_LEVEL_BAR_OFFSET_HIGH);
  gtk_level_bar_remove_offset_value (GTK_LEVEL_BAR (bar), GTK_LEVEL_BAR_OFFSET_FULL);
  gtk_widget_set_valign (bar, GTK_ALIGN_CENTER);
  gtk_widget_set_size_request (bar, BAR_WIDTH, -1);
  add_class (bar, "caminito-thermal-bar");
  return bar;
}

static GtkWidget *
value_new (void)
{
  GtkWidget *value = gtk_label_new (NULL);

  gtk_label_set_xalign (GTK_LABEL (value), 1);
  gtk_label_set_width_chars (GTK_LABEL (value), 8);
  return value;
}

/* The temperatures in the order shown: by group, and in the source's order in each. */
static GPtrArray *
ordered_temperatures (ThModel *model)
{
  GPtrArray *temperatures = th_model_temperatures (model);
  GPtrArray *ordered = g_ptr_array_new ();

  for (guint g = 0; g < G_N_ELEMENTS (groups); g++)
    for (guint i = 0; i < temperatures->len; i++)
      if (((ThReading *) temperatures->pdata[i])->group == groups[g].group)
        g_ptr_array_add (ordered, temperatures->pdata[i]);
  return ordered;
}

static char *
shape_of (GPtrArray *temperatures, GPtrArray *fans)
{
  GString *shape = g_string_new (NULL);

  for (guint i = 0; i < temperatures->len; i++)
    g_string_append_printf (shape, "%s;", ((ThReading *) temperatures->pdata[i])->id);
  g_string_append (shape, "|");
  for (guint i = 0; i < fans->len; i++)
    g_string_append_printf (shape, "%s;", ((ThReading *) fans->pdata[i])->id);
  return g_string_free (shape, FALSE);
}

static Tile *
tile_add (ThPopup *popup, GtkWidget *heading, gboolean fans, ThGroup group)
{
  Tile *tile = g_new0 (Tile, 1);
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 2);
  guint n = popup->tiles->len;

  tile->fans = fans;
  tile->group = group;
  tile->value = label_new (NULL, "caminito-thermal-big");
  tile->sub = label_new (NULL, GTK_STYLE_CLASS_DIM_LABEL);
  gtk_box_pack_start (GTK_BOX (box), heading, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), tile->value, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), tile->sub, FALSE, FALSE, 0);
  add_class (box, "caminito-thermal-tile");
  gtk_grid_attach (GTK_GRID (popup->grid), box, n % 2, n / 2, 1, 1);
  g_ptr_array_add (popup->tiles, tile);
  return tile;
}

static GtkWidget *
list_new (void)
{
  GtkWidget *list = gtk_list_box_new ();

  gtk_list_box_set_selection_mode (GTK_LIST_BOX (list), GTK_SELECTION_NONE);
  gtk_list_box_set_activate_on_single_click (GTK_LIST_BOX (list), FALSE);
  add_class (list, "caminito-thermal-list");
  return list;
}

static Row *
row_add (GtkWidget *list, gboolean fan)
{
  Row *row = g_new0 (Row, 1);
  GtkWidget *line = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *list_row = gtk_list_box_row_new ();

  if (fan)
    gtk_box_pack_start (GTK_BOX (line), icon_new (fan_icon), FALSE, FALSE, 0);
  row->name = label_new (NULL, NULL);
  row->bar = bar_new ();
  row->value = value_new ();
  gtk_box_pack_start (GTK_BOX (line), row->name, TRUE, TRUE, 0);
  gtk_box_pack_start (GTK_BOX (line), row->bar, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (line), row->value, FALSE, FALSE, 0);
  if (fan)
    {
      /* Room where a temperature's pin is, so that the columns line up. */
      GtkWidget *spacer = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 0);

      gtk_widget_set_size_request (spacer, 24, -1);
      gtk_box_pack_start (GTK_BOX (line), spacer, FALSE, FALSE, 0);
      add_class (row->bar, "caminito-thermal-fan");
    }
  else
    {
      /* It does nothing until 269. */
      row->pin = gtk_button_new ();
      gtk_button_set_relief (GTK_BUTTON (row->pin), GTK_RELIEF_NONE);
      gtk_container_add (GTK_CONTAINER (row->pin), icon_new (pin_icon));
      add_class (row->pin, "caminito-thermal-pin");
      gtk_box_pack_start (GTK_BOX (line), row->pin, FALSE, FALSE, 0);
    }
  gtk_list_box_row_set_activatable (GTK_LIST_BOX_ROW (list_row), FALSE);
  gtk_container_add (GTK_CONTAINER (list_row), line);
  gtk_container_add (GTK_CONTAINER (list), list_row);
  return row;
}

/* A group's heading over its rows. */
static GtkWidget *
section_add (GtkWidget *all, const char *const *icons, const char *name)
{
  GtkWidget *section = gtk_box_new (GTK_ORIENTATION_VERTICAL, 4);
  GtkWidget *list = list_new ();

  gtk_box_pack_start (GTK_BOX (section), heading_new (icons, name), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (section), list, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (all), section, FALSE, FALSE, 0);
  return list;
}

static void
destroy (GtkWidget *widget, gpointer data G_GNUC_UNUSED)
{
  gtk_widget_destroy (widget);
}

/* The tiles and Show all's rows for these readings, in place of the old ones. */
static void
build (ThPopup *popup, ThModel *model, GPtrArray *temperatures, GPtrArray *fans)
{
  GtkWidget *all = gtk_bin_get_child (GTK_BIN (popup->expander));
  GtkWidget *list = NULL;
  ThGroup group = TH_GROUP_OTHER;

  gtk_container_foreach (GTK_CONTAINER (popup->grid), destroy, NULL);
  if (all != NULL)
    gtk_widget_destroy (all);
  g_ptr_array_set_size (popup->tiles, 0);
  g_ptr_array_set_size (popup->temperature_rows, 0);
  g_ptr_array_set_size (popup->fan_rows, 0);

  for (guint t = 0; t < G_N_ELEMENTS (tiled); t++)
    {
      guint count;

      if (th_model_hottest (model, tiled[t], &count) == NULL)
        continue;
      guint g = group_index (tiled[t]);

      tile_add (popup, heading_new (groups[g].icons, groups[g].name), FALSE, tiled[t]);
    }
  if (fans->len > 0)
    tile_add (popup, heading_new (fan_icon, fans->len == 1 ? "Fan" : "Fans"), TRUE, 0);

  all = gtk_box_new (GTK_ORIENTATION_VERTICAL, 10);
  gtk_widget_set_margin_top (all, 8);
  for (guint i = 0; i < temperatures->len; i++)
    {
      ThReading *reading = temperatures->pdata[i];

      if (list == NULL || reading->group != group)
        {
          guint g = group_index (reading->group);

          list = section_add (all, groups[g].icons, groups[g].name);
        }
      group = reading->group;
      g_ptr_array_add (popup->temperature_rows, row_add (list, FALSE));
    }
  if (fans->len > 0)
    {
      list = section_add (all, fan_icon, "Fans");
      for (guint i = 0; i < fans->len; i++)
        g_ptr_array_add (popup->fan_rows, row_add (list, TRUE));
    }
  gtk_container_add (GTK_CONTAINER (popup->expander), all);
  gtk_widget_show_all (popup->grid);
  gtk_widget_show_all (all);
}

static void
update_tile (Tile *tile, ThModel *model)
{
  g_autofree char *text = NULL;
  ThReading *reading;
  guint count;

  if (tile->fans)
    {
      reading = th_model_fastest_fan (model);
      count = th_model_fans (model)->len;
      text = th_fan_text (reading->value);
    }
  else
    {
      reading = th_model_hottest (model, tile->group, &count);
      text = th_temperature_text (reading->value);
      set_band (tile->value, th_reading_band (reading));
    }
  gtk_label_set_text (GTK_LABEL (tile->value), text);
  gtk_label_set_text (GTK_LABEL (tile->sub), count > 1 ? reading->name : "");
  gtk_widget_set_visible (tile->sub, count > 1);
}

static void
update_temperature_row (Row *row, ThModel *model, const ThReading *reading)
{
  g_autofree char *text = th_temperature_text (reading->value);
  ThBand band = th_reading_band (reading);
  GtkStyleContext *pin = gtk_widget_get_style_context (row->pin);

  gtk_label_set_text (GTK_LABEL (row->name), reading->name);
  /* A drive's model is often longer than the room. */
  gtk_widget_set_tooltip_text (row->name, reading->name);
  gtk_label_set_text (GTK_LABEL (row->value), text);
  gtk_level_bar_set_value (GTK_LEVEL_BAR (row->bar), th_temperature_fraction (reading->value));
  set_band (row->bar, band);
  set_band (row->value, band);
  if (th_model_is_panel_sensor (model, reading))
    gtk_style_context_add_class (pin, "caminito-thermal-pinned");
  else
    gtk_style_context_remove_class (pin, "caminito-thermal-pinned");
}

static void
update_fan_row (Row *row, ThModel *model, const ThReading *fan)
{
  g_autofree char *text = th_fan_text (fan->value);
  double scale = th_model_fan_scale (model, fan);

  gtk_label_set_text (GTK_LABEL (row->name), fan->name);
  gtk_widget_set_tooltip_text (row->name, fan->name);
  gtk_label_set_text (GTK_LABEL (row->value), text);
  /* Hidden, not removed, so that the row doesn't move when it comes. */
  gtk_widget_set_opacity (row->bar, isnan (scale) ? 0 : 1);
  gtk_level_bar_set_value (GTK_LEVEL_BAR (row->bar),
                           isnan (scale) ? 0 : CLAMP (fan->value / scale, 0.0, 1.0));
}

/* A window doesn't shrink when its content does: asking for the least brings it back to
   what the content needs, at the width it has. */
static void
fit_window (ThPopup *popup)
{
  gtk_window_resize (GTK_WINDOW (caminito_applet_popup_get_window (popup->applet_popup)),
                     POPUP_WIDTH, 1);
}

static void
on_expanded (GObject *expander G_GNUC_UNUSED, GParamSpec *pspec G_GNUC_UNUSED, gpointer data)
{
  fit_window (data);
}

ThPopup *
th_popup_new (CaminitoAppletPopup *applet_popup)
{
  ThPopup *popup = g_new0 (ThPopup, 1);
  GtkWidget *content = gtk_box_new (GTK_ORIENTATION_VERTICAL, 10);

  add_styles ();
  popup->applet_popup = applet_popup;
  popup->tiles = g_ptr_array_new_with_free_func (g_free);
  popup->temperature_rows = g_ptr_array_new_with_free_func (g_free);
  popup->fan_rows = g_ptr_array_new_with_free_func (g_free);
  popup->grid = gtk_grid_new ();
  gtk_grid_set_column_spacing (GTK_GRID (popup->grid), 8);
  gtk_grid_set_row_spacing (GTK_GRID (popup->grid), 8);
  gtk_grid_set_column_homogeneous (GTK_GRID (popup->grid), TRUE);
  popup->expander = gtk_expander_new ("Show all sensors");
  g_signal_connect (popup->expander, "notify::expanded", G_CALLBACK (on_expanded), popup);
  gtk_box_pack_start (GTK_BOX (content), popup->grid, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (content), popup->expander, FALSE, FALSE, 0);
  gtk_widget_set_size_request (content, POPUP_WIDTH, -1);
  g_object_set (content, "margin", 12, NULL);
  gtk_container_add (GTK_CONTAINER (caminito_applet_popup_get_window (applet_popup)), content);
  gtk_widget_show_all (content);
  return popup;
}

void
th_popup_free (ThPopup *popup)
{
  /* The window, and the expander in it, can outlive the popup's state. */
  g_signal_handlers_disconnect_by_data (popup->expander, popup);
  g_ptr_array_unref (popup->tiles);
  g_ptr_array_unref (popup->temperature_rows);
  g_ptr_array_unref (popup->fan_rows);
  g_free (popup->shape);
  g_free (popup);
}

void
th_popup_update (ThPopup *popup, ThModel *model)
{
  g_autoptr (GPtrArray) temperatures = ordered_temperatures (model);
  GPtrArray *fans = th_model_fans (model);
  g_autofree char *shape = shape_of (temperatures, fans);

  if (g_strcmp0 (shape, popup->shape) != 0)
    {
      build (popup, model, temperatures, fans);
      g_free (popup->shape);
      popup->shape = g_steal_pointer (&shape);
      fit_window (popup);
    }
  for (guint i = 0; i < popup->tiles->len; i++)
    update_tile (popup->tiles->pdata[i], model);
  for (guint i = 0; i < temperatures->len; i++)
    update_temperature_row (popup->temperature_rows->pdata[i], model, temperatures->pdata[i]);
  for (guint i = 0; i < fans->len; i++)
    update_fan_row (popup->fan_rows->pdata[i], model, fans->pdata[i]);
}
