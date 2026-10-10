# Edge resistance and snap-move

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's edge resistance and snapping during moves and resizes ([05](05-v01-marco-feature-cut.md)). [64](64-work-areas-from-struts.md) left them to M8.

- **Edge resistance:** a window being moved or resized stops for a moment at an output's edge, a work area's edge, and the edges of the other windows on its workspace, then goes on if the pointer keeps going, as Marco's does.
- **Snap-move:** with Shift held during a move or a resize, the window snaps to those edges instead.
- **Every move and resize:** pointer moves, modifier-drags, the titlebar drag after [144](144-drag-to-unmaximize.md) unmaximises, and the moves clients ask for (xdg `move`, X11 `_NET_WM_MOVERESIZE`).
- **The rule is Policy's,** working on the rectangles it already has, with time from `now` where Marco's resistance has a timeout.

## Acceptance criteria

- [ ] Policy tests, written before the code: resistance at each kind of edge, on a move and a resize; going past it; Shift snapping; two outputs side by side; a window on another workspace not counted.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: a drag stopping at the work area's edge by the panel, and going past it; a Shift drag snapping to another window.
- [ ] Their rows in `docs/parity.md` (edge resistance and snap-move have no key; they're in the keyless section) say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: dragging a window to the panel, to the outputs' shared edge and to another window, with and without Shift. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: `edge-resistance.c`'s thresholds and timeouts for each kind of edge, and which edges count.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **Port `edge-resistance.c` or simplify it.** Proposed: port its thresholds and timeouts, and simplify only where Wayland has no counterpart, saying where.
- **Keyboard moves** ([156](156-keyboard-move-and-resize.md)): whether they resist too. Proposed: as Marco does.
