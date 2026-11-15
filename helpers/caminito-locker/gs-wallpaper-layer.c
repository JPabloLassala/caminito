/* SPDX-FileCopyrightText: 2026 Juan Pablo Lassala
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * A lock window's wallpaper, in two subsurfaces under the window (CMN-301, ADR 0056): the sharp
 * one at the bottom, which Esmeril reads, and over it a copy of it that asks for Esmeril and
 * shows through it by the alpha modifier's factor, which is the fade. The compositor does the
 * blending, so a frame of the fade is a commit of the copy and no repaint of anything. The two
 * stay synchronized for a new picture: GTK's next commit of the window shows the pictures and the
 * window's own pixels of the same size together. A fade's commits are the only ones that aren't.
 */

#define _GNU_SOURCE

#include "config.h"

#include "gs-wallpaper-layer.h"

#include <errno.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#include <gdk/gdkwayland.h>
#include <wayland-client.h>

#include "alpha-modifier-v1-client-protocol.h"
#include "ext-background-effect-v1-client-protocol.h"

/* A picture's pixels, shared with Caminito, in a buffer for each of the layer's surfaces. */
typedef struct
{
	struct wl_buffer *below;
	struct wl_buffer *front;
	void             *data;
	gsize             size;
	GSWallpaperLayer *layer;
	/* How many of its buffers Caminito hasn't released yet, and whether the layer is done with
	 * it. It goes with both. */
	int               held;
	gboolean          retired;
} Picture;

struct GSWallpaperLayer
{
	struct wl_surface                       *window_surface;
	/* The sharp wallpaper, which Esmeril reads. */
	struct wl_surface                       *below;
	struct wl_subsurface                    *below_subsurface;
	/* The copy over it, which asks for Esmeril and fades. */
	struct wl_surface                       *front;
	struct wl_subsurface                    *front_subsurface;
	struct ext_background_effect_surface_v1 *effect;
	struct wp_alpha_modifier_surface_v1     *alpha;
	Picture                                 *picture;
	/* What Caminito hasn't released yet of the pictures before this one. */
	GList                                   *retired;
	/* The frame callback a fade waits for, and what it calls. */
	struct wl_callback                      *frame;
	GSWallpaperLayerFrame                    on_frame;
	gpointer                                 on_frame_data;
	int                                      width;
	int                                      height;
	gboolean                                 blurred;
};

