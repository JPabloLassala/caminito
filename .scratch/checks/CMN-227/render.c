/*
 * CMN-227's badge and progress bar, drawn by the Dock item's own draw handler into PNGs:
 * every state on a panel on each of the four edges, 64 pixels thick, one PNG each and sheet.png
 * with all of them, a row for each state and a column for each edge (bottom, top, left,
 * right), in the order of `looks`. It links dk-item.c with a
 * stand-in for the one libxfce4windowing call the item makes, so it needs GTK 3 and a
 * display (run.sh gives it Xvfb), not libxfce4windowing.
 *
 *   render DIRECTORY
 */

#include <gtk/gtk.h>

#include "dk-item.h"

/* The item tells the compositor where its windows are; here there is none to tell. */
void
xfw_window_set_button_geometry (gpointer window G_GNUC_UNUSED, GdkWindow *parent G_GNUC_UNUSED,
                                const GdkRectangle *rect G_GNUC_UNUSED, gpointer extra G_GNUC_UNUSED)
{
}

typedef struct
{
  const char *name;
  DkLauncherEntry entry;
  gboolean urgent;
} Look;

static void
settle (void)
{
  for (int i = 0; i < 20; i++)
    while (g_main_context_iteration (NULL, FALSE))
      ;
}

#define CELL 72
/* The sheet is enlarged, to be read by eye. */
#define ZOOM 3

/* A cell of the sheet: a row for each state and a column for each edge. */
static void
render (const char *directory, cairo_t *sheet, int row, int column, const char *edge_name,
        GtkPositionType edge, const Look *look)
{
  GtkWidget *offscreen = gtk_offscreen_window_new ();
  DkItemView *view = dk_item_view_new ();
  GtkWidget *widget = dk_item_view_get_widget (view);
  g_autoptr (GPtrArray) windows = g_ptr_array_new_with_free_func (g_object_unref);
  DkFeedbackState feedback = { .highlighted = look->urgent, .bounce_start = -1 };
  g_autoptr (GIcon) icon = g_themed_icon_new ("folder");
  g_autofree char *file = g_strdup_printf ("%s/%s-%s.png", directory, look->name, edge_name);
  cairo_surface_t *surface;

  g_ptr_array_add (windows, g_object_new (G_TYPE_OBJECT, NULL));
  dk_item_view_set_geometry (view, 64, edge);
  dk_item_view_set (view, "org.mate.Caja.desktop", icon, "Caja", windows, FALSE, TRUE);
  dk_item_view_set_feedback (view, &feedback);
  dk_item_view_set_launcher (view, &look->entry);
  gtk_container_add (GTK_CONTAINER (offscreen), widget);
  gtk_widget_show_all (offscreen);
  settle ();
  surface = gtk_offscreen_window_get_surface (GTK_OFFSCREEN_WINDOW (offscreen));
  g_assert_nonnull (surface);
  /* A panel's colour behind it, as the probe has. */
  {
    cairo_surface_t *flat = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 64, 64);
    cairo_t *cr = cairo_create (flat);

    cairo_set_source_rgb (cr, 0x30 / 255.0, 0x32 / 255.0, 0x36 / 255.0);
    cairo_paint (cr);
    cairo_set_source_surface (cr, surface, 0, 0);
    cairo_paint (cr);
    cairo_destroy (cr);
    g_assert_cmpint (cairo_surface_write_to_png (flat, file), ==, CAIRO_STATUS_SUCCESS);
    cairo_set_source_surface (sheet, flat, column * CELL + 4, row * CELL + 4);
    cairo_pattern_set_filter (cairo_get_source (sheet), CAIRO_FILTER_NEAREST);
    cairo_paint (sheet);
    cairo_surface_destroy (flat);
  }
  gtk_widget_destroy (offscreen);
}

int
main (int argc, char **argv)
{
  static const Look looks[] = {
    { "badge-5", { .count = 5, .count_visible = TRUE }, FALSE },
    { "badge-150", { .count = 150, .count_visible = TRUE }, FALSE },
    { "badge-hidden", { .count = 5, .count_visible = FALSE }, FALSE },
    { "badge-zero", { .count = 0, .count_visible = TRUE }, FALSE },
    { "progress-40", { .progress = 0.4, .progress_visible = TRUE }, FALSE },
    { "progress-0", { .progress = 0.0, .progress_visible = TRUE }, FALSE },
    { "progress-100", { .progress = 1.0, .progress_visible = TRUE }, FALSE },
    { "both", { .count = 12, .count_visible = TRUE, .progress = 0.7, .progress_visible = TRUE }, FALSE },
    { "urgent-and-both", { .count = 3, .count_visible = TRUE, .progress = 0.25,
                           .progress_visible = TRUE, .urgent = TRUE }, TRUE },
  };
  static const struct { const char *name; GtkPositionType edge; } edges[] = {
    { "bottom", GTK_POS_BOTTOM }, { "top", GTK_POS_TOP },
    { "left", GTK_POS_LEFT }, { "right", GTK_POS_RIGHT },
  };

  cairo_surface_t *surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 4 * CELL * ZOOM,
                                                         G_N_ELEMENTS (looks) * CELL * ZOOM);
  cairo_t *sheet = cairo_create (surface);
  g_autofree char *file = NULL;

  gtk_init (&argc, &argv);
  g_assert_cmpint (argc, ==, 2);
  cairo_scale (sheet, ZOOM, ZOOM);
  cairo_set_source_rgb (sheet, 0.12, 0.12, 0.12);
  cairo_paint (sheet);
  for (guint i = 0; i < G_N_ELEMENTS (looks); i++)
    for (guint j = 0; j < G_N_ELEMENTS (edges); j++)
      render (argv[1], sheet, i, j, edges[j].name, edges[j].edge, &looks[i]);
  file = g_strdup_printf ("%s/sheet.png", argv[1]);
  g_assert_cmpint (cairo_surface_write_to_png (surface, file), ==, CAIRO_STATUS_SUCCESS);
  cairo_destroy (sheet);
  cairo_surface_destroy (surface);
  return 0;
}
