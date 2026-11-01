/* SPDX-FileCopyrightText: 2026 Juan Pablo Lassala
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * A lock window's sharp wallpaper, in a subsurface under the window, and the window's asking for
 * Esmeril over it (CMN-301, ADR 0056). The subsurface stays synchronized: GTK's next commit of
 * the window shows its picture and the window's own pixels of the same size together.
 */

#define _GNU_SOURCE

#include "config.h"

#include "gs-backdrop.h"

#include <errno.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#include <gdk/gdkwayland.h>
#include <wayland-client.h>

#include "ext-background-effect-v1-client-protocol.h"

/* A picture's pixels, shared with Caminito. */
typedef struct
{
	struct wl_buffer *buffer;
	void             *data;
	gsize             size;
	cairo_surface_t  *surface;
	GSBackdrop       *backdrop;
	/* Whether Caminito is done with it, and whether the backdrop is. It goes with both. */
	gboolean          released;
	gboolean          retired;
} Picture;

struct GSBackdrop
{
	struct wl_surface                       *window_surface;
	struct wl_surface                       *surface;
	struct wl_subsurface                    *subsurface;
	struct ext_background_effect_surface_v1 *effect;
	Picture                                 *picture;
	/* What Caminito hasn't released yet of the pictures before this one. */
	GList                                   *retired;
	int                                      width;
	int                                      height;
	gboolean                                 blurred;
};

static struct wl_shm *shm = NULL;
static struct wl_subcompositor *subcompositor = NULL;
static struct ext_background_effect_manager_v1 *manager = NULL;
static gboolean can_blur = FALSE;
static void (*on_can_blur_changed)(void) = NULL;

static void manager_capabilities(void* data G_GNUC_UNUSED, struct ext_background_effect_manager_v1* source G_GNUC_UNUSED, uint32_t flags)
{
	gboolean now = (flags & EXT_BACKGROUND_EFFECT_MANAGER_V1_CAPABILITY_BLUR) != 0;

	if (now != can_blur)
	{
		g_debug("Caminito %s blur", now ? "offers" : "doesn't offer");
		can_blur = now;
		if (on_can_blur_changed != NULL)
		{
			on_can_blur_changed();
		}
	}
}

static const struct ext_background_effect_manager_v1_listener manager_listener = {
	.capabilities = manager_capabilities,
};

static void registry_global(void* data G_GNUC_UNUSED, struct wl_registry* registry, uint32_t name, const char* interface, uint32_t version G_GNUC_UNUSED)
{
	if (strcmp(interface, wl_shm_interface.name) == 0)
	{
		shm = wl_registry_bind(registry, name, &wl_shm_interface, 1);
	}
	else if (strcmp(interface, wl_subcompositor_interface.name) == 0)
	{
		subcompositor = wl_registry_bind(registry, name, &wl_subcompositor_interface, 1);
	}
	else if (strcmp(interface, ext_background_effect_manager_v1_interface.name) == 0)
	{
		manager = wl_registry_bind(registry, name, &ext_background_effect_manager_v1_interface, 1);
		ext_background_effect_manager_v1_add_listener(manager, &manager_listener, NULL);
	}
}

static void registry_global_remove(void* data G_GNUC_UNUSED, struct wl_registry* registry G_GNUC_UNUSED, uint32_t name G_GNUC_UNUSED)
{
}

static const struct wl_registry_listener registry_listener = {
	.global = registry_global,
	.global_remove = registry_global_remove,
};

void gs_backdrop_follow_can_blur(void (*changed)(void))
{
	on_can_blur_changed = changed;
}

void gs_backdrop_start(GdkDisplay* display)
{
	struct wl_display* wl_display;
	struct wl_display* wrapper;
	struct wl_event_queue* queue;
	struct wl_registry* registry;

	if (!GDK_IS_WAYLAND_DISPLAY(display))
	{
		return;
	}

	wl_display = gdk_wayland_display_get_wl_display(display);
	/* On a queue of its own, so that none of GDK's events come early. Once for the globals, and
	 * once for the offer. */
	queue = wl_display_create_queue(wl_display);
	wrapper = wl_proxy_create_wrapper(wl_display);
	wl_proxy_set_queue((struct wl_proxy*) wrapper, queue);
	registry = wl_display_get_registry(wrapper);
	wl_proxy_wrapper_destroy(wrapper);
	wl_registry_add_listener(registry, &registry_listener, NULL);
	wl_display_roundtrip_queue(wl_display, queue);
	wl_display_roundtrip_queue(wl_display, queue);
	wl_registry_destroy(registry);
	if (manager != NULL)
	{
		wl_proxy_set_queue((struct wl_proxy*) manager, NULL);
	}
	if (shm != NULL)
	{
		wl_proxy_set_queue((struct wl_proxy*) shm, NULL);
	}
	if (subcompositor != NULL)
	{
		wl_proxy_set_queue((struct wl_proxy*) subcompositor, NULL);
	}
	wl_event_queue_destroy(queue);
}

gboolean gs_backdrop_can_blur(void)
{
	return can_blur;
}

GSBackdrop* gs_backdrop_new(GtkWidget* window)
{
	GdkWindow* gdk_window = gtk_widget_get_window(window);
	struct wl_compositor* compositor = gdk_wayland_display_get_wl_compositor(gdk_window_get_display(gdk_window));
	struct wl_region* input;
	GSBackdrop* backdrop;

	if (shm == NULL || subcompositor == NULL || manager == NULL)
	{
		return NULL;
	}

	backdrop = g_new0(GSBackdrop, 1);
	backdrop->window_surface = gdk_wayland_window_get_wl_surface(gdk_window);
	backdrop->surface = wl_compositor_create_surface(compositor);
	backdrop->subsurface = wl_subcompositor_get_subsurface(subcompositor, backdrop->surface, backdrop->window_surface);
	wl_subsurface_place_below(backdrop->subsurface, backdrop->window_surface);
	/* The window takes the pointer: GDK knows nothing of this surface. */
	input = wl_compositor_create_region(compositor);
	wl_surface_set_input_region(backdrop->surface, input);
	wl_region_destroy(input);
	backdrop->effect = ext_background_effect_manager_v1_get_background_effect(manager, backdrop->window_surface);

	return backdrop;
}

