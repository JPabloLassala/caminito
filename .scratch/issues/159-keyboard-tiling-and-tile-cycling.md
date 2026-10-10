# Keyboard tiling and tile cycling

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 158
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The tiling bindings, on [158](158-edge-and-corner-tiling.md)'s tiled state ([05](05-v01-marco-feature-cut.md)):

- **`tile-to-side-e` and `-w`,** and **`tile-to-corner-ne`, `-nw`, `-se` and `-sw`,** tile the focused window as a drag to that side or corner would, on the output it's on.
- **A second press** of a binding on a window already tiled there untiles it, if Marco's does.
- **`allow-tile-cycling`:** repeated presses of a side's binding cycle through Marco's sizes for that side.

## Acceptance criteria

- [ ] Policy tests, written before the code: each binding's half or quarter; a second press; the cycle of sizes with `allow-tile-cycling` on and off; a window on a second output.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: each binding set in the test (Marco's defaults are `disabled`) tiling the focused window, and the xdg `tiled_*` states sent.
- [ ] `allow-tile-cycling` leaves `UNSUPPORTED`; the six actions' and the key's rows in `docs/parity.md` say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: the six bindings, set in mate-control-center, on a native and an X11 window, and a side's binding pressed three times. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the cycle of sizes and where it starts, whether corners cycle, what a second press does without cycling, and whether a tile binding unmaximises a maximised window first.

**For the developer:** none.
