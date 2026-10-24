/*
 * The kept Dock items' persistence (CMN-219): keeping, removing and reordering survive the
 * panel's restart, which is a new store over the same settings file. The schema is the
 * installed one, compiled by the build into GSETTINGS_SCHEMA_DIR, over a keyfile backend.
 */

#define G_SETTINGS_ENABLE_BACKEND
#include <gio/gsettingsbackend.h>

#include "dk-store.h"

static const char *path = "/org/mate/panel/objects/dock/prefs/";

/* A panel's start: the settings from the file, the way dconf would hand them over. */
static GSettings *
open_settings (const char *file)
{
  GSettingsSchema *schema = g_settings_schema_source_lookup (
    g_settings_schema_source_get_default (), "org.mate.panel.applet.caminito-dock", TRUE);
  GSettingsBackend *backend = g_keyfile_settings_backend_new (file, "/", NULL);
  GSettings *settings;

  g_assert_nonnull (schema);
  settings = g_settings_new_full (schema, backend, path);
  g_settings_schema_unref (schema);
  g_object_unref (backend);
  return settings;
}

static char *
settings_file (void)
{
  g_autofree char *dir = g_dir_make_tmp ("caminito-dock-XXXXXX", NULL);

  return g_build_filename (dir, "keyfile", NULL);
}

static void
assert_kept (DkStore *store, const char *a, const char *b, const char *c)
{
  const char *const *kept = dk_store_get (store);
  const char *want[] = { a, b, c };
  guint n = 0;

  for (guint i = 0; i < G_N_ELEMENTS (want); i++)
    if (want[i] != NULL)
      g_assert_cmpstr (kept[n++], ==, want[i]);
  g_assert_null (kept[n]);
}

static void
test_keeping_removing_and_reordering_survive_a_restart (void)
{
  g_autofree char *file = settings_file ();
  g_autoptr (GSettings) first = open_settings (file);
  DkStore *store = dk_store_new (first);

  assert_kept (store, NULL, NULL, NULL);
  g_assert_true (dk_store_keep (store, "firefox.desktop"));
  g_assert_true (dk_store_keep (store, "mate-terminal.desktop"));
  g_assert_true (dk_store_keep (store, "org.gnome.Nautilus.desktop"));
  g_assert_true (dk_store_move (store, "org.gnome.Nautilus.desktop", 0));
  g_assert_true (dk_store_remove (store, "firefox.desktop"));
  assert_kept (store, "org.gnome.Nautilus.desktop", "mate-terminal.desktop", NULL);
  dk_store_free (store);
  g_settings_sync ();

  {
    g_autoptr (GSettings) second = open_settings (file);
    DkStore *restarted = dk_store_new (second);

    assert_kept (restarted, "org.gnome.Nautilus.desktop", "mate-terminal.desktop", NULL);
    g_assert_true (dk_store_is_kept (restarted, "mate-terminal.desktop"));
    g_assert_false (dk_store_is_kept (restarted, "firefox.desktop"));
    dk_store_free (restarted);
  }
}

static int changes;

static void
count_change (gpointer data G_GNUC_UNUSED)
{
  changes++;
}

static void
test_the_store_follows_a_change_made_elsewhere (void)
{
  g_autofree char *file = settings_file ();
  g_autoptr (GSettings) settings = open_settings (file);
  DkStore *store = dk_store_new (settings);
  const char *const kept[] = { "a.desktop", "b.desktop", NULL };

  changes = 0;
  dk_store_set_changed (store, count_change, NULL);
  g_settings_set_strv (settings, "kept", kept);
  assert_kept (store, "a.desktop", "b.desktop", NULL);
  g_assert_cmpint (changes, ==, 1);
  /* Its own change is one change. */
  g_assert_true (dk_store_keep (store, "c.desktop"));
  g_assert_cmpint (changes, ==, 2);
  g_assert_false (dk_store_keep (store, "c.desktop"));
  g_assert_cmpint (changes, ==, 2);
  dk_store_free (store);
}

