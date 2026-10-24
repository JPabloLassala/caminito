/*
 * The app id to desktop entry resolution (CMN-219), against catalogs built by hand.
 */

#include "dk-resolve.h"

static DkCatalog *
catalog (void)
{
  DkCatalog *catalog = dk_catalog_new ();

  dk_catalog_add (catalog, "firefox.desktop", "Navigator");
  dk_catalog_add (catalog, "org.gnome.Nautilus.desktop", NULL);
  dk_catalog_add (catalog, "gimp.desktop", "Gimp-2.10");
  dk_catalog_add (catalog, "mate-terminal.desktop", "");
  return catalog;
}

static void
test_app_id_is_the_entrys_id (void)
{
  g_autoptr (DkCatalog) c = catalog ();

  g_assert_cmpstr (dk_catalog_resolve (c, "org.gnome.Nautilus"), ==,
                   "org.gnome.Nautilus.desktop");
  g_assert_cmpstr (dk_catalog_resolve (c, "org.gnome.Nautilus.desktop"), ==,
                   "org.gnome.Nautilus.desktop");
  g_assert_cmpstr (dk_catalog_resolve (c, "firefox"), ==, "firefox.desktop");
}

static void
test_case_does_not_matter (void)
{
  g_autoptr (DkCatalog) c = catalog ();

  g_assert_cmpstr (dk_catalog_resolve (c, "Firefox"), ==, "firefox.desktop");
  g_assert_cmpstr (dk_catalog_resolve (c, "ORG.GNOME.NAUTILUS"), ==,
                   "org.gnome.Nautilus.desktop");
}

/* An XWayland window has no app id but its WM_CLASS, which Caminito sends as the app id. */
static void
test_xwayland_wm_class_is_matched_by_startup_wm_class (void)
{
  g_autoptr (DkCatalog) c = catalog ();

  g_assert_cmpstr (dk_catalog_resolve (c, "Navigator"), ==, "firefox.desktop");
  g_assert_cmpstr (dk_catalog_resolve (c, "gimp-2.10"), ==, "gimp.desktop");
}

static void
test_the_id_wins_over_a_startup_wm_class (void)
{
  g_autoptr (DkCatalog) c = dk_catalog_new ();

  dk_catalog_add (c, "other.desktop", "firefox");
  dk_catalog_add (c, "firefox.desktop", NULL);
  g_assert_cmpstr (dk_catalog_resolve (c, "firefox"), ==, "firefox.desktop");
}

static void
test_no_entry_resolves_to_nothing (void)
{
  g_autoptr (DkCatalog) c = catalog ();

  g_assert_null (dk_catalog_resolve (c, "unknown-app"));
  g_assert_null (dk_catalog_resolve (c, ""));
  g_assert_null (dk_catalog_resolve (c, NULL));
  /* An entry with an empty StartupWMClass isn't named by an empty app id. */
  g_assert_null (dk_catalog_resolve (c, "mate-terminal.desktop.x"));
}

static void
test_the_first_entry_with_an_id_is_kept (void)
{
  g_autoptr (DkCatalog) c = dk_catalog_new ();

  dk_catalog_add (c, "firefox.desktop", "Navigator");
  dk_catalog_add (c, "firefox.desktop", "Other");
  g_assert_cmpstr (dk_catalog_resolve (c, "Navigator"), ==, "firefox.desktop");
  g_assert_null (dk_catalog_resolve (c, "Other"));
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/resolve/app-id-is-the-entrys-id", test_app_id_is_the_entrys_id);
  g_test_add_func ("/resolve/case-does-not-matter", test_case_does_not_matter);
  g_test_add_func ("/resolve/xwayland-wm-class-is-matched-by-startup-wm-class",
                   test_xwayland_wm_class_is_matched_by_startup_wm_class);
  g_test_add_func ("/resolve/the-id-wins-over-a-startup-wm-class",
                   test_the_id_wins_over_a_startup_wm_class);
  g_test_add_func ("/resolve/no-entry-resolves-to-nothing", test_no_entry_resolves_to_nothing);
  g_test_add_func ("/resolve/the-first-entry-with-an-id-is-kept",
                   test_the_first_entry_with_an_id_is_kept);
  return g_test_run ();
}
