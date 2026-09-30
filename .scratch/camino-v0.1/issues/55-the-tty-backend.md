# The tty backend

Type: implementation
Status: open
Milestone: M4
Blocked by: 51
Map: [Camino v0.1](../map.md)

## What to build

`backend::tty` ([15](15-crate-and-module-architecture.md), [08](08-session-launch-model.md)): bare `camino` on a spare tty drives the machine's monitors.

- **Choosing the backend:** winit when `WAYLAND_DISPLAY` or `DISPLAY` is set, as `tools/dev-nested` runs it, the tty otherwise, as niri does (decided 2026-09-30). `main.rs` stops always running nested.
- **Session:** libseat, so seatd or logind grants the devices. Pausing on VT switch away drops the DRM master and input; resuming takes them back and redraws every output. Teardown gives the VT back (08).
- **VT switching:** the `XF86Switch_VT_n` keysyms switch VTs. They're handled before bindings, and pass the lock stage (15).
- **DRM/KMS:** udev finds the GPU and its connectors, and hot-plug events. Each connected connector becomes a [51](51-output-lifecycle.md) output, with its EDID's size, preferred mode and whether it's internal passed to [50](50-default-output-scale.md)'s rule. GBM and GLES render the scene, with the same renderer interface the winit backend uses.
- **Hot-plug:** a connector appearing or going adds or removes its output (51).
- **Smithay features:** `backend_drm`, `backend_gbm`, `backend_udev`, `backend_session_libseat` and the GL renderer.
- **Not here:** input devices ([56](56-libinput-on-the-tty.md)), drm-syncobj ([57](57-drm-syncobj.md)), output management ([58](58-wlr-output-management.md)).
- **Tests:** nothing in CI drives DRM (14); the backend selection is a unit test, and the rest is checked by hand.

## Acceptance criteria

- [x] A unit test, written before the code, of the backend choice for each combination of `WAYLAND_DISPLAY` and `DISPLAY`.
- [x] `tools/dev-nested` still runs winit, unchanged.
- [ ] On a spare tty as `caminotest`, bare `camino` lights both of the developer's monitors in their preferred modes, left to right, at their default scales, and a terminal started with its `WAYLAND_DISPLAY` shows on them.
- [ ] Switching to another VT and back works, and the outputs redraw.
- [ ] Unplugging and re-plugging a monitor removes and adds its output, and windows move off it.
- [ ] Quitting gives the VT back in text mode.

## Hand-check notes

Until libinput comes in 56, no key reaches Camino on the tty, so the VT keysyms can't be pressed there yet. Switch VTs from another VT or over ssh with `chvt <n>` (or `loginctl activate`), and quit by sending `camino` SIGTERM or SIGINT, which now stop it cleanly. A terminal is started from another VT with `WAYLAND_DISPLAY` set to the socket Camino logs.
