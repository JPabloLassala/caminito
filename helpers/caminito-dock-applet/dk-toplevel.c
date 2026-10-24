/*
 * A window's name for Caminito (CMN-243).
 */

#include "dk-toplevel.h"

#ifdef GDK_WINDOWING_WAYLAND
#include <gdk/gdkwayland.h>
#include <wayland-client.h>

#include "caminito-toplevel-identifier-v1-client-protocol.h"
#include "ext-foreign-toplevel-list-v1-client-protocol.h"

/* The two protocols on an event queue of the dock's own: GDK dispatches the default queue, and
   an answer to a request of ours must not wait for it or be taken by it. */
typedef struct
{
  struct wl_display                                 *display;
  struct wl_event_queue                             *queue;
  struct ext_foreign_toplevel_list_v1               *list;
  struct caminito_toplevel_identifier_manager_v1    *manager;
  /* The list's handles by identifier: identifier (owned) to ext_foreign_toplevel_handle_v1. */
  GHashTable                                        *handles;
} Binding;

/* A handle of the list, before and after its identifier event. */
typedef struct
{
  Binding                               *binding;
  struct ext_foreign_toplevel_handle_v1 *handle;
  char                                  *identifier;
} Entry;

static void
on_identifier (void *data, struct ext_foreign_toplevel_handle_v1 *handle G_GNUC_UNUSED,
               const char *identifier)
{
  Entry *entry = data;

  g_free (entry->identifier);
  entry->identifier = g_strdup (identifier);
  g_hash_table_replace (entry->binding->handles, entry->identifier, entry);
}

static void
on_closed (void *data, struct ext_foreign_toplevel_handle_v1 *handle)
{
  Entry *entry = data;

  if (entry->identifier != NULL)
    g_hash_table_remove (entry->binding->handles, entry->identifier);
  ext_foreign_toplevel_handle_v1_destroy (handle);
  g_free (entry->identifier);
  g_free (entry);
}

static void
on_ignored_text (void *data G_GNUC_UNUSED, struct ext_foreign_toplevel_handle_v1 *handle G_GNUC_UNUSED,
                 const char *text G_GNUC_UNUSED)
{
}

static void
on_done (void *data G_GNUC_UNUSED, struct ext_foreign_toplevel_handle_v1 *handle G_GNUC_UNUSED)
{
}

static const struct ext_foreign_toplevel_handle_v1_listener handle_listener = {
  .closed = on_closed,
  .done = on_done,
  .title = on_ignored_text,
  .app_id = on_ignored_text,
  .identifier = on_identifier,
};

static void
on_toplevel (void *data, struct ext_foreign_toplevel_list_v1 *list G_GNUC_UNUSED,
             struct ext_foreign_toplevel_handle_v1 *handle)
{
  Entry *entry = g_new0 (Entry, 1);

  entry->binding = data;
  entry->handle = handle;
  ext_foreign_toplevel_handle_v1_add_listener (handle, &handle_listener, entry);
}

static void
on_finished (void *data G_GNUC_UNUSED, struct ext_foreign_toplevel_list_v1 *list G_GNUC_UNUSED)
{
}

static const struct ext_foreign_toplevel_list_v1_listener list_listener = {
  .toplevel = on_toplevel,
  .finished = on_finished,
};

