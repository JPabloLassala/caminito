# Packaging and closing M3

Type: implementation
Status: open
Milestone: M3
Blocked by: 42
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
