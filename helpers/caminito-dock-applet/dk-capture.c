/*
 * A window's picture (CMN-251).
 */

/* memfd_create. */
#define _GNU_SOURCE

#include <gtk/gtk.h>

#include "dk-capture.h"

G_DEFINE_QUARK (dk-capture-error-quark, dk_capture_error)

#ifdef GDK_WINDOWING_WAYLAND
#include <sys/mman.h>
#include <unistd.h>

#include <gdk/gdkwayland.h>
#include <wayland-client.h>

#include "ext-image-capture-source-v1-client-protocol.h"
#include "ext-image-copy-capture-v1-client-protocol.h"

/* The capture globals and an shm of the dock's own. They are bound on a queue of the dock's
   own, as GDK must not take the registry's events, and then moved to the default queue: the
   sessions made from them are answered whenever the compositor has a frame, and GDK's
   dispatch of that queue delivers it on the main loop. */
typedef struct
{
  struct wl_display                                          *display;
  struct ext_image_copy_capture_manager_v1                   *manager;
  struct ext_foreign_toplevel_image_capture_source_manager_v1 *sources;
  struct wl_shm                                              *shm;
} Binding;

/* What a session asks of its buffers, as one round of constraints gives it. */
typedef struct
{
  int            width;
  int            height;
  /* The wl_shm format, and cairo's for the same pixels; format is -1 for none that cairo
     reads. */
  gint64         format;
  cairo_format_t cairo_format;
} Constraints;

struct _DkCapture
{
  Binding                                  *binding;
  gboolean                                  once;
  DkCaptureFunc                             func;
  gpointer                                  data;
  struct ext_image_capture_source_v1       *source;
  /* NULL once the session is over. */
  struct ext_image_copy_capture_session_v1 *session;
  /* The frame being copied, if any: a session has at most one. */
  struct ext_image_copy_capture_frame_v1   *frame;
  /* The constraints as they arrive, and those of the last done event. */
  Constraints                               pending;
  Constraints                               constraints;
  /* The buffer the frames are copied into, of the constraints in buffer_constraints. */
  struct wl_buffer                         *buffer;
  guint8                                   *pixels;
  gsize                                     length;
  int                                       stride;
  Constraints                               buffer_constraints;
  /* The caller's function is running, and the caller freed the capture from it. */
  gboolean                                  delivering;
  gboolean                                  freed;
};

static void
on_global (void *data, struct wl_registry *registry, uint32_t name, const char *interface,
           uint32_t version G_GNUC_UNUSED)
{
  Binding *binding = data;

  if (g_str_equal (interface, ext_image_copy_capture_manager_v1_interface.name))
    binding->manager = wl_registry_bind (registry, name,
                                         &ext_image_copy_capture_manager_v1_interface, 1);
  else if (g_str_equal (interface,
                        ext_foreign_toplevel_image_capture_source_manager_v1_interface.name))
    binding->sources =
      wl_registry_bind (registry, name,
                        &ext_foreign_toplevel_image_capture_source_manager_v1_interface, 1);
  else if (g_str_equal (interface, wl_shm_interface.name))
    binding->shm = wl_registry_bind (registry, name, &wl_shm_interface, 1);
}

static void
on_global_remove (void *data G_GNUC_UNUSED, struct wl_registry *registry G_GNUC_UNUSED,
                  uint32_t name G_GNUC_UNUSED)
{
}

static const struct wl_registry_listener registry_listener = {
  .global = on_global,
  .global_remove = on_global_remove,
};

/* The binding for GDK's display, made once: a process has one display. NULL where the
   compositor lacks a capture global. */
static Binding *
binding_for (struct wl_display *display)
{
  static Binding binding;
  static gboolean made;

  if (!made)
    {
      struct wl_event_queue *queue = wl_display_create_queue (display);
      struct wl_display *wrapper = wl_proxy_create_wrapper (display);
      struct wl_registry *registry;

      made = TRUE;
      binding.display = display;
      wl_proxy_set_queue ((struct wl_proxy *) wrapper, queue);
      registry = wl_display_get_registry (wrapper);
      wl_registry_add_listener (registry, &registry_listener, &binding);
      wl_display_roundtrip_queue (display, queue);
      wl_registry_destroy (registry);
      wl_proxy_wrapper_destroy (wrapper);
      for (guint i = 0; i < 3; i++)
        {
          struct wl_proxy *proxy = i == 0   ? (struct wl_proxy *) binding.manager
                                   : i == 1 ? (struct wl_proxy *) binding.sources
                                            : (struct wl_proxy *) binding.shm;

          if (proxy != NULL)
            wl_proxy_set_queue (proxy, NULL);
        }
      wl_event_queue_destroy (queue);
    }
  return binding.manager != NULL && binding.sources != NULL && binding.shm != NULL ? &binding
                                                                                   : NULL;
}

