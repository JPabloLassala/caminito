# Packaging and closing M4

Type: implementation
Status: open
Milestone: M4
Blocked by: 52, 54, 56, 57, 58
Map: [Camino v0.1](../map.md)

## What to build

M4's code installed through `camino-git` (17: packaging keeps up), its checklist items, and the closing commit.

- **`camino-git` (16):** gains the runtime dependencies the tty backend links or loads: seatd (libseat), libinput, mesa (GBM, EGL), systemd-libs (udev), and whatever Smithay's new features need. `namcap` reports nothing unexplained.
- **Checklist items tagged `M4`** in `docs/testing/checklist.md` (17), each with steps, the expected result and the debug tool:
  - a GTK3 window dragged across a 1.0 and a 2.0 output (13's visual part);
  - hot-plugging a monitor;
  - the scale picked for each of the developer's monitors.
- **Closing:** M4's exit from [17](17-milestone-slicing.md) on the installed package, as `caminotest` on a spare tty, with the results of every `M1` to `M4` checklist item that still applies recorded in a `Close M4 On the metal` commit.

## Acceptance criteria

- [ ] `makepkg -si` in `packaging/arch/camino-git/` installs a `camino` that runs on a tty, and the packaging job is green in CI.
- [x] The three checklist items are written.
- [ ] Closing M4: as `caminotest` on a spare tty, bare `camino` drives both of the developer's monitors at their default scales; `wlr-randr` changes modes, scales and positions; mouse, touchpad and keyboard-layout settings changed with `gsettings` apply live; switching VT away and back works; CI is green; the checklist results are in a `Close M4 On the metal` commit.

## Notes

- **Not closed (2026-10-04).** The packaging and the checklist items are done. The closing run isn't, because the blockers aren't resolved:
  - [58](58-wlr-output-management.md) isn't on `master`. Its only code is a WIP commit on `origin/ticket-58-output-management` (0cd91ec), whose CI failed. M4's exit and two of the checklist items below need `wlr-randr`.
  - [54](54-keyboard-layouts.md), [55](55-the-tty-backend.md), [56](56-libinput-on-the-tty.md) and [57](57-drm-syncobj.md) are on `master`, but their criteria on the tty and nested are unticked.
  - The closing run needs the developer at the machine with both monitors, as `caminotest`.
- **Packaging:** 55 and 56 already added the tty's dependencies: `seatd`, `systemd-libs`, `mesa`, `libdisplay-info`, `libglvnd` and `libinput`. This ticket drops `libdrm`. The Packaging job's namcap on 3380aec ([run 37177692118](https://github.com/JPabloLassala/camino/actions/runs/37177692118)) flagged it as possibly not needed, and it's right: `readelf -d` on a built `camino` shows `libgbm`, `libseat`, `libudev`, `libinput`, `libdisplay-info` and `libxkbcommon`, but no `libdrm`, since Smithay's `drm` crate makes the KMS ioctls itself. mesa depends on libdrm for GBM, so it's still installed. CI's package list drops it too.
- **namcap warnings that stay,** each explained in the PKGBUILD's comment: ld-linux, `librsvg`, `marco`, `mate-themes` and `xorg-xwayland` (as in 33 and 43), and `libglvnd`, whose libEGL Smithay dlopens.
- **Smithay's new features** (`backend_drm`, `backend_gbm`, `backend_udev`, `backend_session_libseat`, `backend_libinput`, `renderer_gl`) link nothing beyond those libraries.
- **`arch` is still `x86_64` only.** `makepkg` refuses on an aarch64 machine without `-A`. Left for the developer to decide.
- **The checklist items** are in a new *Outputs* section, with a paragraph at the top on running bare `camino` as `caminotest`. The scale item expects 50's table: the AOC on DP-1 at 1.0 and the LG on HDMI-A-1 at 1.7. The drag item sets the LG to 2.0 with `wlr-randr`, so it waits for 58, as the scale item's `wlr-randr` step does; `DumpState` shows the same there. [54](54-keyboard-layouts.md) already wrote its nested layout item, tagged `M4`.
- **Not verified here:** nothing was built or pushed after the change. The Packaging job runs when `packaging/arch/` is pushed. The closing wizard (as [43-closing-checks.sh](43-closing-checks.sh)) isn't written yet; it's best written against 58's finished behaviour.
