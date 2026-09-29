# Override-redirect windows

Type: implementation
Status: open
Milestone: M3
Blocked by: 35
Map: [Camino v0.1](../map.md)

## What to build

X11 menus, tooltips and other override-redirect windows in their own scene layer ([11](11-xwayland-integration.md), [15](15-crate-and-module-architecture.md)).

- **The scene's `OverrideRedirect` layer** is filled: above every toplevel layer and the `top` layer-shell layer, below `overlay` and the lock (the layer order is already in `scene.rs`). Within it, windows are ordered by `configure_notify`'s `above` sibling.
- **Placement:** in global logical coordinates, not per workspace. They're left out of policy and of the X stacking sync.
- **Input:** never focused on click. Pointer hit-testing reaches them before toplevels.
- **Keyboard grabs:** `zwp_xwayland_keyboard_grab_v1` through Smithay's handler, so X11 menus get the keyboard. Refusing and releasing grabs while locked is M7's (12); a `// M7` note marks where.
- **Scale:** they render with the X11 client scale like other XWayland surfaces.

## Acceptance criteria

- [ ] X11-tier tests, written before the code: an override-redirect window stacks above a toplevel and above a fullscreen one; two override-redirect windows keep their `above` order after a restack; one mapped on workspace 1 stays visible after switching to workspace 2; clicking one doesn't change keyboard focus.
- [ ] A headless test that the render elements put the override-redirect layer between `top` and `overlay`.
- [ ] Nested, gtk3-demo with `GDK_BACKEND=x11` shows its menus and tooltips above its window, and its menus take keyboard navigation.
