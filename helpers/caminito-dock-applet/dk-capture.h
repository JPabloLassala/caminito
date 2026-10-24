/*
 * A window's picture (CMN-251), for the Dock preview and a Minimised window item (CMN-242):
 * an ext-image-copy-capture session on the window's ext-foreign-toplevel-image-capture-source,
 * on GDK's own connection, with the frames copied into shm buffers. A minimised window gives
 * its picture as it was last drawn, as Caminito captures it so.
 */

#pragma once

#include <cairo.h>
#include <glib.h>

struct ext_foreign_toplevel_handle_v1;

#define DK_CAPTURE_ERROR (dk_capture_error_quark ())

typedef enum
{
  /* The display isn't Wayland's, or the compositor has no capture globals (not Caminito). */
  DK_CAPTURE_ERROR_UNSUPPORTED,
  /* The compositor stopped the session: the window is gone, or was never there. */
  DK_CAPTURE_ERROR_STOPPED,
  /* The session asks for buffers the dock can't make: no shm format cairo reads. */
  DK_CAPTURE_ERROR_BUFFER,
  /* A frame failed for a reason the compositor didn't give. */
  DK_CAPTURE_ERROR_FAILED,
} DkCaptureError;

GQuark dk_capture_error_quark (void);

typedef struct _DkCapture DkCapture;

/* A frame of the window, or the error that ended the session, with frame NULL. The frame is
   the capture's until the function returns: a caller that keeps it takes a reference. The
   function may free the capture. After an error, or after the frame of a one-frame capture,
   the session is over and no more calls come. */
typedef void (*DkCaptureFunc) (DkCapture *capture, cairo_surface_t *frame, const GError *error,
                               gpointer data);

/* A session on the window of handle, the ext handle dk_toplevel_lookup gives. A live session
   gives every new frame, as the window redraws, until it is freed; a one-frame session (once)
   gives its first frame and stops. The frames come from GDK's main loop, never from inside
   this call. Returns NULL with error set at once where there are no capture globals, for the
   caller to show the program's icon. */
DkCapture *dk_capture_start (struct ext_foreign_toplevel_handle_v1 *handle, gboolean once,
                             DkCaptureFunc func, gpointer data, GError **error);

/* Whether the session goes on: FALSE once it has stopped, failed or given its one frame. */
gboolean dk_capture_is_open (DkCapture *capture);

/* Stops the session, if it is open, and frees the capture. NULL is allowed. */
void dk_capture_free (DkCapture *capture);

G_DEFINE_AUTOPTR_CLEANUP_FUNC (DkCapture, dk_capture_free)
