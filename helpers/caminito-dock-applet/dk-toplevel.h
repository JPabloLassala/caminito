/*
 * A window's name for Caminito (CMN-243, ADR 0049). libxfce4windowing gives the dock a
 * window's zwlr_foreign_toplevel_handle_v1 and nothing that names it to image capture or to
 * org.mate.Caminito. Caminito's caminito-toplevel-identifier-v1 answers the handle with the
 * identifier of ext-foreign-toplevel-list, which the dock binds on GDK's own connection to
 * find the window's ext_foreign_toplevel_handle_v1.
 */

#pragma once

#include <glib-object.h>

struct ext_foreign_toplevel_handle_v1;

/* The identifier of a window and its ext handle, for window, an XfwWindow, which only this
   function knows to be a GObject with a Wayland handle. Returns FALSE with both left alone
   where the display isn't Wayland's, the compositor has no caminito-toplevel-identifier-v1
   (not Caminito) or no ext-foreign-toplevel-list, the window has no handle (X11, or a
   libxfce4windowing without the property), or the window is gone: the caller falls back to
   what it did without a name.

   The identifier is the caller's to free with g_free; it may be NULL for a caller that wants
   only the handle. The handle is the dock's own, valid until the next call, which may dispatch
   the list's events and so close it. Either out parameter may be NULL. */
gboolean dk_toplevel_lookup (GObject *window, char **identifier,
                             struct ext_foreign_toplevel_handle_v1 **handle);
