/* SPDX-FileCopyrightText: 2026 Juan Pablo Lassala
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * A lock window's sharp wallpaper, in a subsurface under the window, and the window's asking for
 * Esmeril over it (CMN-301, ADR 0056). GTK 3 has no API for either.
 */

#ifndef __GS_BACKDROP_H
#define __GS_BACKDROP_H

#include <gtk/gtk.h>

typedef struct GSBackdrop GSBackdrop;

/* Finds what the backdrops need on `display`, waiting for Caminito's answer. It's called before
 * the lock is asked for, as the wait lets what's asked so far reach Caminito: a lock with no lock
 * surface yet would lose the keys pressed as it starts. */
void gs_backdrop_start(GdkDisplay* display);

/* Calls `changed` when Caminito starts or stops offering blur, which follows its settings. */
void gs_backdrop_follow_can_blur(void (*changed)(void));

/* Whether Caminito offers blur now. */
gboolean gs_backdrop_can_blur(void);

/* The backdrop of `window`, whose wl_surface exists, or NULL on a compositor that doesn't serve
 * what it needs. It has to be freed before the window's unrealized. */
GSBackdrop* gs_backdrop_new(GtkWidget* window);
void gs_backdrop_free(GSBackdrop* backdrop);

/* Shows `picture` under the window at the window's next commit, on a buffer of `width` by
 * `height` at `scale`, and returns a surface of the same pixels, which the window can paint and
 * which lasts until the next call or the backdrop's end. NULL if it can't be shown. */
cairo_surface_t* gs_backdrop_set_picture(GSBackdrop* backdrop, cairo_surface_t* picture, int width, int height, int scale);

/* Whether the window asks for Esmeril, from its next commit. */
void gs_backdrop_set_blurred(GSBackdrop* backdrop, gboolean blurred);

#endif /* __GS_BACKDROP_H */
