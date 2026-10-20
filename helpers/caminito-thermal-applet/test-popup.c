/*
 * The Thermal popup's tiles and Show all sensors (268), from computers built by hand: what
 * they show, their bands, the fans' bars, and that a read changes them in place. Without a
 * display, GTK can't start, and it's skipped.
 */

#include <string.h>

#include "test-computers.h"
#include "th-popup.h"

typedef struct
{
  GtkWidget           *anchor;
  CaminitoAppletPopup *popup;
  ThPopup             *content;
  ThModel             *model;
} Fixture;

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  fixture->anchor = g_object_ref_sink (gtk_button_new ());
  fixture->popup = caminito_applet_popup_new (fixture->anchor);
  fixture->content = th_popup_new (fixture->popup);
  fixture->model = th_model_new ();
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  th_popup_free (fixture->content);
  th_model_free (fixture->model);
  /* The popup goes with its anchor. */
  g_object_unref (fixture->anchor);
}

/* Reads raws, which it frees, and shows them. */
static void
read_in (Fixture *fixture, GPtrArray *raws)
{
  th_model_update (fixture->model, raws);
  th_popup_update (fixture->content, fixture->model);
  g_ptr_array_unref (raws);
}

typedef struct
{
  GType      type;
  GPtrArray *found;
} Search;

static void
search (GtkWidget *widget, gpointer data)
{
  Search *s = data;

  if (G_TYPE_CHECK_INSTANCE_TYPE (widget, s->type))
    g_ptr_array_add (s->found, widget);
  if (GTK_IS_CONTAINER (widget))
    gtk_container_forall (GTK_CONTAINER (widget), search, data);
}

static GPtrArray *
all_of (GtkWidget *widget, GType type)
{
  Search s = { type, g_ptr_array_new () };

  search (widget, &s);
  return s.found;
}

static GtkWidget *
content (Fixture *fixture)
{
  return gtk_bin_get_child (GTK_BIN (caminito_applet_popup_get_window (fixture->popup)));
}

static GtkWidget *
tiles (Fixture *fixture)
{
  g_autoptr (GPtrArray) grids = all_of (content (fixture), GTK_TYPE_GRID);

  return grids->pdata[0];
}

static GtkWidget *
show_all (Fixture *fixture)
{
  g_autoptr (GPtrArray) expanders = all_of (content (fixture), GTK_TYPE_EXPANDER);

  return expanders->pdata[0];
}

/* The visible labels' texts under widget, in order, joined with "|". */
static char *
texts (GtkWidget *widget)
{
  g_autoptr (GPtrArray) labels = all_of (widget, GTK_TYPE_LABEL);
  GString *joined = g_string_new (NULL);

  for (guint i = 0; i < labels->len; i++)
    {
      GtkWidget *label = labels->pdata[i];

      if (!gtk_widget_get_visible (label))
        continue;
      g_string_append_printf (joined, "%s%s", joined->len > 0 ? "|" : "",
                              gtk_label_get_text (GTK_LABEL (label)));
    }
  return g_string_free (joined, FALSE);
}

/* The tiles in reading order, two to a row. */
static GPtrArray *
tiles_in_order (Fixture *fixture)
{
  GPtrArray *found = g_ptr_array_new ();
  GtkWidget *tile;

  for (int i = 0; (tile = gtk_grid_get_child_at (GTK_GRID (tiles (fixture)), i % 2, i / 2)) != NULL;
       i++)
    g_ptr_array_add (found, tile);
  return found;
}

static char *
tiles_texts (Fixture *fixture)
{
  g_autoptr (GPtrArray) found = tiles_in_order (fixture);
  g_autoptr (GStrvBuilder) builder = g_strv_builder_new ();
  g_auto (GStrv) parts = NULL;

  for (guint i = 0; i < found->len; i++)
    g_strv_builder_take (builder, texts (found->pdata[i]));
  parts = g_strv_builder_end (builder);
  return g_strjoinv ("|", parts);
}

