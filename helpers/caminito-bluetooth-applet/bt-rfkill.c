/*
 * The rfkill block behind Bluetooth off (259). See bt-rfkill.h.
 */

#include "bt-rfkill.h"

#include <errno.h>
#include <fcntl.h>
#include <gio/gio.h>
#include <linux/rfkill.h>
#include <unistd.h>

gboolean
bt_rfkill_set_blocked (const char *path, gboolean blocked, GError **error)
{
  struct rfkill_event event = {
    .type = RFKILL_TYPE_BLUETOOTH,
    .op = RFKILL_OP_CHANGE_ALL,
    .soft = blocked ? 1 : 0,
  };
  ssize_t written;
  int fd;

  fd = open (path, O_WRONLY | O_CLOEXEC);
  if (fd < 0)
    {
      int saved = errno;

      g_set_error (error, G_IO_ERROR, g_io_error_from_errno (saved),
                   "can't open %s: %s", path, g_strerror (saved));
      return FALSE;
    }
  /* The original event's size, which every kernel takes (linux/rfkill.h). */
  written = write (fd, &event, RFKILL_EVENT_SIZE_V1);
  if (written != (ssize_t) RFKILL_EVENT_SIZE_V1)
    {
      int saved = written < 0 ? errno : EIO;

      g_set_error (error, G_IO_ERROR, g_io_error_from_errno (saved),
                   "can't write to %s: %s", path, g_strerror (saved));
      close (fd);
      return FALSE;
    }
  close (fd);
  return TRUE;
}