static gboolean
same_constraints (const Constraints *a, const Constraints *b)
{
  return a->width == b->width && a->height == b->height && a->format == b->format;
}

static void
drop_buffer (DkCapture *capture)
{
  g_clear_pointer (&capture->buffer, wl_buffer_destroy);
  if (capture->pixels != NULL)
    munmap (capture->pixels, capture->length);
  capture->pixels = NULL;
}

/* Ends the session, if it is open: Caminito forgets it as the session object goes. */
static void
close_session (DkCapture *capture)
{
  g_clear_pointer (&capture->frame, ext_image_copy_capture_frame_v1_destroy);
  g_clear_pointer (&capture->session, ext_image_copy_capture_session_v1_destroy);
  g_clear_pointer (&capture->source, ext_image_capture_source_v1_destroy);
  drop_buffer (capture);
  wl_display_flush (capture->binding->display);
}

/* Calls the caller's function. Returns FALSE where the caller freed the capture, which is
   then gone. */
static gboolean
deliver (DkCapture *capture, cairo_surface_t *frame, const GError *error)
{
  capture->delivering = TRUE;
  capture->func (capture, frame, error, capture->data);
  capture->delivering = FALSE;
  if (capture->freed)
    {
      g_free (capture);
      return FALSE;
    }
  return TRUE;
}

static void
fail (DkCapture *capture, DkCaptureError code, const char *message)
{
  g_autoptr (GError) error = g_error_new_literal (DK_CAPTURE_ERROR, code, message);

  close_session (capture);
  deliver (capture, NULL, error);
}

/* An shm buffer of the session's constraints, in a memfd the dock maps. */
static gboolean
make_buffer (DkCapture *capture)
{
  const Constraints *constraints = &capture->constraints;
  int stride = cairo_format_stride_for_width (constraints->cairo_format, constraints->width);
  gsize length = (gsize) stride * constraints->height;
  struct wl_shm_pool *pool;
  int fd;

  drop_buffer (capture);
  if (stride <= 0 || length == 0)
    return FALSE;
  fd = memfd_create ("dk-capture", MFD_CLOEXEC);
  if (fd < 0)
    return FALSE;
  if (ftruncate (fd, length) < 0)
    {
      close (fd);
      return FALSE;
    }
  capture->pixels = mmap (NULL, length, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
  if (capture->pixels == MAP_FAILED)
    {
      capture->pixels = NULL;
      close (fd);
      return FALSE;
    }
  capture->length = length;
  capture->stride = stride;
  pool = wl_shm_create_pool (capture->binding->shm, fd, length);
  capture->buffer = wl_shm_pool_create_buffer (pool, 0, constraints->width, constraints->height,
                                               stride, constraints->format);
  wl_shm_pool_destroy (pool);
  close (fd);
  capture->buffer_constraints = *constraints;
  return TRUE;
}

static const struct ext_image_copy_capture_frame_v1_listener frame_listener;

/* Asks for the session's next frame, which the compositor copies once the window has
   changed, and its first at once. */
static void
request_frame (DkCapture *capture)
{
  const Constraints *constraints = &capture->constraints;

  if (capture->buffer == NULL || !same_constraints (&capture->buffer_constraints, constraints))
    if (!make_buffer (capture))
      {
        fail (capture, DK_CAPTURE_ERROR_BUFFER, "can't make a buffer for the window's picture");
        return;
      }
  capture->frame = ext_image_copy_capture_session_v1_create_frame (capture->session);
  ext_image_copy_capture_frame_v1_add_listener (capture->frame, &frame_listener, capture);
  ext_image_copy_capture_frame_v1_attach_buffer (capture->frame, capture->buffer);
  ext_image_copy_capture_frame_v1_damage_buffer (capture->frame, 0, 0, constraints->width,
                                                 constraints->height);
  ext_image_copy_capture_frame_v1_capture (capture->frame);
}

/* The buffer's pixels in an image surface of their own, which the caller may keep while the
   next frame is copied into the buffer. Caminito's frames have the normal transform. */
static cairo_surface_t *
copy_frame (DkCapture *capture)
{
  const Constraints *constraints = &capture->buffer_constraints;
  cairo_surface_t *surface =
    cairo_image_surface_create (constraints->cairo_format, constraints->width,
                                constraints->height);
  int stride = cairo_image_surface_get_stride (surface);
  guint8 *pixels;

  if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS)
    return surface;
  cairo_surface_flush (surface);
  pixels = cairo_image_surface_get_data (surface);
  for (int y = 0; y < constraints->height; y++)
    memcpy (pixels + (gsize) y * stride, capture->pixels + (gsize) y * capture->stride,
            (gsize) MIN (stride, capture->stride));
  cairo_surface_mark_dirty (surface);
  return surface;
}