/* The tile's value: its second label. */
static GtkWidget *
tile_value (Fixture *fixture, guint i)
{
  g_autoptr (GPtrArray) found = tiles_in_order (fixture);
  g_autoptr (GPtrArray) labels = all_of (found->pdata[i], GTK_TYPE_LABEL);

  return labels->pdata[1];
}

/* Show all's rows' texts, without the expander's own label. */
static char *
rows_texts (Fixture *fixture)
{
  return texts (gtk_bin_get_child (GTK_BIN (show_all (fixture))));
}

/* The row's box with the label that reads name. */
static GtkWidget *
row_of (Fixture *fixture, const char *name)
{
  g_autoptr (GPtrArray) labels = all_of (gtk_bin_get_child (GTK_BIN (show_all (fixture))),
                                         GTK_TYPE_LABEL);

  for (guint i = 0; i < labels->len; i++)
    if (g_str_equal (gtk_label_get_text (labels->pdata[i]), name)
        && gtk_widget_get_ancestor (labels->pdata[i], GTK_TYPE_LIST_BOX_ROW) != NULL)
      return gtk_widget_get_parent (labels->pdata[i]);
  return NULL;
}

static GtkWidget *
child_of_type (GtkWidget *widget, GType type)
{
  g_autoptr (GPtrArray) found = all_of (widget, type);

  return found->len > 0 ? found->pdata[0] : NULL;
}

/* The row's value: its last label. */
static GtkWidget *
value_of (GtkWidget *row)
{
  g_autoptr (GPtrArray) labels = all_of (row, GTK_TYPE_LABEL);

  return labels->pdata[labels->len - 1];
}

static gboolean
has_class (GtkWidget *widget, const char *name)
{
  return gtk_style_context_has_class (gtk_widget_get_style_context (widget), name);
}

static void
test_desktop_has_four_tiles (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *all = NULL;

  read_in (fixture, desktop (58, 61, 1180, 820, 0));
  all = tiles_texts (fixture);
  /* Each group's hottest and its name, the fastest fan and its; Wireless and Board have
     none. */
  g_assert_cmpstr (all, ==,
                   "CPU|58 °C|CPU"
                   "|GPU|61 °C|GPU junction"
                   "|Storage|46 °C|WD_BLACK SN850X 1000GB"
                   "|Fans|1180 RPM|Fan 2");
}

static void
test_thinkpad_has_no_gpu_tile (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *all = NULL;

  read_in (fixture, thinkpad (55, 2865));
  all = tiles_texts (fixture);
  /* One drive's reading has no name under it, and one fan is "Fan". */
  g_assert_cmpstr (all, ==,
                   "CPU|55 °C|CPU Package"
                   "|Storage|38 °C"
                   "|Fan|2865 RPM");
}

static void
test_show_all_is_grouped_with_fans_last (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *all = NULL;

  read_in (fixture, thinkpad (55, 0));
  all = rows_texts (fixture);
  g_assert_cmpstr (all, ==,
                   "CPU|CPU Package|55 °C|Core 0|53 °C|Core 1|52 °C"
                   "|Storage|SAMSUNG MZVL2512HCJQ|38 °C"
                   "|Wireless|Wi-Fi|41 °C"
                   "|Board|EC: CPU|52 °C|ACPI zone|50 °C"
                   "|Fans|Fan|Stopped");
  g_assert_false (gtk_expander_get_expanded (GTK_EXPANDER (show_all (fixture))));
}

static void
test_bar_is_on_one_scale (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  GtkWidget *bar;

  read_in (fixture, thinkpad (60, 2865));
  bar = child_of_type (row_of (fixture, "CPU Package"), GTK_TYPE_LEVEL_BAR);
  g_assert_cmpfloat (gtk_level_bar_get_value (GTK_LEVEL_BAR (bar)), ==, 0.5);
  bar = child_of_type (row_of (fixture, "SAMSUNG MZVL2512HCJQ"), GTK_TYPE_LEVEL_BAR);
  g_assert_cmpfloat (gtk_level_bar_get_value (GTK_LEVEL_BAR (bar)), ==, 18.0 / 80);
}

