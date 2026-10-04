# Tty checks left over from 55, 57 and 58

Type: verification
Status: open
Milestone: before v0.1
Blocked by: 55, 57, 58
Map: [Camino v0.1](../map.md)

## What to check

Two of [55](55-the-tty-backend.md)'s criteria and one of [57](57-drm-syncobj.md)'s were checked only in the UTM virtual machine, which has no real monitors and no client that uses explicit sync. This ticket runs them on the developer's machine, with its two monitors. It's checking only: anything that fails gets its own ticket.

- **Two monitors (55):** the virtual machine has one connector with a picture. A second one, `Virtual-2`, was forced on through `/sys/class/drm/card0-Virtual-2/status`. Camino lit both, left to right, but the forced connector has no EDID, so its preferred mode, physical size and default scale ([50](50-default-output-scale.md)) weren't exercised, and nobody saw its picture.
- **Explicit sync (57):** [57](57-drm-syncobj.md)'s global is offered in the virtual machine, but no client there uses it: Mesa's EGL on virgl doesn't bind `wp_linux_drm_syncobj_manager_v1`, and there's no Vulkan driver. So no commit waited on an acquire point, and nobody watched for stalls or tearing. With `WAYLAND_DEBUG=1`, a client that uses explicit sync logs `set_acquire_point` with each commit.
- **Hot-plug (55):** the forced connector was turned off and on again, with a udev change event sent by hand. Camino removed and added the output. No real hot-plug event came from hardware, and no window was on the output that went, so windows moving off it wasn't checked.
- **Output management (58):** [58](58-wlr-output-management.md)'s tty criterion wasn't checked anywhere, not even in the virtual machine. Its headless tests and the nested `wlr-randr` run don't reach the tty's code: the mode switch (`DrmOutput::use_mode`), and a monitor turned off and lit again on its CRTC. `wlr-randr` is run from another VT or over ssh, with `WAYLAND_DISPLAY` set.

Setup: on a spare tty as `caminotest`, run bare `camino`. With [56](56-libinput-on-the-tty.md) in, the keyboard and pointer work there, so a window can be dragged to the second monitor. A terminal is started from another VT or over ssh with `WAYLAND_DISPLAY` set to the socket Camino logs. The outputs' places and scales are in the debug interface's `DumpState` (32).

## Acceptance criteria

- [x] Bare `camino` lights both of the developer's monitors in their preferred modes, left to right, at their default scales, and a terminal started with its `WAYLAND_DISPLAY` shows on them.
- [ ] Unplugging a monitor removes its output, and a window that was on it moves to the other monitor.
- [ ] Re-plugging the monitor adds its output again, in its preferred mode and at its default scale.
- [x] `wayland-info` lists `wp_linux_drm_syncobj_manager_v1`, and a client that uses explicit sync (a GTK4 app, or `vkcube --wsi wayland` with Mesa) sets acquire points and renders without stalls or tearing.
- [x] `wlr-randr` lists both monitors with all their modes, and changes one monitor's mode, scale and position; the picture and `DumpState` follow, and frames keep coming after the mode switch.
- [x] `wlr-randr --output <name> --off` turns a monitor off and its windows move to the other; `--on` lights it again.
- [ ] The results are recorded in this ticket's notes, with a new ticket for each failure.

## Notes

- **Run with 59's closing wizard (2026-10-04),** on `camino-git r116.2e03d31-1`, on the developer's machine. The results are in [59](59-packaging-and-closing-m4.md)'s notes, under *Closing run on the tty*. Of this ticket's criteria: the two monitors at their default scales, explicit sync, and `wlr-randr`'s modes, scales and positions passed. `wlr-randr --off` did nothing in the wizard, but by hand from Camino's VT it turned DP-1 off, foot moved from it to the LG, and `--on` lit it again, so that criterion passed. The hot-plug wasn't run, so its two criteria are still open.
- **M4 closed without the hot-plug (2026-10-04).** The developer put it off, so this ticket stays open for its two hot-plug criteria and gates no milestone. The *Hot-plugging a monitor* checklist item is the procedure, and stage 7 of 59's wizard walks through it.