static void
on_frame_transform (void *data G_GNUC_UNUSED,
                    struct ext_image_copy_capture_frame_v1 *frame G_GNUC_UNUSED,
                    uint32_t transform G_GNUC_UNUSED)
{
}

static void
on_frame_damage (void *data G_GNUC_UNUSED, struct ext_image_copy_capture_frame_v1 *frame G_GNUC_UNUSED,
                 int32_t x G_GNUC_UNUSED, int32_t y G_GNUC_UNUSED, int32_t width G_GNUC_UNUSED,
                 int32_t height G_GNUC_UNUSED)
{
}

static void
on_frame_presentation_time (void *data G_GNUC_UNUSED,
                            struct ext_image_copy_capture_frame_v1 *frame G_GNUC_UNUSED,
                            uint32_t sec_hi G_GNUC_UNUSED, uint32_t sec_lo G_GNUC_UNUSED,
                            uint32_t nsec G_GNUC_UNUSED)
{
}

static void
on_frame_ready (void *data, struct ext_image_copy_capture_frame_v1 *frame G_GNUC_UNUSED)
{
  DkCapture *capture = data;
  cairo_surface_t *surface;
  gboolean kept;

  g_clear_pointer (&capture->frame, ext_image_copy_capture_frame_v1_destroy);
  surface = copy_frame (capture);
  if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS)
    {
      cairo_surface_destroy (surface);
      fail (capture, DK_CAPTURE_ERROR_BUFFER, "can't hold the window's picture");
      return;
    }
  if (capture->once)
    close_session (capture);
  kept = deliver (capture, surface, NULL);
  cairo_surface_destroy (surface);
  if (kept && capture->session != NULL)
    request_frame (capture);
}

static void
on_frame_failed (void *data, struct ext_image_copy_capture_frame_v1 *frame G_GNUC_UNUSED,
                 uint32_t reason)
{
  DkCapture *capture = data;

  g_clear_pointer (&capture->frame, ext_image_copy_capture_frame_v1_destroy);
  switch (reason)
    {
    case EXT_IMAGE_COPY_CAPTURE_FRAME_V1_FAILURE_REASON_BUFFER_CONSTRAINTS:
      /* The window changed size as the frame was asked for. The new constraints are here
         already, or their done event asks for the frame again. */
      if (!same_constraints (&capture->buffer_constraints, &capture->constraints))
        request_frame (capture);
      break;
    case EXT_IMAGE_COPY_CAPTURE_FRAME_V1_FAILURE_REASON_STOPPED:
      fail (capture, DK_CAPTURE_ERROR_STOPPED, "the window's capture session stopped");
      break;
    default:
      fail (capture, DK_CAPTURE_ERROR_FAILED, "can't capture the window");
      break;
    }
}

static const struct ext_image_copy_capture_frame_v1_listener frame_listener = {
  .transform = on_frame_transform,
  .damage = on_frame_damage,
  .presentation_time = on_frame_presentation_time,
  .ready = on_frame_ready,
  .failed = on_frame_failed,
};

static void
on_buffer_size (void *data, struct ext_image_copy_capture_session_v1 *session G_GNUC_UNUSED,
                uint32_t width, uint32_t height)
{
  DkCapture *capture = data;

  capture->pending.width = MIN (width, G_MAXINT);
  capture->pending.height = MIN (height, G_MAXINT);
}

/* ARGB8888 is cairo's ARGB32 and XRGB8888 its RGB24, on a little-endian machine as on a
   big-endian one: both are native-endian 32-bit words. The first is taken over the second. */
