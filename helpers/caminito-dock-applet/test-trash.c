/*
 * The Dock's Trash (CMN-219's follow-up, CMN-220), without a panel: whether it is full as it
 * changes, putting files in it and emptying it. The Dock opens `trash:///`, which needs
 * gvfs; here the Trash is a directory of the test's own, which is all the state reads, and
 * XDG_DATA_HOME is the test's too, so that GIO's own trashing of a file goes where the
 * directory is.
 */

#include <gio/gio.h>
#include <glib/gstdio.h>

#include "dk-trash.h"

typedef struct
{
  char    *files;
  GFile   *location;
  DkTrash *trash;
  guint    changes;
} Fixture;

static void
on_changed (gpointer data)
{
  ((Fixture *) data)->changes++;
}

static void
remove_tree (const char *path)
{
  g_autoptr (GDir) dir = g_dir_open (path, 0, NULL);
  const char *name;

  while (dir != NULL && (name = g_dir_read_name (dir)) != NULL)
    {
      g_autofree char *child = g_build_filename (path, name, NULL);

      if (g_file_test (child, G_FILE_TEST_IS_DIR))
        remove_tree (child);
      else
        g_unlink (child);
    }
  g_rmdir (path);
}

static void
fixture_set_up (Fixture *f, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *trash = g_build_filename (g_get_user_data_dir (), "Trash", NULL);
  g_autofree char *outside = g_build_filename (g_get_user_data_dir (), "outside", NULL);

  /* Each test starts with nothing in the Trash and nothing outside it. */
  remove_tree (trash);
  remove_tree (outside);
  f->files = g_build_filename (g_get_user_data_dir (), "Trash", "files", NULL);
  g_assert_cmpint (g_mkdir_with_parents (f->files, 0700), ==, 0);
  f->location = g_file_new_for_path (f->files);
  f->trash = dk_trash_new (f->location);
  dk_trash_set_changed (f->trash, on_changed, f);
}

static void
fixture_tear_down (Fixture *f, gconstpointer data G_GNUC_UNUSED)
{
  dk_trash_free (f->trash);
  g_object_unref (f->location);
  g_free (f->files);
}

static gboolean
on_timeout (gpointer data)
{
  *(gboolean *) data = TRUE;
  return G_SOURCE_REMOVE;
}

/* Runs the main loop until the Trash is (not) full. */
static void
wait_for_full (DkTrash *trash, gboolean full)
{
  gboolean timed_out = FALSE;
  guint id = g_timeout_add_seconds (5, on_timeout, &timed_out);

  while (dk_trash_is_full (trash) != full && !timed_out)
    g_main_context_iteration (NULL, TRUE);
  g_assert_false (timed_out);
  g_source_remove (id);
}

static char *
write_file (const char *dir, const char *name)
{
  char *path = g_build_filename (dir, name, NULL);

  g_assert_true (g_file_set_contents (path, "content", -1, NULL));
  return path;
}

static void
test_an_empty_trash_is_not_full (Fixture *f, gconstpointer data G_GNUC_UNUSED)
{
  g_main_context_iteration (NULL, FALSE);
  g_assert_false (dk_trash_is_full (f->trash));
}

static void
test_a_trash_that_has_files_at_the_start_is_full (Fixture *f, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *path = write_file (f->files, "old.txt");
  DkTrash *later = dk_trash_new (f->location);

  wait_for_full (later, TRUE);
  dk_trash_free (later);
}

static void
test_the_trash_follows_a_file_coming_and_going (Fixture *f, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *path = write_file (f->files, "a.txt");
  guint after_filling;

  wait_for_full (f->trash, TRUE);
  g_assert_cmpuint (f->changes, >, 0);
  after_filling = f->changes;
  g_assert_cmpint (g_unlink (path), ==, 0);
  wait_for_full (f->trash, FALSE);
  g_assert_cmpuint (f->changes, >, after_filling);
}

typedef struct
{
  gboolean done;
  GError  *error;
  guint    failures;
  char    *failed_uri;
} Result;

static void
on_emptied (GError *error, gpointer data)
{
  Result *r = data;

  r->error = error != NULL ? g_error_copy (error) : NULL;
  r->done = TRUE;
}

static void
run_until_done (const gboolean *done)
{
  gboolean timed_out = FALSE;
  guint id = g_timeout_add_seconds (5, on_timeout, &timed_out);

  while (!*done && !timed_out)
    g_main_context_iteration (NULL, TRUE);
  g_assert_false (timed_out);
  g_source_remove (id);
}

