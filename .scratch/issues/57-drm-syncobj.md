# drm-syncobj

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M4
Blocked by: 55
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`linux-drm-syncobj-v1` on the tty backend, deferred from M1 to M4's DRM ([22](22-xdg-shell-and-input.md), [06](06-v01-protocol-and-integration-set.md)): explicit sync for clients that use it, as Smithay's `drm_syncobj` provides.

- **Offered only when the DRM device supports syncobj timelines**, and never on winit, where there's no DRM device to import them on.
- **linux-dmabuf on the tty (decided 2026-09-30):** Caminito offered no linux-dmabuf, and drm-syncobj applies only to dmabuf buffers, so 57 offers it on the tty, with the primary GPU's render formats as default feedback. Winit stays shm-only: nested GPU clients get no dmabuf, a gap for a later ticket.
- **Blocking:** a surface's commit waits for its acquire point before the scene uses the buffer, and the release point is signalled once the buffer isn't read any more.

## Acceptance criteria

- [ ] On the tty as `caminitotest`, the global is offered (`wayland-info`), and a client using explicit sync (a GTK4 app, or `vkcube --wsi wayland` with Mesa) renders without stalls or tearing. The global was checked in the virtual machine; no client there uses explicit sync, so that part is [60](60-tty-checks-left-over.md)'s.
- [x] Nested, the global isn't offered.

## Hand-check notes

Checked on 2026-10-04 in the UTM virtual machine (Arch Linux ARM, Linux 7.2.8, virtio-gpu with virgl, Mesa 26.2.3), with 5828597's `caminito`:

- **Nested:** `wayland-info` on a `tools/dev-nested --bin` Caminito in Plasma lists neither `zwp_linux_dmabuf_v1` nor `wp_linux_drm_syncobj_manager_v1`.
- **On the tty as `caminitotest`:** `wayland-info` lists `zwp_linux_dmabuf_v1` version 6, with the render formats on `/dev/dri/renderD128` as its one tranche, and `wp_linux_drm_syncobj_manager_v1` version 1. So virtio-gpu waits on syncobj timelines' points.
- **A dmabuf client:** `es2gears_wayland` for 15 s made 4 dmabufs and committed 57 to 68 times in each second, with no gap, so the wait on the implicit fence doesn't stall it. Caminito logged no error.
- **Not checked:** explicit sync itself. Mesa's EGL on virgl sees the global and doesn't bind it, so `es2gears_wayland` set no acquire point, and a GTK4 app would use the same EGL. The machine has no Vulkan driver for `vkcube`. Nobody watched the screen, so tearing wasn't looked for.

A tty Caminito started through `systemd-run` needs a VT above 6 there: on 1 to 6 logind starts `kmsconvt@`, which hangs the tty up, and Caminito quits without a word.

The acquire and release points are met only as far as the review of the code against anvil goes. A client that uses them, on a real GPU, is [60](60-tty-checks-left-over.md)'s, so this ticket is resolved.
