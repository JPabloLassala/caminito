/* SPDX-FileCopyrightText: 2026 Juan Pablo Lassala
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * A lock window's wallpaper, in two subsurfaces under the window: the sharp one, and a copy of it
 * over that, which asks for Esmeril and fades by the alpha modifier (CMN-301, ADR 0056). GTK 3 has
 * no API for either.
 */

#ifndef __GS_WALLPAPER_LAYER_H
#define __GS_WALLPAPER_LAYER_H

#include <gtk/gtk.h>

typedef struct GSWallpaperLayer GSWallpaperLayer;

/* Finds what the layers need on `display`, waiting for Caminito's answer. It's called before
 * the lock is asked for, as the wait lets what's asked so far reach Caminito: a lock with no lock
 * surface yet would lose the keys pressed as it starts. */
void gs_wallpaper_layer_start(GdkDisplay* display);

/* Calls `changed` when Caminito starts or stops offering blur, which follows its settings. */
void gs_wallpaper_layer_follow_can_blur(void (*changed)(void));

/* Whether Caminito offers blur now. */
gboolean gs_wallpaper_layer_can_blur(void);

/* The layer of `window`, whose wl_surface exists, or NULL on a compositor that doesn't serve
 * what it needs. It has to be freed before the window's unrealized. */
GSWallpaperLayer* gs_wallpaper_layer_new(GtkWidget* window);
void gs_wallpaper_layer_free(GSWallpaperLayer* layer);

typedef void (*GSWallpaperLayerFrame)(GSWallpaperLayer* layer, gpointer data);

/* Shows `picture` under the window at the window's next commit, on a buffer of `width` by
 * `height` at `scale`. FALSE if it can't be shown. */
gboolean gs_wallpaper_layer_set_picture(GSWallpaperLayer* layer, cairo_surface_t* picture, int width, int height, int scale);

/* Whether the copy asks for Esmeril, from the next `gs_wallpaper_layer_show`. */
void gs_wallpaper_layer_set_blurred(GSWallpaperLayer* layer, gboolean blurred);

/* Shows `amount` of Esmeril now, from none to all of it: the copy shows through that much less.
 * If `on_frame` isn't NULL it's called when the compositor has shown the result, for the next
 * step of a fade. */
void gs_wallpaper_layer_show(GSWallpaperLayer* layer, double amount, GSWallpaperLayerFrame on_frame, gpointer data);

#endif /* __GS_WALLPAPER_LAYER_H */