static void
test_with_no_settings_the_items_are_kept_for_the_session (void)
{
  DkStore *store = dk_store_new (NULL);

  g_assert_true (dk_store_keep (store, "a.desktop"));
  g_assert_true (dk_store_keep (store, "b.desktop"));
  assert_kept (store, "a.desktop", "b.desktop", NULL);
  dk_store_free (store);
}

static void
assert_folders (DkStore *store, const char *a, const char *b)
{
  const char *const *folders = dk_store_get_folders (store);
  const char *want[] = { a, b };
  guint n = 0;

  for (guint i = 0; i < G_N_ELEMENTS (want); i++)
    if (want[i] != NULL)
      g_assert_cmpstr (folders[n++], ==, want[i]);
  g_assert_null (folders[n]);
}

static void
test_kept_folders_survive_a_restart_apart_from_the_kept_items (void)
{
  g_autofree char *file = settings_file ();
  g_autoptr (GSettings) first = open_settings (file);
  DkStore *store = dk_store_new (first);

  assert_folders (store, NULL, NULL);
  g_assert_true (dk_store_keep (store, "firefox.desktop"));
  g_assert_true (dk_store_keep_folder (store, "file:///home/u/Music"));
  g_assert_true (dk_store_keep_folder (store, "file:///home/u/Documents"));
  g_assert_false (dk_store_keep_folder (store, "file:///home/u/Documents"));
  g_assert_true (dk_store_move_folder (store, "file:///home/u/Documents", 0));
  assert_folders (store, "file:///home/u/Documents", "file:///home/u/Music");
  assert_kept (store, "firefox.desktop", NULL, NULL);
  dk_store_free (store);
  g_settings_sync ();

  {
    g_autoptr (GSettings) second = open_settings (file);
    DkStore *restarted = dk_store_new (second);

    assert_folders (restarted, "file:///home/u/Documents", "file:///home/u/Music");
    assert_kept (restarted, "firefox.desktop", NULL, NULL);
    g_assert_true (dk_store_is_folder_kept (restarted, "file:///home/u/Music"));
    g_assert_true (dk_store_remove_folder (restarted, "file:///home/u/Music"));
    g_assert_false (dk_store_remove_folder (restarted, "file:///home/u/Music"));
    g_assert_false (dk_store_is_folder_kept (restarted, "file:///home/u/Music"));
    assert_folders (restarted, "file:///home/u/Documents", NULL);
    dk_store_free (restarted);
  }
}

static void
test_the_store_follows_a_change_to_the_folders_made_elsewhere (void)
{
  g_autofree char *file = settings_file ();
  g_autoptr (GSettings) settings = open_settings (file);
  DkStore *store = dk_store_new (settings);
  const char *const folders[] = { "file:///a", "file:///b", NULL };

  changes = 0;
  dk_store_set_changed (store, count_change, NULL);
  g_settings_set_strv (settings, "kept-folders", folders);
  assert_folders (store, "file:///a", "file:///b");
  g_assert_cmpint (changes, ==, 1);
  g_assert_true (dk_store_keep_folder (store, "file:///c"));
  g_assert_cmpint (changes, ==, 2);
  dk_store_free (store);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/store/keeping-removing-and-reordering-survive-a-restart",
                   test_keeping_removing_and_reordering_survive_a_restart);
  g_test_add_func ("/store/follows-a-change-made-elsewhere",
                   test_the_store_follows_a_change_made_elsewhere);
  g_test_add_func ("/store/with-no-settings-the-items-are-kept-for-the-session",
                   test_with_no_settings_the_items_are_kept_for_the_session);
  g_test_add_func ("/store/kept-folders-survive-a-restart-apart-from-the-kept-items",
                   test_kept_folders_survive_a_restart_apart_from_the_kept_items);
  g_test_add_func ("/store/follows-a-change-to-the-folders-made-elsewhere",
                   test_the_store_follows_a_change_to_the_folders_made_elsewhere);
  return g_test_run ();
}
