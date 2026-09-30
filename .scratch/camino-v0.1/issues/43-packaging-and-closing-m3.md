# Packaging and closing M3

Type: implementation
Status: open
Milestone: M3
Blocked by: 42, 49
Map: [Camino v0.1](../map.md)

## What to build

M3's code installed through `camino-git` (17: packaging keeps up), its checklist items, and the closing commit.

- **`camino-git` (16):**
  - gains the runtime dependency `xorg-xwayland`, and whatever x11rb's features need;
  - `namcap` reports nothing unexplained.
- **`tools/dev-nested`:** prints the nested `DISPLAY` beside `WAYLAND_DISPLAY`.
- **Checklist items tagged `M3`** in `docs/testing/checklist.md` (17), each with steps, the expected result and the debug tool:
  - an X11 app's copy and paste to and from a native app;
  - `DISPLAY` only exported after the root properties exist (read from `/proc/<pid>/environ` of a child spawned early, and `xprop -root`).
- **Closing:** M3's exit from [17](17-milestone-slicing.md) on the installed package, with the results of every `M1`, `M2` and `M3` checklist item recorded in a `Close M3 XWayland` commit.

## Acceptance criteria

- [ ] `makepkg -si` in `packaging/arch/camino-git/` installs a `camino` that starts Xwayland, and the packaging job is green in CI.
- [x] The two checklist items are written.
- [ ] Closing M3: nested, xterm, xeyes and a GTK3 app with `GDK_BACKEND=x11` map, get Marco frames, take focus and move; their menus and tooltips sit in the override-redirect layer; `_NET_FRAME_EXTENTS` and `_NET_WORKAREA` are right; killing Xwayland brings it back on the same display with the root properties republished; the X11 tier passes in CI; the checklist results are in a `Close M3 XWayland` commit.

## Notes

