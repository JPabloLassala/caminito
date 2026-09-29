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
- [ ] The two checklist items are written.
- [ ] Closing M3: nested, xterm, xeyes and a GTK3 app with `GDK_BACKEND=x11` map, get Marco frames, take focus and move; their menus and tooltips sit in the override-redirect layer; `_NET_FRAME_EXTENTS` and `_NET_WORKAREA` are right; killing Xwayland brings it back on the same display with the root properties republished; the X11 tier passes in CI; the checklist results are in a `Close M3 XWayland` commit.
