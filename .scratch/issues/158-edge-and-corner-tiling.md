# Edge and corner tiling, with the tile preview

Epic: caminito-v0.1
Type: implementation
Status: needs-triage
Milestone: M8
Blocked by: 157
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's tiling by drag ([05](05-v01-marco-feature-cut.md)): a window dragged to an output's side fills that half of the work area, to its top fills all of it, and to a corner fills that quarter.

- **A tiled state in Policy:** which half or quarter a window is tiled to, and its floating geometry to come back to, as maximised windows keep theirs in `Managed::floating`.
- **The keys:** `allow-tiling` turns side and corner tiling on, and `allow-top-tiling` the top edge's maximise, as in Marco.
- **The tile preview:** while the dragged pointer is at an edge or corner, a translucent rectangle shows where the window will go, drawn as a layer of the Scene. Letting go there tiles the window; moving away hides the preview.
- **What a tiled window tells its client:**
  - an xdg toplevel gets the `tiled_*` states for the edges it touches, so GTK drops its rounded corners and shadows ([07](07-decoration-strategy.md));
  - an SSD window's Frame is drawn in the theme's tiled styles where the theme has them;
  - an XWayland window's configure requests are refused while it's tiled, as for a maximised one.
- **Untiling:** dragging a tiled window by its titlebar past `drag-threshold` untiles it and keeps it under the pointer, the way [144](144-drag-to-unmaximize.md) does for a maximised window. This changes 144's titlebar-drag path, which is resolved: the same entry point now also untiles.
- **Edge resistance** ([157](157-edge-resistance-and-snap-move.md)) and tiling share the edge: the order in which they act is Marco's.

## Acceptance criteria

- [ ] Policy tests, written before the code: each side, the top and each corner; `allow-tiling` and `allow-top-tiling` off; the floating geometry restored on untile; a tiled window's titlebar drag untiling it as 144's tests do for maximised windows; the work area changing under a tiled window.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: a drag to each edge showing the preview in the debug dump and tiling on release; the xdg `tiled_*` states sent; an X11 configure request refused while tiled; a drag out untiling.
- [ ] A drawing test for the preview, read from the output's pixels: Marco's has no goldens.
- [ ] `allow-tiling` and `allow-top-tiling` leave `UNSUPPORTED`; their rows in `docs/parity.md` and the keyless section's edge and corner tiling say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: dragging a native GTK3 window, a GTK4 CSD window and an X11 window to each edge and corner, and back out. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, to settle before building:

- **Tiled as its own state, or half-maximise** (ADR-worthy). Policy has horizontal and vertical maximise, and 144 calls a window maximised on one axis "snapped". A left half is vertical maximise plus a place and a width, but a quarter isn't any maximise. Proposed: a tiled state of its own, beside maximise, as Marco has, with 144's half-maximised windows staying what they are.
- **The preview's look:** Marco draws a theme-coloured rectangle; proposed, the Palette's selected background at a fixed opacity.
- **An edge between two outputs:** whether a drag past the shared edge tiles to the first output's side, or crosses to the second, as Marco decides on X11 with Xinerama.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the size of the edge zones and corners, how a maximised window's top drag interacts with `allow-top-tiling`, and what Marco does with a tiled window's size hints.
