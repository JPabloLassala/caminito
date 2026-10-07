# Override-redirect windows

Type: implementation
Status: resolved
Milestone: M3
Blocked by: 35, 45
Map: [Caminito v0.1](../map.md)

## What to build

X11 menus, tooltips and other override-redirect windows in their own scene layer ([11](11-xwayland-integration.md), [15](15-crate-and-module-architecture.md)).

- **The scene's `OverrideRedirect` layer** is filled: above every toplevel layer and the `top` layer-shell layer, below `overlay` and the lock (the layer order is already in `scene.rs`). Within it, windows are ordered by `configure_notify`'s `above` sibling.
- **Placement:** in global logical coordinates, not per workspace. They're left out of policy and of the X stacking sync.
- **Input:** never focused on click. Pointer hit-testing reaches them before toplevels.
- **Keyboard grabs:** `zwp_xwayland_keyboard_grab_v1` through Smithay's handler, so X11 menus get the keyboard. Refusing and releasing grabs while locked is M7's (12); a `// M7` note marks where.
- **Scale:** they render with the X11 client scale like other XWayland surfaces.

## Acceptance criteria

- [x] X11-tier tests, written before the code: an override-redirect window stacks above a toplevel and above a fullscreen one; two override-redirect windows keep their `above` order after a restack; one mapped on workspace 1 stays visible after switching to workspace 2; clicking one doesn't change keyboard focus.
- [x] A headless test that the render elements put the override-redirect layer between `top` and `overlay`.
- [x] Nested, gtk3-demo with `GDK_BACKEND=x11` shows its menus and tooltips above its window, and its menus take keyboard navigation.

## Notes

- **Stacking:** a `configure_notify`'s `above` is the sibling just below the window, and that can be a managed window's frame, whose place among override-redirect windows Caminito can't know from the override-redirect windows alone. So `xwayland::stack::RootStack` follows every root child Smithay reports: created on top, reparented into a frame on top at a map request, back on the root on top at an unmap, restacked by `configure_notify`, and removed at destroy. A sibling it doesn't know is one of Smithay's own windows, which are at the bottom. Override-redirect windows are drawn in that order. Caminito's own X connection (38) could ask X instead.
- **Keyboard grabs:** Xwayland asks for the grab as the menu maps, before Caminito has heard of the map from X, so any surface Xwayland names gets it; only Xwayland sees the global, and it asks only for override-redirect windows. The menu gets the keyboard at once rather than at the next key, and when Xwayland destroys the grab, the refresh pass gives the keyboard back to policy's focus rather than waiting for a key.
- Override-redirect windows get frame callbacks on every output they're drawn on, since they have no primary output. They're sent no `wl_surface.enter`; Xwayland doesn't need it for its rootless windows.
- The nested check ran winit on Xvfb driven through XTest, as 35's did. In gtk3-demo's Application Class demo (`GDK_BACKEND=x11`), the Preferences menu opened over its window; Down three times moved to Color, Right opened its submenu with Red highlighted, three Escapes closed them, and the next key went to the window again. The demo's toolbar showed no tooltip on hover, and that wasn't checked against bare X, so a PyGObject GTK3 window's button tooltip stood in: it mapped as an override-redirect window and showed above its window. The check found that Xwayland asks for the keyboard grab before Caminito hears of the map, which the first version refused.