static void
test_emptying_removes_files_and_folders_with_their_contents (Fixture *f,
                                                             gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *folder = g_build_filename (f->files, "folder", NULL);
  g_autofree char *inner = g_build_filename (folder, "inner", NULL);
  g_autofree char *a = write_file (f->files, "a.txt");
  g_autofree char *b = NULL;
  g_autofree char *c = NULL;
  Result r = { 0 };

  g_assert_cmpint (g_mkdir_with_parents (inner, 0700), ==, 0);
  b = write_file (folder, "b.txt");
  c = write_file (inner, "c.txt");
  wait_for_full (f->trash, TRUE);
  dk_trash_empty (f->trash, on_emptied, &r);
  run_until_done (&r.done);
  g_assert_no_error (r.error);
  wait_for_full (f->trash, FALSE);
  g_assert_false (g_file_test (b, G_FILE_TEST_EXISTS));
  g_assert_false (g_file_test (c, G_FILE_TEST_EXISTS));
  g_assert_false (g_file_test (a, G_FILE_TEST_EXISTS));
  g_assert_false (g_file_test (folder, G_FILE_TEST_EXISTS));
}

static void
on_put_failed (const char *uri, GError *error G_GNUC_UNUSED, gpointer data)
{
  Result *r = data;

  r->failures++;
  g_free (r->failed_uri);
  r->failed_uri = g_strdup (uri);
}

static void
on_put_done (gpointer data)
{
  ((Result *) data)->done = TRUE;
}

static void
test_putting_files_moves_them_into_the_trash (Fixture *f, gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *dir = g_build_filename (g_get_user_data_dir (), "outside", NULL);
  g_autofree char *one = NULL;
  g_autofree char *two = NULL;
  g_autofree char *uri_one = NULL;
  g_autofree char *uri_two = NULL;
  const char *uris[3];
  Result r = { 0 };

  g_assert_cmpint (g_mkdir_with_parents (dir, 0700), ==, 0);
  one = write_file (dir, "one.txt");
  two = write_file (dir, "two.txt");
  uri_one = g_filename_to_uri (one, NULL, NULL);
  uri_two = g_filename_to_uri (two, NULL, NULL);
  uris[0] = uri_one;
  uris[1] = uri_two;
  uris[2] = NULL;
  g_assert_false (dk_trash_is_full (f->trash));
  dk_trash_put (f->trash, uris, on_put_failed, on_put_done, &r);
  run_until_done (&r.done);
  g_assert_cmpuint (r.failures, ==, 0);
  g_assert_false (g_file_test (one, G_FILE_TEST_EXISTS));
  g_assert_false (g_file_test (two, G_FILE_TEST_EXISTS));
  wait_for_full (f->trash, TRUE);
  g_assert_true (g_file_test (f->files, G_FILE_TEST_IS_DIR));
}

static void
test_a_file_that_cannot_be_put_is_reported_and_the_others_still_go (Fixture *f,
                                                                    gconstpointer data G_GNUC_UNUSED)
{
  g_autofree char *dir = g_build_filename (g_get_user_data_dir (), "outside", NULL);
  g_autofree char *good = NULL;
  g_autofree char *missing = g_build_filename (dir, "missing.txt", NULL);
  g_autofree char *uri_good = NULL;
  g_autofree char *uri_missing = g_filename_to_uri (missing, NULL, NULL);
  const char *uris[3];
  Result r = { 0 };

  g_assert_cmpint (g_mkdir_with_parents (dir, 0700), ==, 0);
  good = write_file (dir, "good.txt");
  uri_good = g_filename_to_uri (good, NULL, NULL);
  uris[0] = uri_missing;
  uris[1] = uri_good;
  uris[2] = NULL;
  dk_trash_put (f->trash, uris, on_put_failed, on_put_done, &r);
  run_until_done (&r.done);
  g_assert_cmpuint (r.failures, ==, 1);
  g_assert_cmpstr (r.failed_uri, ==, uri_missing);
  g_free (r.failed_uri);
  g_assert_false (g_file_test (good, G_FILE_TEST_EXISTS));
  wait_for_full (f->trash, TRUE);
}

int
main (int argc, char **argv)
{
  g_autofree char *home = g_dir_make_tmp ("caminito-dock-trash-XXXXXX", NULL);

  /* Before anything reads the user's directories. */
  g_assert_nonnull (home);
  g_setenv ("XDG_DATA_HOME", home, TRUE);
  g_setenv ("HOME", home, TRUE);
  g_test_init (&argc, &argv, NULL);
  g_test_add ("/trash/empty", Fixture, NULL, fixture_set_up,
              test_an_empty_trash_is_not_full, fixture_tear_down);
  g_test_add ("/trash/full-at-start", Fixture, NULL, fixture_set_up,
              test_a_trash_that_has_files_at_the_start_is_full, fixture_tear_down);
  g_test_add ("/trash/follows", Fixture, NULL, fixture_set_up,
              test_the_trash_follows_a_file_coming_and_going, fixture_tear_down);
  g_test_add ("/trash/empties", Fixture, NULL, fixture_set_up,
              test_emptying_removes_files_and_folders_with_their_contents, fixture_tear_down);
  g_test_add ("/trash/puts", Fixture, NULL, fixture_set_up,
              test_putting_files_moves_them_into_the_trash, fixture_tear_down);
  g_test_add ("/trash/put-failure", Fixture, NULL, fixture_set_up,
              test_a_file_that_cannot_be_put_is_reported_and_the_others_still_go,
              fixture_tear_down);
  return g_test_run ();
}
