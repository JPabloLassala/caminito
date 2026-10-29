# 49. The Dock applet names a window to Caminito by its ext identifier

Date: 2026-10-24
Status: Superseded on 2026-10-25: the Dock applet became the Icon Task Manager, and the Dock preview, the Minimised window item's picture and the Application overview for a Dock item, the three features this protocol served, were cut ("Spec: Icon Task Manager"). Its code is removed.

## Context

The Dock applet knows its windows through libxfce4windowing, which on Wayland uses wlr-foreign-toplevel-management and Xfce's private extension only. Three of its features must name a window to Caminito: the Dock preview's live thumbnails (CMN-225) and a Minimised window item's picture (CMN-242), which capture through `ext-foreign-toplevel-image-capture-source` and so need the window's `ext_foreign_toplevel_handle_v1`, and the Application overview for a Dock item's windows (CMN-222). libxfce4windowing gives no window id on Wayland (`xfw_window_get_id` was removed because it was always zero there), has no ext-foreign-toplevel-list support upstream, and binds Xfce's extension at version 1, so a new event there wouldn't reach it. A match of the ext list's handles to the XfwWindows by app id and title fails for two windows with the same title.

Each `XfwWindow` on Wayland has its `zwlr_foreign_toplevel_handle_v1` as a readable GObject property, `"wlr-handle"` from 4.20.7 and `"handle"` before. It isn't in an installed header, but libxfce4windowing uses GDK's Wayland connection, which is the applet's too, so the handle can be an argument of the applet's own requests. Caminito already maps a wlr handle to its window and a window to its ext identifier.

## Decision

- **A small private protocol, `caminito-toplevel-identifier-v1`, answers a wlr handle with the window's ext-foreign-toplevel-list identifier.** Its one request takes a `zwlr_foreign_toplevel_handle_v1`, and its event gives the identifier. Like the capture globals, it is hidden from sandboxed clients.
- **The applet reads the handle from the property**, looking up `"wlr-handle"` and then `"handle"` with `g_object_class_find_property`, in one function that goes from an `XfwWindow` to its identifier and to the ext handle with that identifier in the applet's own ext-foreign-toplevel-list.
- **A capture takes the ext handle**, through Caminito's existing capture source.
- **The Application overview for a Dock item is a method on `org.mate.Caminito` taking identifiers**, as `GetActiveToplevel` returns one. Any client on the session bus can call it; it opens the Overview and gives nothing back, and a sandboxed client can't learn identifiers.

## Consequences

- The ext identifier is the one name for a window between the applet and Caminito, so a later feature, such as highlighting a window from the Dock preview, needs no new way.
- If libxfce4windowing renames the property again, only the applet's one function changes; if it gains ext-foreign-toplevel-list support, that function can use it and the protocol can go.
- Rejected: a private protocol that makes a capture source straight from a wlr handle (it gives CMN-222 nothing); a patched libxfce4windowing that exposes the identifier (one more patched package on three distributions); matching by app id and title (wrong for equal titles).