static void
on_shm_format (void *data, struct ext_image_copy_capture_session_v1 *session G_GNUC_UNUSED,
               uint32_t format)
{
  DkCapture *capture = data;

  if (format == WL_SHM_FORMAT_ARGB8888)
    {
      capture->pending.format = format;
      capture->pending.cairo_format = CAIRO_FORMAT_ARGB32;
    }
  else if (format == WL_SHM_FORMAT_XRGB8888 && capture->pending.format < 0)
    {
      capture->pending.format = format;
      capture->pending.cairo_format = CAIRO_FORMAT_RGB24;
    }
}

static void
on_dmabuf_device (void *data G_GNUC_UNUSED,
                  struct ext_image_copy_capture_session_v1 *session G_GNUC_UNUSED,
                  struct wl_array *device G_GNUC_UNUSED)
{
}

static void
on_dmabuf_format (void *data G_GNUC_UNUSED,
                  struct ext_image_copy_capture_session_v1 *session G_GNUC_UNUSED,
                  uint32_t format G_GNUC_UNUSED, struct wl_array *modifiers G_GNUC_UNUSED)
{
}

static void
on_done (void *data, struct ext_image_copy_capture_session_v1 *session G_GNUC_UNUSED)
{
  DkCapture *capture = data;

  capture->constraints = capture->pending;
  capture->pending.format = -1;
  if (capture->constraints.format < 0)
    {
      fail (capture, DK_CAPTURE_ERROR_BUFFER, "the window's picture has no shm format cairo reads");
      return;
    }
  /* The first frame, or one that failed for the constraints before these. A frame being
     copied has the next asked for once it is in. */
  if (capture->frame == NULL)
    request_frame (capture);
}

static void
on_stopped (void *data, struct ext_image_copy_capture_session_v1 *session G_GNUC_UNUSED)
{
  fail (data, DK_CAPTURE_ERROR_STOPPED, "the window's capture session stopped");
}

static const struct ext_image_copy_capture_session_v1_listener session_listener = {
  .buffer_size = on_buffer_size,
  .shm_format = on_shm_format,
  .dmabuf_device = on_dmabuf_device,
  .dmabuf_format = on_dmabuf_format,
  .done = on_done,
  .stopped = on_stopped,
};

DkCapture *
dk_capture_start (struct ext_foreign_toplevel_handle_v1 *handle, gboolean once,
                  DkCaptureFunc func, gpointer data, GError **error)
{
  GdkDisplay *display = gdk_display_get_default ();
  Binding *binding = NULL;
  DkCapture *capture;

  g_return_val_if_fail (handle != NULL && func != NULL, NULL);
  if (GDK_IS_WAYLAND_DISPLAY (display))
    binding = binding_for (gdk_wayland_display_get_wl_display (display));
  if (binding == NULL)
    {
      g_set_error_literal (error, DK_CAPTURE_ERROR, DK_CAPTURE_ERROR_UNSUPPORTED,
                           "the compositor captures no windows");
      return NULL;
    }
  capture = g_new0 (DkCapture, 1);
  capture->binding = binding;
  capture->once = once;
  capture->func = func;
  capture->data = data;
  capture->pending.format = -1;
  capture->constraints.format = -1;
  capture->source =
    ext_foreign_toplevel_image_capture_source_manager_v1_create_source (binding->sources, handle);
  capture->session =
    ext_image_copy_capture_manager_v1_create_session (binding->manager, capture->source, 0);
  ext_image_copy_capture_session_v1_add_listener (capture->session, &session_listener, capture);
  wl_display_flush (binding->display);
  return capture;
}

gboolean
dk_capture_is_open (DkCapture *capture)
{
  return capture->session != NULL;
}

void
dk_capture_free (DkCapture *capture)
{
  if (capture == NULL)
    return;
  close_session (capture);
  if (capture->delivering)
    capture->freed = TRUE;
  else
    g_free (capture);
}
#else
DkCapture *
dk_capture_start (struct ext_foreign_toplevel_handle_v1 *handle G_GNUC_UNUSED,
                  gboolean once G_GNUC_UNUSED, DkCaptureFunc func G_GNUC_UNUSED,
                  gpointer data G_GNUC_UNUSED, GError **error)
{
  g_set_error_literal (error, DK_CAPTURE_ERROR, DK_CAPTURE_ERROR_UNSUPPORTED,
                       "the compositor captures no windows");
  return NULL;
}

gboolean
dk_capture_is_open (DkCapture *capture G_GNUC_UNUSED)
{
  return FALSE;
}

void
dk_capture_free (DkCapture *capture G_GNUC_UNUSED)
{
}
#endif
