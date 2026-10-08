# Debug interface

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M2
Blocked by: 30
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The first D-Bus in Caminito ([14](14-development-and-testing-loop.md), [15](15-crate-and-module-architecture.md)): the `dbus` thread, `org.mate.Caminito.Debug` and the frame-debug toggle.

- **The `dbus` module (15):** async zbus on its own executor thread. Interface methods send a request with a reply sender into the main loop over a `calloop::channel` and await it, so the main thread never blocks on the bus.
- **`org.mate.Caminito.Debug`,** a thin interface over the `debug` module:
  - a state dump as JSON: windows (id, app id, title, workspace, geometry, SSD or CSD, frame extents, the strip scale), workspaces with their names, stacking, outputs and scales, and focus. The lock state and the X11 scale are added by the milestones that create them;
  - a method to toggle **frame debug** ([14](14-development-and-testing-loop.md)), which outlines each frame's strips, invisible resize border and title rect, and labels the scale its strips were rendered at. The outlines are added at scene assembly. The keybinding for it comes with M6's `org.mate.Caminito` keybinding manifests.
- **Tests (14):** a zbus proxy in `caminito-test-client`, and `caminito`'s test binary running under its own `dbus-run-session` in CI.

## Acceptance criteria

- [x] An insta snapshot of the state dump for a headless fixture with an SSD and a CSD window on two workspaces.
- [x] A headless test that frame debug adds the outline elements and turning it off removes them.
- [x] Nested, `busctl --user call` on the nested bus returns the dump, and frame debug shows its outlines on a GTK3 app.
