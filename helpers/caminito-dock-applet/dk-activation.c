/*
 * An xdg-activation token for a launch from the dock (CMN-221).
 */

#include "dk-activation.h"

#ifdef GDK_WINDOWING_WAYLAND
#include <gdk/gdkwayland.h>
#include <wayland-client.h>

#include "xdg-activation-v1-client-protocol.h"

/* The compositor's xdg_activation_v1, on an event queue of the dock's own: GDK dispatches the
   default queue, and an answer to a request of ours must not wait for it or be taken by it. */
typedef struct
{
  struct wl_display          *display;
  struct wl_event_queue      *queue;
  struct xdg_activation_v1   *activation;
} Binding;

static void
on_global (void *data, struct wl_registry *registry, uint32_t name, const char *interface,
           uint32_t version G_GNUC_UNUSED)
{
  Binding *binding = data;

  if (g_str_equal (interface, xdg_activation_v1_interface.name))
    binding->activation = wl_registry_bind (registry, name, &xdg_activation_v1_interface, 1);
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

/* The binding for GDK's display, made once: a process has one display. */
static Binding *
binding_for (struct wl_display *display)
{
  static Binding binding;
  static gboolean made;

  if (!made)
    {
      struct wl_display *wrapper = wl_proxy_create_wrapper (display);
      struct wl_registry *registry;

      made = TRUE;
      binding.display = display;
      binding.queue = wl_display_create_queue (display);
      wl_proxy_set_queue ((struct wl_proxy *) wrapper, binding.queue);
      registry = wl_display_get_registry (wrapper);
      wl_registry_add_listener (registry, &registry_listener, &binding);
      wl_display_roundtrip_queue (display, binding.queue);
      wl_registry_destroy (registry);
      wl_proxy_wrapper_destroy (wrapper);
    }
  return binding.activation != NULL ? &binding : NULL;
}

static void
on_done (void *data, struct xdg_activation_token_v1 *token G_GNUC_UNUSED, const char *string)
{
  char **result = data;

  g_free (*result);
  *result = g_strdup (string);
}

static const struct xdg_activation_token_v1_listener token_listener = {
  .done = on_done,
};

char *
dk_activation_token_new (GdkWindow *window, const char *app_id)
{
  GdkDisplay *display = gdk_display_get_default ();
  struct xdg_activation_token_v1 *request;
  Binding *binding;
  char *token = NULL;

  if (!GDK_IS_WAYLAND_DISPLAY (display))
    return NULL;
  binding = binding_for (gdk_wayland_display_get_wl_display (display));
  if (binding == NULL)
    return NULL;
  request = xdg_activation_v1_get_activation_token (binding->activation);
  xdg_activation_token_v1_add_listener (request, &token_listener, &token);
  if (app_id != NULL)
    xdg_activation_token_v1_set_app_id (request, app_id);
  if (window != NULL && GDK_IS_WAYLAND_WINDOW (window))
    xdg_activation_token_v1_set_surface (request, gdk_wayland_window_get_wl_surface (window));
  xdg_activation_token_v1_commit (request);
  /* The compositor answers a commit before it answers the round trip that follows it. */
  wl_display_roundtrip_queue (binding->display, binding->queue);
  xdg_activation_token_v1_destroy (request);
  return token;
}
#else
char *
dk_activation_token_new (GdkWindow *window G_GNUC_UNUSED, const char *app_id G_GNUC_UNUSED)
{
  return NULL;
}
#endif
