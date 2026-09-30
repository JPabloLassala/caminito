# drm-syncobj

Type: implementation
Status: open
Milestone: M4
Blocked by: 55
Map: [Camino v0.1](../map.md)

## What to build

`linux-drm-syncobj-v1` on the tty backend, deferred from M1 to M4's DRM ([22](22-xdg-shell-and-input.md), [06](06-v01-protocol-and-integration-set.md)): explicit sync for clients that use it, as Smithay's `drm_syncobj` provides.

- **Offered only when the DRM device supports syncobj timelines**, and never on winit, where there's no DRM device to import them on.
- **Blocking:** a surface's commit waits for its acquire point before the scene uses the buffer, and the release point is signalled once the buffer isn't read any more.

## Acceptance criteria

- [ ] On the tty as `caminotest`, the global is offered (`wayland-info`), and a client using explicit sync (a GTK4 app, or `vkcube --wsi wayland` with Mesa) renders without stalls or tearing.
- [ ] Nested, the global isn't offered.