static struct wl_shm *shm = NULL;
static struct wl_subcompositor *subcompositor = NULL;
static struct wp_alpha_modifier_v1 *alpha_manager = NULL;
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
	else if (strcmp(interface, wp_alpha_modifier_v1_interface.name) == 0)
	{
		alpha_manager = wl_registry_bind(registry, name, &wp_alpha_modifier_v1_interface, 1);
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

void gs_wallpaper_layer_follow_can_blur(void (*changed)(void))
{
	on_can_blur_changed = changed;
}

void gs_wallpaper_layer_start(GdkDisplay* display)
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
	if (alpha_manager != NULL)
	{
		wl_proxy_set_queue((struct wl_proxy*) alpha_manager, NULL);
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

gboolean gs_wallpaper_layer_can_blur(void)
{
	return can_blur;
}

/* A surface of the layer under `parent`, in front of `sibling`'s place, with no input. */
static struct wl_subsurface* new_subsurface(struct wl_compositor* compositor, struct wl_surface* surface, struct wl_surface* parent, struct wl_surface* sibling)
{
	struct wl_subsurface* subsurface = wl_subcompositor_get_subsurface(subcompositor, surface, parent);
	struct wl_region* input;

	wl_subsurface_place_below(subsurface, sibling);
	/* The window takes the pointer: GDK knows nothing of this surface. */
	input = wl_compositor_create_region(compositor);
	wl_surface_set_input_region(surface, input);
	wl_region_destroy(input);

	return subsurface;
}

GSWallpaperLayer* gs_wallpaper_layer_new(GtkWidget* window)
{
	GdkWindow* gdk_window = gtk_widget_get_window(window);
	struct wl_compositor* compositor = gdk_wayland_display_get_wl_compositor(gdk_window_get_display(gdk_window));
	GSWallpaperLayer* layer;

	if (shm == NULL || subcompositor == NULL || manager == NULL || alpha_manager == NULL)
	{
		return NULL;
	}

	layer = g_new0(GSWallpaperLayer, 1);
	layer->window_surface = gdk_wayland_window_get_wl_surface(gdk_window);
	layer->front = wl_compositor_create_surface(compositor);
	layer->front_subsurface = new_subsurface(compositor, layer->front, layer->window_surface, layer->window_surface);
	layer->below = wl_compositor_create_surface(compositor);
	layer->below_subsurface = new_subsurface(compositor, layer->below, layer->window_surface, layer->front);
	layer->effect = ext_background_effect_manager_v1_get_background_effect(manager, layer->front);
	layer->alpha = wp_alpha_modifier_v1_get_surface(alpha_manager, layer->front);

	return layer;
}

static void picture_free(Picture* picture)
{
	wl_buffer_destroy(picture->below);
	wl_buffer_destroy(picture->front);
	munmap(picture->data, picture->size);
	g_free(picture);
}

static void buffer_release(void* data, struct wl_buffer* buffer G_GNUC_UNUSED)
{
	Picture* picture = data;

	picture->held--;
	if (picture->retired && picture->held <= 0)
	{
		picture->layer->retired = g_list_remove(picture->layer->retired, picture);
		picture_free(picture);
	}
}

static const struct wl_buffer_listener buffer_listener = {
	.release = buffer_release,
};

/* A new picture of `width` by `height` buffer pixels, or NULL. */
static Picture* picture_new(GSWallpaperLayer* layer, int width, int height)
{
	Picture* picture;
	struct wl_shm_pool* pool;
	int stride = cairo_format_stride_for_width(CAIRO_FORMAT_ARGB32, width);
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
	picture->layer = layer;
	picture->data = data;
	picture->size = size;
	picture->held = 2;
	/* Two buffers of the same pixels, one for each surface. The copy's has an alpha channel,
	 * all opaque, though it needs none: Caminito's software renderer draws a picture without
	 * one by replacing what's under it, whatever its factor. */
	pool = wl_shm_create_pool(shm, fd, size);
	picture->below = wl_shm_pool_create_buffer(pool, 0, width, height, stride, WL_SHM_FORMAT_XRGB8888);
	picture->front = wl_shm_pool_create_buffer(pool, 0, width, height, stride, WL_SHM_FORMAT_ARGB8888);
	wl_shm_pool_destroy(pool);
	close(fd);
	wl_buffer_add_listener(picture->below, &buffer_listener, picture);
	wl_buffer_add_listener(picture->front, &buffer_listener, picture);

	return picture;
}

/* Shows `buffer` on `surface`, from the next commit of the window. */
static void show_buffer(struct wl_surface* surface, struct wl_buffer* buffer, int width, int height, int scale)
{
	wl_surface_set_buffer_scale(surface, scale);
	wl_surface_attach(surface, buffer, 0, 0);
	/* GDK binds the compositor at version 3, which has no damage_buffer. */
	wl_surface_damage(surface, 0, 0, width, height);
}

gboolean gs_wallpaper_layer_set_picture(GSWallpaperLayer* layer, cairo_surface_t* source, int width, int height, int scale)
{
	Picture* picture = picture_new(layer, width * scale, height * scale);
	cairo_surface_t* surface;
	cairo_t* cr;

	if (picture == NULL)
	{
		return FALSE;
	}

	surface = cairo_image_surface_create_for_data(picture->data, CAIRO_FORMAT_ARGB32, width * scale, height * scale, cairo_format_stride_for_width(CAIRO_FORMAT_ARGB32, width * scale));
	cairo_surface_set_device_scale(surface, scale, scale);
	cr = cairo_create(surface);
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
	cairo_surface_flush(surface);
	cairo_surface_destroy(surface);

	if (layer->picture != NULL)
	{
		Picture* old = layer->picture;

		old->retired = TRUE;
		if (old->held <= 0)
		{
			picture_free(old);
		}
		else
		{
			layer->retired = g_list_prepend(layer->retired, old);
		}
	}
	layer->picture = picture;
	layer->width = width;
	layer->height = height;

	show_buffer(layer->below, picture->below, width, height, scale);
	show_buffer(layer->front, picture->front, width, height, scale);
	/* The region asks again for the new size. */
	if (layer->blurred)
	{
		layer->blurred = FALSE;
		gs_wallpaper_layer_set_blurred(layer, TRUE);
	}
	wl_surface_commit(layer->below);
	wl_surface_commit(layer->front);

	return TRUE;
}

void gs_wallpaper_layer_set_blurred(GSWallpaperLayer* layer, gboolean blurred)
{
	struct wl_region* region = NULL;

	if (blurred == layer->blurred)
	{
		return;
	}
	layer->blurred = blurred;

	if (blurred)
	{
		GdkDisplay* display = gdk_display_get_default();

		region = wl_compositor_create_region(gdk_wayland_display_get_wl_compositor(display));
		wl_region_add(region, 0, 0, layer->width, layer->height);
	}
	ext_background_effect_surface_v1_set_blur_region(layer->effect, region);
	if (region != NULL)
	{
		wl_region_destroy(region);
	}
}

static void frame_done(void* data, struct wl_callback* callback, uint32_t time G_GNUC_UNUSED)
{
	GSWallpaperLayer* layer = data;
	GSWallpaperLayerFrame on_frame = layer->on_frame;

	wl_callback_destroy(callback);
	layer->frame = NULL;
	if (on_frame != NULL)
	{
		on_frame(layer, layer->on_frame_data);
	}
}

static const struct wl_callback_listener frame_listener = {
	.done = frame_done,
};

void gs_wallpaper_layer_show(GSWallpaperLayer* layer, double amount, GSWallpaperLayerFrame on_frame, gpointer data)
{
	uint32_t factor = (uint32_t) (CLAMP(1.0 - amount, 0.0, 1.0) * (double) UINT32_MAX + 0.5);

	if (layer->frame != NULL)
	{
		wl_callback_destroy(layer->frame);
		layer->frame = NULL;
	}
	wp_alpha_modifier_surface_v1_set_multiplier(layer->alpha, factor);
	layer->on_frame = on_frame;
	layer->on_frame_data = data;
	if (on_frame != NULL)
	{
		layer->frame = wl_surface_frame(layer->front);
		wl_callback_add_listener(layer->frame, &frame_listener, layer);
	}
	/* Free of the window: it has nothing to draw during a fade, so no commit of it would come. */
	wl_subsurface_set_desync(layer->front_subsurface);
	wl_surface_commit(layer->front);
	wl_subsurface_set_sync(layer->front_subsurface);
}

void gs_wallpaper_layer_free(GSWallpaperLayer* layer)
{
	g_list_free_full(layer->retired, (GDestroyNotify) picture_free);
	if (layer->frame != NULL)
	{
		wl_callback_destroy(layer->frame);
	}
	wp_alpha_modifier_surface_v1_destroy(layer->alpha);
	ext_background_effect_surface_v1_destroy(layer->effect);
	wl_subsurface_destroy(layer->below_subsurface);
	wl_surface_destroy(layer->below);
	wl_subsurface_destroy(layer->front_subsurface);
	wl_surface_destroy(layer->front);
	if (layer->picture != NULL)
	{
		picture_free(layer->picture);
	}
	g_free(layer);
}
