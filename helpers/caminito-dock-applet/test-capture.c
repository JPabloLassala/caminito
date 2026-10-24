/*
 * A window's picture where there is nothing to capture it with (CMN-251): GDK has no display,
 * as it has no Wayland one under X11 MATE. Caminito's own captures are tested end to end in
 * caminito's dock_applet test, through dk-probe.
 */

#include "dk-capture.h"

static void
on_frame (DkCapture *capture G_GNUC_UNUSED, cairo_surface_t *frame G_GNUC_UNUSED,
          const GError *error G_GNUC_UNUSED, gpointer data)
{
  *(gboolean *) data = TRUE;
}

static void
test_without_a_wayland_display_the_capture_fails_at_once (void)
{
  /* Never read: the capture fails before it names the window. */
  int window;
  gboolean called = FALSE;
  g_autoptr (GError) error = NULL;
  g_autoptr (DkCapture) capture =
    dk_capture_start ((struct ext_foreign_toplevel_handle_v1 *) &window, FALSE, on_frame,
                      &called, &error);

  g_assert_null (capture);
  g_assert_error (error, DK_CAPTURE_ERROR, DK_CAPTURE_ERROR_UNSUPPORTED);
  g_assert_false (called);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/capture/without-a-wayland-display-fails-at-once",
                   test_without_a_wayland_display_the_capture_fails_at_once);
  return g_test_run ();
}
