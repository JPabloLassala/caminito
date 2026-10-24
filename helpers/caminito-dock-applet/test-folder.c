/*
 * A Kept folder's folder (CMN-239), without a panel: its contents in the order the popup shows
 * them, its custom icon, and putting files in it, a move on the file system the folder is on and
 * a copy from another. The folders are the test's own.
 */

#include <gio/gio.h>
#include <glib/gstdio.h>

#include "dk-folder.h"

static char *
make_dir (const char *base)
{
  g_autofree char *template = g_build_filename (base, "caminito-folder-XXXXXX", NULL);

  return g_mkdtemp (g_steal_pointer (&template));
}

static void
write_file (const char *dir, const char *name, const char *contents)
{
  g_autofree char *path = g_build_filename (dir, name, NULL);

  g_assert_true (g_file_set_contents (path, contents, -1, NULL));
}

static void
make_subdir (const char *dir, const char *name)
{
  g_autofree char *path = g_build_filename (dir, name, NULL);

  g_assert_cmpint (g_mkdir_with_parents (path, 0700), ==, 0);
}

static char *
uri_of (const char *dir, const char *name)
{
  g_autofree char *path = g_build_filename (dir, name, NULL);

  return g_filename_to_uri (path, NULL, NULL);
}

static gboolean
has (const char *dir, const char *name)
{
  g_autofree char *path = g_build_filename (dir, name, NULL);

  return g_file_test (path, G_FILE_TEST_EXISTS);
}

static void
test_the_contents_are_folders_first_then_by_name_without_the_hidden (void)
{
  g_autofree char *dir = make_dir (g_get_tmp_dir ());
  g_autoptr (GFile) folder = g_file_new_for_path (dir);
  g_autoptr (GPtrArray) entries = NULL;
  g_autoptr (GError) error = NULL;

  write_file (dir, "b.txt", "b");
  write_file (dir, "A.txt", "a");
  write_file (dir, ".hidden", "h");
  write_file (dir, "backup~", "b");
  make_subdir (dir, "zeta");
  make_subdir (dir, "alpha");
  entries = dk_folder_list (folder, &error);
  g_assert_no_error (error);
  g_assert_cmpuint (entries->len, ==, 4);
  g_assert_cmpstr (((DkFolderEntry *) entries->pdata[0])->name, ==, "alpha");
  g_assert_true (((DkFolderEntry *) entries->pdata[0])->is_folder);
  g_assert_cmpstr (((DkFolderEntry *) entries->pdata[1])->name, ==, "zeta");
  g_assert_cmpstr (((DkFolderEntry *) entries->pdata[2])->name, ==, "A.txt");
  g_assert_false (((DkFolderEntry *) entries->pdata[2])->is_folder);
  g_assert_cmpstr (((DkFolderEntry *) entries->pdata[3])->name, ==, "b.txt");
  g_assert_nonnull (((DkFolderEntry *) entries->pdata[3])->icon);
}

static void
test_a_folder_that_is_gone_does_not_exist_and_cant_be_listed (void)
{
  g_autofree char *dir = make_dir (g_get_tmp_dir ());
  g_autoptr (GFile) folder = g_file_new_for_path (dir);
  g_autoptr (GError) error = NULL;

  g_assert_true (dk_folder_exists (folder));
  g_assert_cmpstr (dk_folder_display_name (folder), !=, NULL);
  g_assert_cmpint (g_rmdir (dir), ==, 0);
  g_assert_false (dk_folder_exists (folder));
  g_assert_null (dk_folder_list (folder, &error));
  g_assert_nonnull (error);
  /* It comes back with the folder. */
  g_assert_cmpint (g_mkdir (dir, 0700), ==, 0);
  g_assert_true (dk_folder_exists (folder));
  g_assert_cmpint (g_rmdir (dir), ==, 0);
}

static void
test_a_file_is_not_a_folder_to_keep (void)
{
  g_autofree char *dir = make_dir (g_get_tmp_dir ());
  g_autofree char *file = NULL;
  g_autofree char *folder = g_filename_to_uri (dir, NULL, NULL);

  write_file (dir, "f.txt", "f");
  file = uri_of (dir, "f.txt");
  g_assert_true (dk_folder_uri_is_folder (folder));
  g_assert_false (dk_folder_uri_is_folder (file));
}