static void picture_free(Picture* picture)
{
	cairo_surface_destroy(picture->surface);
	wl_buffer_destroy(picture->buffer);
	munmap(picture->data, picture->size);
	g_free(picture);
}

static void buffer_release(void* data, struct wl_buffer* buffer G_GNUC_UNUSED)
{
	Picture* picture = data;

	picture->released = TRUE;
	if (picture->retired)
	{
		picture->backdrop->retired = g_list_remove(picture->backdrop->retired, picture);
		picture_free(picture);
	}
}

static const struct wl_buffer_listener buffer_listener = {
	.release = buffer_release,
};

/* A new picture of `width` by `height` buffer pixels, or NULL. */
static Picture* picture_new(GSBackdrop* backdrop, int width, int height, int scale)
{
	Picture* picture;
	struct wl_shm_pool* pool;
	int stride = cairo_format_stride_for_width(CAIRO_FORMAT_RGB24, width);
	gsize size = (gsize) stride * height;
	void* data;
	int fd;

	fd = memfd_create("caminito-locker-wallpaper", MFD_CLOEXEC);
	if (fd < 0 || ftruncate(fd, size) < 0)
	{
		g_warning("can't make a wallpaper's buffer: %s", g_strerror(errno));
		if (fd >= 0)
		{
			close(fd);
		}
		return NULL;
	}

	data = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
	if (data == MAP_FAILED)
	{
		g_warning("can't map a wallpaper's buffer: %s", g_strerror(errno));
		close(fd);
		return NULL;
	}

	picture = g_new0(Picture, 1);
	picture->backdrop = backdrop;
	picture->data = data;
	picture->size = size;
	pool = wl_shm_create_pool(shm, fd, size);
	picture->buffer = wl_shm_pool_create_buffer(pool, 0, width, height, stride, WL_SHM_FORMAT_XRGB8888);
	wl_shm_pool_destroy(pool);
	close(fd);
	wl_buffer_add_listener(picture->buffer, &buffer_listener, picture);
	picture->surface = cairo_image_surface_create_for_data(data, CAIRO_FORMAT_RGB24, width, height, stride);
	cairo_surface_set_device_scale(picture->surface, scale, scale);

	return picture;
}

cairo_surface_t* gs_backdrop_set_picture(GSBackdrop* backdrop, cairo_surface_t* source, int width, int height, int scale)
{
	Picture* picture = picture_new(backdrop, width * scale, height * scale, scale);
	cairo_t* cr;

	if (picture == NULL)
	{
		return NULL;
	}

	cr = cairo_create(picture->surface);
	cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
	if (source != NULL)
	{
		cairo_set_source_surface(cr, source, 0, 0);
		/* MateBG makes a solid colour's surface 1x1. */
		cairo_pattern_set_extend(cairo_get_source(cr), CAIRO_EXTEND_REPEAT);
	}
	else
	{
		cairo_set_source_rgb(cr, 0, 0, 0);
	}
	cairo_paint(cr);
	cairo_destroy(cr);
	cairo_surface_flush(picture->surface);

	wl_surface_set_buffer_scale(backdrop->surface, scale);
	wl_surface_attach(backdrop->surface, picture->buffer, 0, 0);
	/* GDK binds the compositor at version 3, which has no damage_buffer. */
	wl_surface_damage(backdrop->surface, 0, 0, width, height);
	wl_surface_commit(backdrop->surface);

	if (backdrop->picture != NULL)
	{
		Picture* old = backdrop->picture;

		old->retired = TRUE;
		if (old->released)
		{
			picture_free(old);
		}
		else
		{
			backdrop->retired = g_list_prepend(backdrop->retired, old);
		}
	}
	backdrop->picture = picture;
	backdrop->width = width;
	backdrop->height = height;
	/* It asks again for the new size. */
	if (backdrop->blurred)
	{
		backdrop->blurred = FALSE;
		gs_backdrop_set_blurred(backdrop, TRUE);
	}

	return picture->surface;
}

void gs_backdrop_set_blurred(GSBackdrop* backdrop, gboolean blurred)
{
	struct wl_region* region = NULL;

	if (blurred == backdrop->blurred)
	{
		return;
	}
	backdrop->blurred = blurred;

	if (blurred)
	{
		GdkDisplay* display = gdk_display_get_default();

		region = wl_compositor_create_region(gdk_wayland_display_get_wl_compositor(display));
		wl_region_add(region, 0, 0, backdrop->width, backdrop->height);
	}
	ext_background_effect_surface_v1_set_blur_region(backdrop->effect, region);
	if (region != NULL)
	{
		wl_region_destroy(region);
	}
}

void gs_backdrop_free(GSBackdrop* backdrop)
{
	g_list_free_full(backdrop->retired, (GDestroyNotify) picture_free);
	ext_background_effect_surface_v1_destroy(backdrop->effect);
	wl_subsurface_destroy(backdrop->subsurface);
	wl_surface_destroy(backdrop->surface);
	if (backdrop->picture != NULL)
	{
		picture_free(backdrop->picture);
	}
	g_free(backdrop);
}
