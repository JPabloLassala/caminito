# Moving a window to a corner, a side, the centre or another monitor

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 148
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's bindings that move the focused window without resizing it ([05](05-v01-marco-feature-cut.md)):

- **`move-to-corner-nw`, `-ne`, `-sw` and `-se`,** and **`move-to-side-n`, `-s`, `-e` and `-w`:** the window's Frame goes against that corner or side of its output's work area.
- **`move-to-center`:** the window goes to the middle of its output's work area.
- **`move-to-monitor-n`, `-s`, `-e` and `-w`:** the window goes to the next output that way, keeping its place relative to the work area if Marco does, and clamped into it.

A maximised or fullscreen window, and one that doesn't allow moving, isn't moved, except by `move-to-monitor-*` if Marco moves those.

## Acceptance criteria

- [ ] Policy tests, written before the code: each corner and side, the centre, against a work area with a panel; each monitor direction with two outputs side by side and one above, and with no output that way; a window bigger than the target work area.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: each binding set in the test (Marco's defaults are `disabled`), and the X11 window's position written back.
- [ ] The thirteen actions' rows in `docs/parity.md` say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: the bindings, set in mate-control-center, on a native and an X11 window, with two monitors of different scales. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **A maximised window moved to another monitor:** proposed, it stays maximised there, as Plasma and GNOME do, if Marco refuses to move it; Marco's behaviour is followed if it moves it.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: whether the corners and sides count the Frame or the client's area, how a window's place on the new monitor is computed, and whether a tiled window ([158](158-edge-and-corner-tiling.md)) keeps its tile.