static void
test_a_custom_icon_in_caja_is_the_folders_icon (void)
{
  g_autofree char *dir = make_dir (g_get_tmp_dir ());
  g_autoptr (GFile) folder = g_file_new_for_path (dir);
  g_autofree char *image = g_build_filename (dir, "pic.png", NULL);
  g_autofree char *image_uri = g_filename_to_uri (image, NULL, NULL);
  g_autoptr (GFile) image_file = g_file_new_for_path (image);
  g_autoptr (GIcon) plain = dk_folder_icon (folder);
  g_autoptr (GIcon) custom = NULL;
  g_autoptr (GError) error = NULL;

  g_assert_nonnull (plain);
  g_assert_false (G_IS_FILE_ICON (plain));
  write_file (dir, "pic.png", "not really");
  /* Metadata needs gvfs's daemon; where it is missing the folder keeps its own icon. */
  if (!g_file_set_attribute_string (folder, "metadata::custom-icon", image_uri,
                                    G_FILE_QUERY_INFO_NONE, NULL, &error))
    {
      g_test_skip ("no metadata store for the custom icon");
      return;
    }
  custom = dk_folder_icon (folder);
  g_assert_true (G_IS_FILE_ICON (custom));
  g_assert_true (g_file_equal (g_file_icon_get_file (G_FILE_ICON (custom)), image_file));
}

typedef struct
{
  GMainLoop *loop;
  GPtrArray *failed;
} Putting;

static void
on_failed (const char *uri, GError *error G_GNUC_UNUSED, gpointer data)
{
  g_ptr_array_add (((Putting *) data)->failed, g_strdup (uri));
}

static void
on_done (gpointer data)
{
  g_main_loop_quit (((Putting *) data)->loop);
}

/* Puts the URIs in the folder, and waits for the end. */
static GPtrArray *
put (const char *folder_dir, const char *const *uris)
{
  g_autoptr (GFile) folder = g_file_new_for_path (folder_dir);
  Putting putting = { g_main_loop_new (NULL, FALSE), g_ptr_array_new_with_free_func (g_free) };

  dk_folder_put (folder, uris, NULL, on_failed, on_done, &putting, NULL);
  g_main_loop_run (putting.loop);
  g_main_loop_unref (putting.loop);
  return putting.failed;
}

static void
test_a_file_dropped_on_the_same_file_system_is_moved (void)
{
  g_autofree char *from = make_dir (g_get_tmp_dir ());
  g_autofree char *into = make_dir (g_get_tmp_dir ());
  g_autofree char *file = NULL;
  g_autofree char *folder = NULL;
  g_autoptr (GPtrArray) failed = NULL;

  write_file (from, "f.txt", "f");
  make_subdir (from, "d");
  write_file (from, "d/inner.txt", "i");
  file = uri_of (from, "f.txt");
  folder = uri_of (from, "d");
  failed = put (into, (const char *const[]) { file, folder, NULL });
  g_assert_cmpuint (failed->len, ==, 0);
  g_assert_true (has (into, "f.txt"));
  g_assert_false (has (from, "f.txt"));
  g_assert_true (has (into, "d/inner.txt"));
  g_assert_false (has (from, "d"));
}

