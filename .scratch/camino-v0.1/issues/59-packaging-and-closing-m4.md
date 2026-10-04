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

- **Not closed (2026-10-04).** The packaging, the checklist items and the closing wizard are done, and the blockers are all resolved. What's left needs the developer:
  - the closing run on the machine with both monitors, as `caminotest`, with the wizard below;
  - a push. `master` is 17 commits ahead of `origin/master` (3380aec), so CI and the Packaging job haven't run on 58's or 56's code, or on the `libdrm` change;
  - the `Close M4 On the metal` commit, with the results.
- **Packaging:** 55 and 56 already added the tty's dependencies: `seatd`, `systemd-libs`, `mesa`, `libdisplay-info`, `libglvnd` and `libinput`. This ticket drops `libdrm`. The Packaging job's namcap on 3380aec ([run 37177692118](https://github.com/JPabloLassala/camino/actions/runs/37177692118)) flagged it as possibly not needed, and it's right: `readelf -d` on a built `camino` shows `libgbm`, `libseat`, `libudev`, `libinput`, `libdisplay-info` and `libxkbcommon`, but no `libdrm`, since Smithay's `drm` crate makes the KMS ioctls itself. mesa depends on libdrm for GBM, so it's still installed. CI's package list drops it too.
- **namcap warnings that stay,** each explained in the PKGBUILD's comment: ld-linux, `librsvg`, `marco`, `mate-themes` and `xorg-xwayland` (as in 33 and 43), and `libglvnd`, whose libEGL Smithay dlopens.
- **Smithay's new features** (`backend_drm`, `backend_gbm`, `backend_udev`, `backend_session_libseat`, `backend_libinput`, `renderer_gl`) link nothing beyond those libraries.
- **`arch` is still `x86_64` only.** `makepkg` refuses on an aarch64 machine without `-A`. Left for the developer to decide.
- **The checklist items** are in a new *Outputs* section, with a paragraph at the top on running bare `camino` as `caminotest`. The scale item expects 50's table: the AOC on DP-1 at 1.0 and the LG on HDMI-A-1 at 1.7. The drag item sets the LG to 2.0 with `wlr-randr`. [54](54-keyboard-layouts.md) already wrote its nested layout item, tagged `M4`.
- **Installed in the virtual machine (2026-10-04),** Arch Linux ARM, on cfda02b: `makepkg -A` built `camino-git r115.cfda02b-1`, and `namcap PKGBUILD` reports nothing. The installed `/usr/bin/camino`, as `caminotest` in a logind session on tty4, lit `Virtual-1` in its preferred mode, started Xwayland, offered `zwlr_output_manager_v1` v4 and `wp_linux_drm_syncobj_manager_v1`, answered `DumpState`, logged the VT switched away and back, and stopped cleanly on SIGTERM. `readelf -d` on the packaged `camino` shows no `libdrm`. Not checked there: `namcap` on the built package, which reports every library as uninstalled on Arch Linux ARM, so CI's is the one that counts; and the x86_64 build.
- **`check()` fails on aarch64.** `camino-theme`'s `goldens` test differs from Marco's pixels in 4 of 680 cases, each by one pixel, by 1: Menta's `minimize-prelight@1x`, `maximize-prelight@1x`, `close-prelight@1x` and `unmaximize-prelight@1x`. Every other test passed (450 in `camino`, 113 in `camino-policy`, 139 unit tests in `camino-theme`). The goldens are rendered on x86_64 (ADR 0009), and CI is x86_64, so the package was built with `--nocheck` there. It has no ticket yet; it matters only if `arch` gains `aarch64`.
- **The closing wizard** is [59-closing-checks.sh](59-closing-checks.sh), as 43's. Run it as `caminotest` from a second login (ssh from another machine is best, since the monitors stay on Camino), with `camino 2>~/camino.log` on a spare tty. It walks through M4's exit from 17 (both monitors at their default scales; `wlr-randr` changing a mode, a scale and a position; mouse, touchpad and layout keys set with `gsettings`; a VT switch), the three *Outputs* checklist items, and what else the same sitting can do: [60](60-tty-checks-left-over.md)'s monitor off and on and its explicit sync, and 55's quitting. 60's results are written here with the rest, so 60's notes need a line that points to them. A machine with no touchpad records that part of the exit as `n/a`. A run that starts over needs a new Camino, since what `wlr-randr` set stays set (58). It puts the outputs back after each `wlr-randr` stage, resets each key it sets, and saves each result as it's given, in `~/.local/state/camino/59-closing-checks.progress`. `caminotest` can't write the developer's checkout, so from a copy the results go to `~/.local/state/camino/59-closing-results.md`, to copy into these notes.
- **The nested items** (M1 to M3, and 54's M4 layout item) still apply. 43's and 54's wizards walk through them on the host desktop, with `tools/dev-nested` on the installed package, and now take the ticket to write to: `TICKET=$PWD/59-packaging-and-closing-m4.md ./43-closing-checks.sh`, and the same for `54-hand-checks.sh`, also without the `--bin` that 54's wizard names.
- **The wizard's dry run (2026-10-04)** in the virtual machine, on the installed package, with one answer for every question: all 12 stages ran with no script error. The review's fixes after it (a resumed run, the place asked for in stage 4, the places set in stage 6) were not run again there. With its one monitor, `window-scaling-factor` 2 put `Virtual-1` at 2 and 0 brought 1 back; `wlr-randr` set scale 1.5 and position 0,2400, and `DumpState` followed (853x533 at 0,2400); `--off` listed it as not enabled and `--on` lit it again; the saved layout was put back each time. Camino stayed up through all of it. That is the first run of 58's tty code for scale, place, off and on. A mode switch, two monitors, real input and a real hot-plug are still the developer's.
