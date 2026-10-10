# Edge resistance and snap-move

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in and tested; the headless tests' X11 halves are unrun where Xwayland wouldn't start, and the checklist run in a Session is the developer's, at 173's closing run
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

- [x] Policy tests, written before the code: resistance at each kind of edge, on a move and a resize; going past it; Shift snapping; two outputs side by side; a window on another workspace not counted.
- [x] Headless tests through the fixture, on xdg and XWayland windows: a drag stopping at the work area's edge by the panel, and going past it; a Shift drag snapping to another window.
- [x] Their rows in `docs/parity.md` (edge resistance and snap-move have no key; they're in the keyless section) say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: dragging a window to the panel, to the outputs' shared edge and to another window, with and without Shift. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: `edge-resistance.c`'s thresholds and timeouts for each kind of edge, and which edges count.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **Port `edge-resistance.c` or simplify it.** Proposed: port its thresholds and timeouts, and simplify only where Wayland has no counterpart, saying where.
- **Keyboard moves** ([156](156-keyboard-move-and-resize.md)): whether they resist too. Proposed: as Marco does.

## Done (2026-10-10)

- **Tested first:** `caminito-policy/src/tests/edge_resistance.rs`, starting from a move stopping at the work area's edge by a panel (red against the old `grab_motion`), then each kind of edge on a move and a resize, going past, a frame rather than its window meeting the edge, nothing holding a window moving away, Shift snapping on a move and a resize, two outputs side by side (with the right side, then the left), a window on another workspace, a covered edge and a desktop window. Headless: `caminito/src/tests/edge_resistance.rs`, through Alt+drag on both kinds of window: stopping at the work area's edge under a layer-shell top bar and going on 32 past, and a Shift drag snapping to another window.
- **Marco's thresholds, from `edge-resistance.c` (master, lines 342-350):** 16 px towards a window's edge, 32 towards a Xinerama (an output's) or screen edge, 0 moving away from any edge, so nothing holds a window leaving one. Every timeout is 0 ms, so the rule is by distance alone and `grab_motion` takes no `now`. Snapping is to the nearest aligned edge of any kind at any distance, with the 8 px slip guard (lines 518-529); a move then shifts by the smaller change of its two sides each way (lines 1176-1216).
- **Which edges count:** the work areas' edges (Marco's screen edges are the screen less the struts, its Xinerama edges the outputs' inner edges; with Caminito's whole-side reservations (64) both are each output's work area); the frames of the other windows showing on the active workspace, less desktops, menus and splash screens (lines 33-39); a dock's frame has no edges of its own (its strut's are the work area's) but covers those below it; window edges are clipped to the outputs' bounding box, and the parts a window higher in the stack covers don't count (lines 1016-1092).
- **Port or simplify (for the developer to confirm):** ported, as proposed. Simplified only where Caminito differs: the edges are recomputed on each motion rather than cached at the grab's start, which differs only when another window moves mid-grab, and then follows it; Marco's exception for resizing a window with size increments doesn't arise, since policy's size hints have none (XWayland's increments are *later* in `docs/parity.md`); a side that snaps past the opposite one during a resize leaves the size to the hints' clamp, as Marco leaves it to its constraints.
- **Keyboard moves (for the developer to confirm):** they resist too, as Marco does, but Marco's keyboard path (stop at each edge crossed, no threshold) belongs with the arrow keys, which are 156's; this ticket ports the pointer path. X11's keyboard `_NET_WM_MOVERESIZE` is driven by the pointer today, so it resists and snaps as any drag does.
- **Every move and resize goes through `Policy::grab_motion`,** so pointer and modifier drags, the titlebar drag after 144's unmaximise, xdg `move`/`resize` and X11's `_NET_WM_MOVERESIZE` all get it. `grab_motion` takes `snap`, which `caminito/src/input/grab.rs` reads from Shift on each motion, as Marco reads it from each motion event; the proptest sequences draw it at random.
- **Two older tests changed with the behaviour:** `frames::dragging_each_border_and_corner_resizes_from_it` now Alt+drags its window to the middle of the output first, since its top border's 30 px drag ended within 32 of the output's top; `preferred_scale::a_window_that_resizes_with_its_scale_gets_each_scale_once_as_its_dragged_across_and_back`'s snapshot enters the other output one 100 px step later each way, where the window is held at the shared edge.
- **Parity row:** edited in place under *To do in M8* to say honoured, to keep the edit local while other M8 tickets change that table; moving it under a heading of its own is left to 173's pass.
- **Not run here:** the headless tests' `::x11` halves, since Xwayland doesn't start in this environment ("Xwayland starts"), along with the ~238 other tests that need it; and the checklist item, which needs the developer's two monitors and a Session.