static void
test_a_file_dropped_from_another_file_system_is_copied (void)
{
  g_autofree char *from = make_dir ("/dev/shm");
  g_autofree char *into = make_dir (g_get_tmp_dir ());
  g_autoptr (GFile) a = g_file_new_for_path (from);
  g_autoptr (GFile) b = g_file_new_for_path (into);
  g_autoptr (GFileInfo) ia = g_file_query_info (a, "id::filesystem", 0, NULL, NULL);
  g_autoptr (GFileInfo) ib = g_file_query_info (b, "id::filesystem", 0, NULL, NULL);
  g_autofree char *file = NULL;
  g_autofree char *folder = NULL;
  g_autoptr (GPtrArray) failed = NULL;

  if (from == NULL || ia == NULL || ib == NULL
      || g_strcmp0 (g_file_info_get_attribute_string (ia, "id::filesystem"),
                    g_file_info_get_attribute_string (ib, "id::filesystem")) == 0)
    {
      g_test_skip ("no second file system");
      return;
    }
  write_file (from, "f.txt", "f");
  make_subdir (from, "d/sub");
  write_file (from, "d/sub/inner.txt", "i");
  file = uri_of (from, "f.txt");
  folder = uri_of (from, "d");
  failed = put (into, (const char *const[]) { file, folder, NULL });
  g_assert_cmpuint (failed->len, ==, 0);
  g_assert_true (has (into, "f.txt"));
  g_assert_true (has (from, "f.txt"));
  g_assert_true (has (into, "d/sub/inner.txt"));
  g_assert_true (has (from, "d/sub/inner.txt"));
}

static void
test_a_file_already_in_the_folder_stays_and_a_clash_fails (void)
{
  g_autofree char *from = make_dir (g_get_tmp_dir ());
  g_autofree char *into = make_dir (g_get_tmp_dir ());
  g_autofree char *inside = NULL;
  g_autofree char *clash = NULL;
  g_autoptr (GPtrArray) failed = NULL;

  write_file (into, "here.txt", "h");
  write_file (into, "clash.txt", "old");
  write_file (from, "clash.txt", "new");
  inside = uri_of (into, "here.txt");
  clash = uri_of (from, "clash.txt");
  failed = put (into, (const char *const[]) { inside, clash, NULL });
  /* Where it came from is no error, a name taken is one, and nothing is overwritten. */
  g_assert_cmpuint (failed->len, ==, 1);
  g_assert_cmpstr (failed->pdata[0], ==, clash);
  g_assert_true (has (into, "here.txt"));
  g_assert_true (has (from, "clash.txt"));
  {
    g_autofree char *path = g_build_filename (into, "clash.txt", NULL);
    g_autofree char *contents = NULL;

    g_assert_true (g_file_get_contents (path, &contents, NULL, NULL));
    g_assert_cmpstr (contents, ==, "old");
  }
}

static void
test_a_folder_cant_be_dropped_into_itself (void)
{
  g_autofree char *dir = make_dir (g_get_tmp_dir ());
  g_autofree char *folder = NULL;
  g_autofree char *inner = NULL;
  g_autoptr (GPtrArray) failed = NULL;

  make_subdir (dir, "d/e");
  folder = uri_of (dir, "d");
  inner = g_build_filename (dir, "d", "e", NULL);
  failed = put (inner, (const char *const[]) { folder, NULL });
  g_assert_cmpuint (failed->len, ==, 1);
  g_assert_true (has (dir, "d/e"));
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/folder/contents-folders-first-then-by-name-without-the-hidden",
                   test_the_contents_are_folders_first_then_by_name_without_the_hidden);
  g_test_add_func ("/folder/a-folder-that-is-gone-does-not-exist",
                   test_a_folder_that_is_gone_does_not_exist_and_cant_be_listed);
  g_test_add_func ("/folder/a-file-is-not-a-folder-to-keep", test_a_file_is_not_a_folder_to_keep);
  g_test_add_func ("/folder/a-custom-icon-in-caja-is-the-icon",
                   test_a_custom_icon_in_caja_is_the_folders_icon);
  g_test_add_func ("/folder/put/a-file-on-the-same-file-system-is-moved",
                   test_a_file_dropped_on_the_same_file_system_is_moved);
  g_test_add_func ("/folder/put/a-file-from-another-file-system-is-copied",
                   test_a_file_dropped_from_another_file_system_is_copied);
  g_test_add_func ("/folder/put/a-file-already-there-stays-and-a-clash-fails",
                   test_a_file_already_in_the_folder_stays_and_a_clash_fails);
  g_test_add_func ("/folder/put/a-folder-cant-be-dropped-into-itself",
                   test_a_folder_cant_be_dropped_into_itself);
  return g_test_run ();
}