- **Packaging:** `depends` gains `xorg-xwayland`, which `check()`'s X11 tier runs too. Camino's own x11rb features, `shape` and `xkb`, are pure Rust and link nothing, so they add no dependency. `namcap PKGBUILD` reports nothing. `namcap` on the built package is for Closing, as in 33.
- **`tools/dev-nested`** passes Camino's stderr through unchanged, and prints `dev-nested: WAYLAND_DISPLAY=wayland-N DISPLAY=:M` each time Camino logs `Xwayland is ready`, so again after a restart. Checked on Xvfb.
- **The `DISPLAY` item is tagged M5,** not M3, by the checklist's rule for items that need a later milestone: nothing reads `Camino::environment` until `camino --session` spawns mate-session with it (34), so no child spawned under a nested run shows `DISPLAY`. Decided with the developer. In M3, 38's X11-tier test that `DISPLAY` isn't set until the root properties exist stands in.
- **The closing checks** are in the wizard [43-closing-checks.sh](43-closing-checks.sh). Run it from a terminal on the host desktop, and `tools/dev-nested` (the installed camino, not `--bin`) in another when it says to. It walks through M3's nested exit from 17 (xterm, xeyes and gtk3-demo on X11; menus and tooltips; `_NET_FRAME_EXTENTS` and `_NET_WORKAREA`; killing Xwayland, with the root properties read before and after and compared) and every `M1`, `M2` and `M3` checklist item. It saves each result as it's given, in `~/.local/state/camino/43-closing-checks.progress`, so a stopped run resumes, and at the end writes them into these notes for the `Close M3 XWayland` commit.
- **Installed (2026-09-30):** `makepkg -si --cleanbuild` built and installed `camino-git r82.069e4cf-1`, with `check()`'s tests, the X11 tier's included, passing. The installed camino under `tools/dev-nested` logged `Xwayland is ready on DISPLAY=:3` and printed `dev-nested: WAYLAND_DISPLAY=wayland-2 DISPLAY=:3`. `namcap PKGBUILD` reports nothing. `namcap` on the built package warns only about ld-linux, librsvg, marco and mate-themes, M2's explained ones, and `xorg-xwayland`, which camino spawns without linking. CI and the Packaging job were last green on d588a4b. The commits since (46 to 48) haven't been pushed, so the first criterion waits for a push and a green run.
- **Closing run (2026-09-30)**, on the installed `camino-git r82.069e4cf-1` under `tools/dev-nested`, run by the developer with Claude driving the commands:
  - A nested run uses its private bus: pass. Camino's `DBUS_SESSION_BUS_ADDRESS` was a `/tmp/dbus-*` bus, not `unix:path=/run/user/1000/bus`.
  - dconf goes to the scratch config: pass. The nested read printed `true` and `dconf/user` was in the scratch `XDG_CONFIG_HOME`. The host read printed nothing, and the scratch directory was gone after Camino exited.
  - The host desktop's systemd environment is untouched: pass. `systemctl --user show-environment` was the same before and after the run.
  - The debug interface answers on the nested bus: pass. `DumpState` listed pluma as `ssd` with extents 1/1/31/1 and `strip_scale` 1. The first `ToggleFrameDebug` answered `true` and drew the red, blue and green outlines labelled ×1, and the second answered `false` and they went.
  - A theme switch while windows are open: pass. pluma was maximised, and caja unmaximised, with gtk3-demo beside them as CSD. BlackMATE, HighContrast, TraditionalOk, NoSuchTheme and Menta each changed both frames at once, with no window unmapped. pluma and its frame filled the 1280x800 output every time, and caja kept its 800x526 client area. `frame_extents` followed the theme, from caja's 1/1/31/1 on Menta to 6/6/34/7 on HighContrast. NoSuchTheme logged one `can't load the Marco theme NoSuchTheme` line and used the built-in theme. On Menta the buttons still prelit, pressed and acted.
    - BlackMATE's title was dark on its dark titlebar. Its title colour blends `gtk:fg[NORMAL]`, and the nested scratch config's GTK theme was still Menta. Setting the nested `gtk-theme` to BlackMATE turned it light at once, so the palette follows a live GTK theme change.
    - A second pluma exits at once, handing its window to the running one, so caja stood in for the second framed window. The checklist item and the wizard now say so.
  - X11 apps map, get frames, take focus and move: pass. xterm, xeyes and gtk3-demo's Application Class demo mapped with Menta frames. Each took focus on a click and moved by its titlebar, xterm and the demo took typing, and xeyes followed the pointer over X windows.
    - gtk3-demo `--run` maps its launcher window beside the demo, on bare Xvfb too, so the second "Application Class" window isn't Camino's.
  - Menus and tooltips sit in the override-redirect layer: pass. The Application Class demo's Preferences menu and its Color submenu opened above the window, with Down, Right and Escape navigating them. A click outside closed the menu, and typing went back to the window. The Builder demo's New tooltip showed above both demo windows and went when the pointer left. xterm's Ctrl+click menu opened above it, shown while the button is held, as on the host.
  - `_NET_FRAME_EXTENTS` and `_NET_WORKAREA` are right: pass. xterm, xeyes and both demos had 1, 1, 31, 1. `_NET_WORKAREA` was 0, 0, 1280, 800 for each of the four desktops, the root's size, and `_GTK_WORKAREAS_D0` the same rectangle.
  - An X11 app's copy and paste to and from a native app: pass. Text went both ways between xterm (with the checklist's clipboard keys) and foot, through the clipboard and the primary selection. With foot focused after copying "new text4", `xclip` on `:3` still printed xterm's "new text3". Once xterm had focus, it printed "new text4".
    - A first try used a plain xterm, which has no Ctrl+Shift+C, so xterm never owned the clipboard. `xclip` then got nothing, not an old value: with foot focused, the Wayland selection Camino had pushed into X earlier is refused to X clients (37's gate).
  - Killing Xwayland brings it back with the root properties republished: pass. After SIGKILL of the nested Xwayland, Camino logged `Xwayland exited, so it's started again on :3 in 500ms`, and it was ready on `:3` 590 ms after the kill. The X windows went and foot stayed. `xprop -root` of `_NET_SUPPORTED`, the desktop atoms, `_NET_WORKAREA` and `_GTK_WORKAREAS_D0` was identical before and after, and a new xterm got a Menta frame (1, 1, 31, 1) and took typing.
  - Frames at an output scale of 2 in a nested run: pass. `DumpState` showed the output at scale 2 and pluma as `ssd` with `strip_scale` 2. Its extents were 1/1/31/1 unmaximised and 0/0/31/0 maximised, as at scale 1. Frame debug labelled the frame ×2 on its edges, the title and icons were sharp, the buttons prelit, pressed and acted, and drag and edge and corner resize worked where the outlines said.
- **The Packaging job (2026-09-30)**, dispatched on 803a58f, failed twice in `check()`, on `a_restarted_xwayland_is_owed_the_wayland_clipboard`. CI was green on the same commit. The restart can lose its display to another X server in the gap before it respawns, and on the runner's two cores the other X11-tier tests take it. That's filed as [49](49-xwayland-display-across-restart.md), which blocks closing M3. The hand run above stands.