static void
on_global (void *data, struct wl_registry *registry, uint32_t name, const char *interface,
           uint32_t version G_GNUC_UNUSED)
{
  Binding *binding = data;

  if (g_str_equal (interface, ext_foreign_toplevel_list_v1_interface.name))
    {
      binding->list = wl_registry_bind (registry, name, &ext_foreign_toplevel_list_v1_interface, 1);
      /* Before the roundtrip that follows dispatches the handles the compositor sends at once:
         a handle that arrives with no listener is never heard from again. */
      ext_foreign_toplevel_list_v1_add_listener (binding->list, &list_listener, binding);
    }
  else if (g_str_equal (interface, caminito_toplevel_identifier_manager_v1_interface.name))
    binding->manager = wl_registry_bind (registry, name,
                                         &caminito_toplevel_identifier_manager_v1_interface, 1);
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
   compositor lacks either protocol. */
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
      binding.handles = g_hash_table_new (g_str_hash, g_str_equal);
      wl_proxy_set_queue ((struct wl_proxy *) wrapper, binding.queue);
      registry = wl_display_get_registry (wrapper);
      wl_registry_add_listener (registry, &registry_listener, &binding);
      wl_display_roundtrip_queue (display, binding.queue);
      wl_registry_destroy (registry);
      wl_proxy_wrapper_destroy (wrapper);
    }
  return binding.list != NULL && binding.manager != NULL ? &binding : NULL;
}

/* The wlr handle of an XfwWindow, which is a property of it: "wlr-handle" since
   libxfce4windowing 4.20.7 and "handle" before. It isn't in an installed header. NULL for a
   window with neither (an X11 one). */
static gpointer
wlr_handle_of (GObject *window)
{
  static const char *const names[] = { "wlr-handle", "handle" };
  GObjectClass *klass = G_OBJECT_GET_CLASS (window);

  for (guint i = 0; i < G_N_ELEMENTS (names); i++)
    {
      GParamSpec *spec = g_object_class_find_property (klass, names[i]);
      gpointer handle = NULL;

      if (spec == NULL || spec->value_type != G_TYPE_POINTER)
        continue;
      g_object_get (window, names[i], &handle, NULL);
      return handle;
    }
  return NULL;
}

static void
on_answer_identifier (void *data, struct caminito_toplevel_identifier_v1 *object G_GNUC_UNUSED,
                      const char *identifier)
{
  char **result = data;

  g_free (*result);
  *result = g_strdup (identifier);
}

static void
on_answer_gone (void *data G_GNUC_UNUSED, struct caminito_toplevel_identifier_v1 *object G_GNUC_UNUSED)
{
}

static const struct caminito_toplevel_identifier_v1_listener answer_listener = {
  .identifier = on_answer_identifier,
  .gone = on_answer_gone,
};

gboolean
dk_toplevel_lookup (GObject *window, char **identifier,
                    struct ext_foreign_toplevel_handle_v1 **handle)
{
  GdkDisplay *display = gdk_display_get_default ();
  struct caminito_toplevel_identifier_v1 *request;
  struct zwlr_foreign_toplevel_handle_v1 *wlr_handle;
  Binding *binding;
  char *name = NULL;
  Entry *entry;

  if (!GDK_IS_WAYLAND_DISPLAY (display) || window == NULL)
    return FALSE;
  wlr_handle = wlr_handle_of (window);
  if (wlr_handle == NULL)
    return FALSE;
  binding = binding_for (gdk_wayland_display_get_wl_display (display));
  if (binding == NULL)
    return FALSE;
  request = caminito_toplevel_identifier_manager_v1_get_identifier (binding->manager, wlr_handle);
  caminito_toplevel_identifier_v1_add_listener (request, &answer_listener, &name);
  /* The compositor answers before it answers the round trip, and it sent the list's handle
     for a window before it sent the window to the panel's library, so this reads both. */
  wl_display_roundtrip_queue (binding->display, binding->queue);
  caminito_toplevel_identifier_v1_destroy (request);
  if (name == NULL)
    return FALSE;
  entry = g_hash_table_lookup (binding->handles, name);
  if (entry == NULL)
    {
      g_free (name);
      return FALSE;
    }
  if (handle != NULL)
    *handle = entry->handle;
  if (identifier != NULL)
    *identifier = name;
  else
    g_free (name);
  return TRUE;
}
#else
gboolean
dk_toplevel_lookup (GObject *window G_GNUC_UNUSED, char **identifier G_GNUC_UNUSED,
                    struct ext_foreign_toplevel_handle_v1 **handle G_GNUC_UNUSED)
{
  return FALSE;
}
#endif
