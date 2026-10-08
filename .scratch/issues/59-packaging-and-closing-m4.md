# Packaging and closing M4

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M4
Blocked by: 52, 54, 56, 57, 58
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

M4's code installed through `caminito-git` (17: packaging keeps up), its checklist items, and the closing commit.

- **`caminito-git` (16):** gains the runtime dependencies the tty backend links or loads: seatd (libseat), libinput, mesa (GBM, EGL), systemd-libs (udev), and whatever Smithay's new features need. `namcap` reports nothing unexplained.
- **Checklist items tagged `M4`** in `docs/testing/checklist.md` (17), each with steps, the expected result and the debug tool:
  - a GTK3 window dragged across a 1.0 and a 2.0 output (13's visual part);
  - hot-plugging a monitor;
  - the scale picked for each of the developer's monitors.
- **Closing:** M4's exit from [17](17-milestone-slicing.md) on the installed package, as `caminitotest` on a spare tty, with the results of every `M1` to `M4` checklist item that still applies recorded in a `Close M4 On the metal` commit.

## Acceptance criteria

- [x] `makepkg -si` in `packaging/arch/caminito-git/` installs a `caminito` that runs on a tty, and the packaging job is green in CI.
- [x] The three checklist items are written.
- [x] Closing M4: as `caminitotest` on a spare tty, bare `caminito` drives both of the developer's monitors at their default scales; `wlr-randr` changes modes, scales and positions; mouse, touchpad and keyboard-layout settings changed with `gsettings` apply live; switching VT away and back works; CI is green; the checklist results are in a `Close M4 On the metal` commit.

## Notes

- **Closed (2026-10-04)** in the `Close M4 On the metal` commit, with the hot-plug item not run: the developer put it off, and [60](60-tty-checks-left-over.md) carries it.
- **Pushed (2026-10-04).** `master` is `origin/master` at 2e03d31. CI ([run 37182870690](https://github.com/JPabloLassala/caminito/actions/runs/37182870690)) and the Packaging job ([run 37182870731](https://github.com/JPabloLassala/caminito/actions/runs/37182870731)) are green on it, with 56's and 58's code and the `libdrm` change.
- **Closing run on the tty (2026-10-04), on caminito-git r116.2e03d31-1,** installed with `pacman -U` from `makepkg -f` on the developer's machine (x86_64, `check()` passing), as `caminitotest` on tty4, with `RUST_LOG=debug` and the log in `~/debug.log`. The saved layout was HDMI-A-1 at 0,0, 2259x1271 at 1.7, and DP-1 at 2259,0, 1920x1080 at 1.0. The wizard stopped after stage 11, so these are from its progress file:
  - Bare caminito drives both monitors at their default scales: pass.
  - The scale picked for each of the developer's monitors: pass.
  - wlr-randr changes modes, scales and positions: pass.
  - wlr-randr turns a monitor off and on: FAIL, twice. The first time the wizard ran from the KDE session's VT, so Caminito's VT was switched away when `--off` was sent, and Caminito refuses every output change then (58). The second time, on a new Caminito with the wizard waiting for Caminito's VT before each change, stages 4 and 5 were run again: 4 passed, and 5 failed with "nothing happened". Neither log has a `was turned off` line or a DRM warning, so the request was refused before it reached the GPU, or never arrived. What `wlr-randr` printed wasn't kept.
  - The same check by hand, on a third Caminito with its VT active: pass. `wlr-randr --output DP-1 --off` exited 0, the AOC went dark while the LG stayed lit, `wlr-randr` listed DP-1 as `Enabled: no`, and the log has `DP-1 was turned off`. Six seconds later `--on` exited 0 and DP-1 was lit again in its preferred mode, at 2259,0 and scale 1.0, with `lit DP-1` in the log and no new warning. So the wizard's stage 5 failed for a reason of its own, most likely the output's name as typed, which `wlr-randr` refuses without changing anything. Run again with foot dragged onto DP-1: when DP-1 went dark foot showed on the LG, which never went black, and `DumpState` has it against the LG's right edge (x 1394, 864 wide, on an output 2259 wide). It stayed there when DP-1 was lit again.
  - A GTK3 window dragged across a 1.0 and a 2.0 output: pass.
  - Hot-plugging a monitor: not run. Deferred by the developer to a later ticket; recorded by the wizard as FAIL.
  - Mouse settings apply live: pass.
  - Touchpad settings apply live: n/a. The machine has no touchpad.
  - Keyboard-layout settings apply live: pass.
  - Switching VT away and back works: pass. The log has `the VT was switched away` and `back` three times.
  - Explicit sync: pass.
  - Quitting gives the VT back in text mode: pass. Recorded as FAIL by mistake: the developer saw tty4 back at the shell prompt from before `caminito`, which is what's expected. The log ends with `stopping on a signal`.
  - The log's only errors are 8 of Smithay's `surface missing from known popups`. Its warnings are from startup: no DRM master at first, two `Failed to destroy old mode property blob`, and X displays 0 and 1 taken by the host's session, so Xwayland is on `:2`.
- **The nested items (2026-10-04), on caminito-git r116.2e03d31-1,** with `tools/dev-nested` on the host desktop (KDE on Wayland). Claude ran the commands and read the results; what needs eyes and hands was the developer's.
  - A nested run uses its private bus: pass. Caminito's `DBUS_SESSION_BUS_ADDRESS` was a `/tmp/dbus-*` bus, not `unix:path=/run/user/1000/bus`.
  - dconf goes to the scratch config: pass. The nested read printed `true`, `dconf/user` was in the scratch `XDG_CONFIG_HOME`, the host read printed nothing, and the scratch directory was gone after exit.
  - The host desktop's systemd environment is untouched: pass. It was the same before and after the run.
  - The debug interface answers on the nested bus: pass. `DumpState` listed pluma as ssd with `strip_scale` 1, and `ToggleFrameDebug` answered true, then false. The outlines weren't looked at in this run at scale 1, only at scale 2.
  - A theme switch while windows are open: pass, by `DumpState` only. Through BlackMATE, HighContrast, TraditionalOk, NoSuchTheme and Menta, pluma, caja and gtk3-demo all stayed mapped, the maximised pluma and its frame filled the 1280x800 output, caja kept its 800x526 client area, and `frame_extents` followed the theme. NoSuchTheme logged one `can't load the Marco theme` line. Nobody looked at the frames or tried Menta's buttons afterwards.
  - Frames at an output scale of 2 in a nested run: pass. The output was at scale 2 and pluma ssd with `strip_scale` 2 and the extents of scale 1. The developer saw the frame labelled x2 and sharp, and the buttons, drag and resize working.
  - X11 apps map, get frames, take focus and move: pass, by the developer. `DumpState` had xterm, xeyes and the Application Class demo as ssd with extents 1/1/31/1.
  - Menus and tooltips sit in the override-redirect layer: pass, by the developer.
  - `_NET_FRAME_EXTENTS` and `_NET_WORKAREA` are right: pass. 1, 1, 31, 1 on xterm, xeyes and the Application Class demo; `_NET_WORKAREA` 0, 0, 1280, 800 once for each of the 4 desktops, and `_GTK_WORKAREAS_D0` the same.
  - An X11 app's copy and paste to and from a native app: pass, by the developer. The `xclip` read with foot focused wasn't done.
  - Killing Xwayland brings it back with the root properties republished: pass. Back on the same `:3` about 590 ms after SIGKILL, the six root properties identical before and after, and a new xterm framed with extents 1/1/31/1.
  - Switching keyboard layouts in native and X11 apps: pass. Xwayland's keymap was `pc_us_es_2_inet(evdev)_group(alt_shift_toggle)`, and the developer typed `;` and `ñ` in pluma and in xterm, one Alt+Shift switching both.
  - `DumpState` also listed gtk3-demo's main window, client-decorated, 800x645 and with no frame, once for each `gtk3-demo --run` started on Xwayland. Nobody checked whether it shows.
- **Packaging:** 55 and 56 already added the tty's dependencies: `seatd`, `systemd-libs`, `mesa`, `libdisplay-info`, `libglvnd` and `libinput`. This ticket drops `libdrm`. The Packaging job's namcap on 3380aec ([run 37177692118](https://github.com/JPabloLassala/caminito/actions/runs/37177692118)) flagged it as possibly not needed, and it's right: `readelf -d` on a built `caminito` shows `libgbm`, `libseat`, `libudev`, `libinput`, `libdisplay-info` and `libxkbcommon`, but no `libdrm`, since Smithay's `drm` crate makes the KMS ioctls itself. mesa depends on libdrm for GBM, so it's still installed. CI's package list drops it too.
- **namcap warnings that stay,** each explained in the PKGBUILD's comment: ld-linux, `librsvg`, `marco`, `mate-themes` and `xorg-xwayland` (as in 33 and 43), and `libglvnd`, whose libEGL Smithay dlopens.
- **Smithay's new features** (`backend_drm`, `backend_gbm`, `backend_udev`, `backend_session_libseat`, `backend_libinput`, `renderer_gl`) link nothing beyond those libraries.
- **`arch` is still `x86_64` only.** `makepkg` refuses on an aarch64 machine without `-A`. Left for the developer to decide.
- **The checklist items** are in a new *Outputs* section, with a paragraph at the top on running bare `caminito` as `caminitotest`. The scale item expects 50's table: the AOC on DP-1 at 1.0 and the LG on HDMI-A-1 at 1.7. The drag item sets the LG to 2.0 with `wlr-randr`. [54](54-keyboard-layouts.md) already wrote its nested layout item, tagged `M4`.
- **Installed in the virtual machine (2026-10-04),** Arch Linux ARM, on cfda02b: `makepkg -A` built `caminito-git r115.cfda02b-1`, and `namcap PKGBUILD` reports nothing. The installed `/usr/bin/caminito`, as `caminitotest` in a logind session on tty4, lit `Virtual-1` in its preferred mode, started Xwayland, offered `zwlr_output_manager_v1` v4 and `wp_linux_drm_syncobj_manager_v1`, answered `DumpState`, logged the VT switched away and back, and stopped cleanly on SIGTERM. `readelf -d` on the packaged `caminito` shows no `libdrm`. Not checked there: `namcap` on the built package, which reports every library as uninstalled on Arch Linux ARM, so CI's is the one that counts; and the x86_64 build.
- **`check()` fails on aarch64.** `caminito-theme`'s `goldens` test differs from Marco's pixels in 4 of 680 cases, each by one pixel, by 1: Menta's `minimize-prelight@1x`, `maximize-prelight@1x`, `close-prelight@1x` and `unmaximize-prelight@1x`. Every other test passed (450 in `caminito`, 113 in `caminito-policy`, 139 unit tests in `caminito-theme`). The goldens are rendered on x86_64 (ADR 0009), and CI is x86_64, so the package was built with `--nocheck` there. It has no ticket yet; it matters only if `arch` gains `aarch64`.
- **The closing wizard** is [59-closing-checks.sh](59-closing-checks.sh), as 43's. Run it as `caminitotest` from a second login (ssh from another machine is best, since the monitors stay on Caminito), with `caminito 2>~/caminito.log` on a spare tty. It walks through M4's exit from 17 (both monitors at their default scales; `wlr-randr` changing a mode, a scale and a position; mouse, touchpad and layout keys set with `gsettings`; a VT switch), the three *Outputs* checklist items, and what else the same sitting can do: [60](60-tty-checks-left-over.md)'s monitor off and on and its explicit sync, and 55's quitting. 60's results are written here with the rest, so 60's notes need a line that points to them. A machine with no touchpad records that part of the exit as `n/a`. A run that starts over needs a new Caminito, since what `wlr-randr` set stays set (58). It puts the outputs back after each `wlr-randr` stage, resets each key it sets, and saves each result as it's given, in `~/.local/state/caminito/59-closing-checks.progress`. `caminitotest` can't write the developer's checkout, so from a copy the results go to `~/.local/state/caminito/59-closing-results.md`, to copy into these notes.
- **The nested items** (M1 to M3, and 54's M4 layout item) still apply. 43's and 54's wizards walk through them on the host desktop, with `tools/dev-nested` on the installed package, and now take the ticket to write to: `TICKET=$PWD/59-packaging-and-closing-m4.md ./43-closing-checks.sh`, and the same for `54-hand-checks.sh`, also without the `--bin` that 54's wizard names.
- **The wizard's dry run (2026-10-04)** in the virtual machine, on the installed package, with one answer for every question: all 12 stages ran with no script error. The review's fixes after it (a resumed run, the place asked for in stage 4, the places set in stage 6) were not run again there. With its one monitor, `window-scaling-factor` 2 put `Virtual-1` at 2 and 0 brought 1 back; `wlr-randr` set scale 1.5 and position 0,2400, and `DumpState` followed (853x533 at 0,2400); `--off` listed it as not enabled and `--on` lit it again; the saved layout was put back each time. Caminito stayed up through all of it. That is the first run of 58's tty code for scale, place, off and on. A mode switch, two monitors, real input and a real hot-plug are still the developer's.
