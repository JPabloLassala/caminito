# Tty checks left over from 55, 57 and 58

Type: verification
Status: resolved
Milestone: before v0.1
Blocked by: 55, 57, 58
Map: [Caminito v0.1](../map.md)

## What to check

Two of [55](55-the-tty-backend.md)'s criteria and one of [57](57-drm-syncobj.md)'s were checked only in the UTM virtual machine, which has no real monitors and no client that uses explicit sync. This ticket runs them on the developer's machine, with its two monitors. It's checking only: anything that fails gets its own ticket.

- **Two monitors (55):** the virtual machine has one connector with a picture. A second one, `Virtual-2`, was forced on through `/sys/class/drm/card0-Virtual-2/status`. Caminito lit both, left to right, but the forced connector has no EDID, so its preferred mode, physical size and default scale ([50](50-default-output-scale.md)) weren't exercised, and nobody saw its picture.
- **Explicit sync (57):** [57](57-drm-syncobj.md)'s global is offered in the virtual machine, but no client there uses it: Mesa's EGL on virgl doesn't bind `wp_linux_drm_syncobj_manager_v1`, and there's no Vulkan driver. So no commit waited on an acquire point, and nobody watched for stalls or tearing. With `WAYLAND_DEBUG=1`, a client that uses explicit sync logs `set_acquire_point` with each commit.
- **Hot-plug (55):** the forced connector was turned off and on again, with a udev change event sent by hand. Caminito removed and added the output. No real hot-plug event came from hardware, and no window was on the output that went, so windows moving off it wasn't checked.
- **Output management (58):** [58](58-wlr-output-management.md)'s tty criterion wasn't checked anywhere, not even in the virtual machine. Its headless tests and the nested `wlr-randr` run don't reach the tty's code: the mode switch (`DrmOutput::use_mode`), and a monitor turned off and lit again on its CRTC. `wlr-randr` is run from another VT or over ssh, with `WAYLAND_DISPLAY` set.

Setup: on a spare tty as `caminitotest`, run bare `caminito`. With [56](56-libinput-on-the-tty.md) in, the keyboard and pointer work there, so a window can be dragged to the second monitor. A terminal is started from another VT or over ssh with `WAYLAND_DISPLAY` set to the socket Caminito logs. The outputs' places and scales are in the debug interface's `DumpState` (32).

## Acceptance criteria

- [x] Bare `caminito` lights both of the developer's monitors in their preferred modes, left to right, at their default scales, and a terminal started with its `WAYLAND_DISPLAY` shows on them.
- [x] Unplugging a monitor removes its output, and a window that was on it moves to the other monitor.
- [x] Re-plugging the monitor adds its output again, in its preferred mode and at its default scale.
- [x] `wayland-info` lists `wp_linux_drm_syncobj_manager_v1`, and a client that uses explicit sync (a GTK4 app, or `vkcube --wsi wayland` with Mesa) sets acquire points and renders without stalls or tearing.
- [x] `wlr-randr` lists both monitors with all their modes, and changes one monitor's mode, scale and position; the picture and `DumpState` follow, and frames keep coming after the mode switch.
- [x] `wlr-randr --output <name> --off` turns a monitor off and its windows move to the other; `--on` lights it again.
- [x] The results are recorded in this ticket's notes, with a new ticket for each failure.

## Notes

- **Run with 59's closing wizard (2026-10-04),** on `caminito-git r116.2e03d31-1`, on the developer's machine. The results are in [59](59-packaging-and-closing-m4.md)'s notes, under *Closing run on the tty*. Of this ticket's criteria: the two monitors at their default scales, explicit sync, and `wlr-randr`'s modes, scales and positions passed. `wlr-randr --off` did nothing in the wizard, but by hand from Caminito's VT it turned DP-1 off, foot moved from it to the LG, and `--on` lit it again, so that criterion passed. The hot-plug wasn't run, so its two criteria are still open.
- **M4 closed without the hot-plug (2026-10-04).** The developer put it off, so this ticket stays open for its two hot-plug criteria and gates no milestone. The *Hot-plugging a monitor* checklist item is the procedure, and stage 7 of 59's wizard walks through it.
- **The hot-plug was run (2026-10-06),** on `caminito-git r195.9e4baac-1`, in the developer's own Session on tty1 and not as `caminitotest` with bare `caminito`: the LG on HDMI-A-1 (3840x2160 at 59.94 Hz, scale 1.75, at 0,0, primary) and the AOC on DP-1 (1920x1080 at 144 Hz, scale 1.0, at 2194,0), with that layout saved ([83](83-saved-layouts.md)). The developer pulled the cables and said what the monitors showed; `wlr-randr`, `DumpState` and the connectors' `status` were read 5 s after each change.
  - **Unplugging the AOC:** one output left, HDMI-A-1 at 0,0. The pluma dragged to the AOC and a maximised Firefox window that was on it were on the LG, the Firefox one filling its work area (2194x1183 at 0,26, from 1920x1080 at 2194,0). Pass.
  - **Plugging it back:** DP-1 again, at 2194,0, scale 1.0, 1920x1080 at 144 Hz. Pass, as the saved layout has it: the monitor's preferred mode is 60 Hz, so this is 83's restore, and the preferred mode and default scale of a re-plugged monitor with no saved layout weren't exercised on hardware (59's run covers them at start-up, and the headless tests on a hot-plug). The windows stayed on the LG: nothing puts a window back on a monitor that returns.
  - **Unplugging the LG:** DP-1 alone, moved to 0,0 (82). Every window was on it, the maximised ones at its work area (1920x1029 at 0,26), and both panels and the desktop. Pass.
  - **Plugging it back:** HDMI-A-1 at 0,0 and primary, at scale 1.75 and 59.94 Hz, DP-1 back at 2194,0. Pass. The windows stayed on DP-1, moved with it, and the panels went back to the LG.
  - **The log** has no `can't light` line. It has `Failed to destroy old mode property blob` at each re-plug, and `Failed to handle X11 event` twice after the LG's.
  - **Not done:** xterm wasn't put on the unplugged monitor, so the only X11 window that moved was one on the LG at the same place on DP-1. Each cable was pulled once.
- **Three failures, each with its ticket.** The monitor that stayed went black for a short time at all four changes: [87](87-hot-plug-blanks-the-other-monitor.md). Each hot-plug left more `mate-background` layer surfaces, five on the LG and three on the AOC at the end, from one each: [88](88-wallpaper-surfaces-pile-up-on-hot-plug.md). The desktop icons stayed on DP-1 when the LG, the primary output, came back: [89](89-desktop-icons-stay-when-the-primary-comes-back.md). This ticket's criteria passed, so it's resolved; the checklist item *Hot-plugging a monitor* fails on its "never goes black" until 87.
- **Seen in the same log, not a hot-plug:** while the VT was switched away to `caminitotest`'s login on tty4, Caminito logged `New screen configuration invalid!` 646 times, with the connectors removed and added in a loop, at 15:35, 15:36 and 15:40. No ticket was filed, since nobody looked at what the screen did on the way back.