static void
test_hot_is_red (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  GtkWidget *junction;
  GtkWidget *edge;

  read_in (fixture, desktop (80, 105, 1180, 820, 0));
  junction = row_of (fixture, "GPU junction");
  edge = row_of (fixture, "GPU edge");
  /* The junction is 105 °C against its crit 110: hot. The edge, at 93 °C against its 100,
     too; the CPU at 80 °C is warm. */
  g_assert_true (has_class (value_of (junction), "caminito-thermal-hot"));
  g_assert_true (has_class (child_of_type (junction, GTK_TYPE_LEVEL_BAR),
                            "caminito-thermal-hot"));
  g_assert_true (has_class (value_of (row_of (fixture, "CPU")), "caminito-thermal-warm"));
  g_assert_true (has_class (value_of (row_of (fixture, "SYSTIN")), "caminito-thermal-normal"));
  g_assert_true (has_class (child_of_type (edge, GTK_TYPE_LEVEL_BAR), "caminito-thermal-hot"));
  /* The GPU's tile is its hottest, the junction, in red. */
  g_assert_cmpstr (gtk_label_get_text (GTK_LABEL (tile_value (fixture, 1))), ==, "105 °C");
  g_assert_true (has_class (tile_value (fixture, 1), "caminito-thermal-hot"));
  g_assert_true (has_class (tile_value (fixture, 0), "caminito-thermal-warm"));

  /* And back to normal as it cools. */
  read_in (fixture, desktop (58, 61, 1180, 820, 0));
  g_assert_true (has_class (value_of (junction), "caminito-thermal-normal"));
  g_assert_false (has_class (value_of (junction), "caminito-thermal-hot"));
}

static void
test_fan_bars (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  GtkWidget *gpu_fan;
  GtkWidget *chassis;

  /* The GPU fan has a max of 3300 RPM, the others none. */
  for (int i = 1; i <= 10; i++)
    read_in (fixture, desktop (58, 61, 1180, 820, 1650));
  gpu_fan = row_of (fixture, "GPU fan");
  chassis = row_of (fixture, "Fan 1");
  g_assert_cmpfloat (gtk_widget_get_opacity (child_of_type (gpu_fan, GTK_TYPE_LEVEL_BAR)), ==, 1);
  g_assert_cmpfloat (gtk_level_bar_get_value (GTK_LEVEL_BAR (child_of_type (gpu_fan, GTK_TYPE_LEVEL_BAR))),
                     ==, 0.5);
  g_assert_cmpfloat (gtk_widget_get_opacity (child_of_type (chassis, GTK_TYPE_LEVEL_BAR)), ==, 0);

  /* The 11th read: against the fastest it has spun. */
  read_in (fixture, desktop (58, 61, 1180, 410, 0));
  g_assert_cmpfloat (gtk_widget_get_opacity (child_of_type (chassis, GTK_TYPE_LEVEL_BAR)), ==, 1);
  g_assert_cmpfloat (gtk_level_bar_get_value (GTK_LEVEL_BAR (child_of_type (chassis, GTK_TYPE_LEVEL_BAR))),
                     ==, 0.5);
  g_assert_cmpstr (gtk_label_get_text (GTK_LABEL (value_of (gpu_fan))), ==, "Stopped");
  g_assert_cmpfloat (gtk_widget_get_opacity (child_of_type (gpu_fan, GTK_TYPE_LEVEL_BAR)), ==, 1);
}

static void
test_pins_show_the_panel_sensors (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  read_in (fixture, desktop (58, 61, 1180, 820, 0));
  g_assert_true (has_class (child_of_type (row_of (fixture, "CPU"), GTK_TYPE_BUTTON),
                            "caminito-thermal-pinned"));
  g_assert_true (has_class (child_of_type (row_of (fixture, "GPU junction"), GTK_TYPE_BUTTON),
                            "caminito-thermal-pinned"));
  g_assert_false (has_class (child_of_type (row_of (fixture, "GPU edge"), GTK_TYPE_BUTTON),
                             "caminito-thermal-pinned"));
  /* A fan has none. */
  g_assert_null (child_of_type (row_of (fixture, "Fan 1"), GTK_TYPE_BUTTON));
}

