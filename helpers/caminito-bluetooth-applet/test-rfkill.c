/*
 * The event that blocks or unblocks every Bluetooth radio (259), written to a file in the
 * device's place.
 */

#include <gio/gio.h>
#include <glib/gstdio.h>
#include <linux/rfkill.h>

#include "bt-rfkill.h"

static void
check_event (gboolean blocked)
{
  g_autoptr (GError) error = NULL;
  g_autofree char *path = NULL;
  g_autofree char *contents = NULL;
  struct rfkill_event event;
  gsize length;
  int fd;

  fd = g_file_open_tmp ("test-rfkill-XXXXXX", &path, &error);
  g_assert_no_error (error);
  g_close (fd, NULL);

  g_assert_true (bt_rfkill_set_blocked (path, blocked, &error));
  g_assert_no_error (error);
  g_assert_true (g_file_get_contents (path, &contents, &length, &error));
  g_assert_cmpuint (length, ==, RFKILL_EVENT_SIZE_V1);
  memcpy (&event, contents, sizeof event);
  g_assert_cmpuint (event.idx, ==, 0);
  g_assert_cmpuint (event.type, ==, RFKILL_TYPE_BLUETOOTH);
  g_assert_cmpuint (event.op, ==, RFKILL_OP_CHANGE_ALL);
  g_assert_cmpuint (event.soft, ==, blocked ? 1 : 0);
  g_assert_cmpuint (event.hard, ==, 0);
  g_unlink (path);
}

static void
test_block (void)
{
  check_event (TRUE);
}

static void
test_unblock (void)
{
  check_event (FALSE);
}

static void
test_no_device (void)
{
  g_autoptr (GError) error = NULL;

  g_assert_false (bt_rfkill_set_blocked ("/nonexistent/rfkill", TRUE, &error));
  g_assert_error (error, G_IO_ERROR, G_IO_ERROR_NOT_FOUND);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/rfkill/off-soft-blocks-every-bluetooth-radio", test_block);
  g_test_add_func ("/rfkill/on-unblocks-every-bluetooth-radio", test_unblock);
  g_test_add_func ("/rfkill/a-missing-device-is-an-error", test_no_device);
  return g_test_run ();
}