static void
test_a_read_changes_the_widgets_in_place (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  GtkWidget *row;
  GtkWidget *grid_child;

  read_in (fixture, thinkpad (55, 2865));
  gtk_expander_set_expanded (GTK_EXPANDER (show_all (fixture)), TRUE);
  row = row_of (fixture, "CPU Package");
  grid_child = gtk_grid_get_child_at (GTK_GRID (tiles (fixture)), 0, 0);

  read_in (fixture, thinkpad (57, 3010));
  g_assert_true (row_of (fixture, "CPU Package") == row);
  g_assert_true (gtk_grid_get_child_at (GTK_GRID (tiles (fixture)), 0, 0) == grid_child);
  g_assert_cmpstr (gtk_label_get_text (GTK_LABEL (value_of (row))), ==, "57 °C");
  g_assert_true (gtk_expander_get_expanded (GTK_EXPANDER (show_all (fixture))));
}

static void
test_a_chip_that_goes_takes_its_tile (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) raws = desktop (58, 61, 1180, 820, 0);
  GPtrArray *without_gpu = raws_new ();
  g_autofree char *all = NULL;
  g_autofree char *rows = NULL;

  read_in (fixture, g_ptr_array_ref (raws));
  gtk_expander_set_expanded (GTK_EXPANDER (show_all (fixture)), TRUE);
  for (guint i = 0; i < raws->len; i++)
    {
      ThRaw *raw = raws->pdata[i];

      if (!g_str_equal (raw->chip, "amdgpu"))
        g_ptr_array_add (without_gpu,
                         th_raw_new (raw->chip, raw->chip_id, raw->feature, raw->label,
                                     raw->device, raw->is_fan, raw->ok, raw->value,
                                     raw->limit));
    }
  read_in (fixture, without_gpu);
  all = tiles_texts (fixture);
  g_assert_cmpstr (all, ==,
                   "CPU|58 °C|CPU"
                   "|Storage|46 °C|WD_BLACK SN850X 1000GB"
                   "|Fans|1180 RPM|Fan 2");
  rows = rows_texts (fixture);
  g_assert_null (strstr (rows, "GPU"));
  g_assert_true (gtk_expander_get_expanded (GTK_EXPANDER (show_all (fixture))));
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
  g_test_add ("/popup/the-desktop-has-four-tiles", Fixture, NULL, setup, test_desktop_has_four_tiles, teardown);
  g_test_add ("/popup/the-thinkpad-has-no-gpu-tile", Fixture, NULL, setup, test_thinkpad_has_no_gpu_tile, teardown);
  g_test_add ("/popup/show-all-is-grouped-with-fans-last", Fixture, NULL, setup, test_show_all_is_grouped_with_fans_last, teardown);
  g_test_add ("/popup/the-bar-is-on-one-scale", Fixture, NULL, setup, test_bar_is_on_one_scale, teardown);
  g_test_add ("/popup/hot-is-red", Fixture, NULL, setup, test_hot_is_red, teardown);
  g_test_add ("/popup/a-fan-without-a-max-gets-its-bar-after-10-reads", Fixture, NULL, setup, test_fan_bars, teardown);
  g_test_add ("/popup/the-pins-show-the-panel-sensors", Fixture, NULL, setup, test_pins_show_the_panel_sensors, teardown);
  g_test_add ("/popup/a-read-changes-the-widgets-in-place", Fixture, NULL, setup, test_a_read_changes_the_widgets_in_place, teardown);
  g_test_add ("/popup/a-chip-that-goes-takes-its-tile", Fixture, NULL, setup, test_a_chip_that_goes_takes_its_tile, teardown);
  return g_test_run ();
}
