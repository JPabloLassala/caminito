# Frame interaction

Type: implementation
Status: open
Milestone: M2
Blocked by: 30
Map: [Camino v0.1](../map.md)

## What to build

Using a frame the way Marco's works ([07](07-decoration-strategy.md)): buttons, the titlebar, resize borders and cursors, plus shade for SSD windows.

- **Hit-testing (07):**
  - a 10px invisible resize border outside the frame, none on the output-touching sides of maximised or tiled frames;
  - Marco's corner resize zones;
  - Fitts's-law button rects from [26](26-theme-geometry.md);
  - the frame's input region follows the rounded-corner shape.
- **Buttons:** per-button hover and press, drawn through the strip cache, with click-and-release semantics: releasing outside the button cancels. Close, maximise and minimise act through policy. The menu, shade, above and stick buttons are drawn and hover, and act once their states exist; the window menu and above and stick come in M8.
- **Titlebar (09):**
  - pressing and dragging past `drag-threshold` starts an interactive move;
  - `action-double-click-titlebar`, `action-middle-click-titlebar` and `action-right-click-titlebar` run on the titlebar, with double-click timed by `double-click`;
  - their values `toggle_maximize`, `toggle_maximize_horizontally`, `toggle_maximize_vertically`, `toggle_shade`, `minimize`, `lower` and `none` all work, so policy gains horizontal and vertical maximise; `menu` waits for M8's window menu.
- **Resize:** dragging a border or corner starts the interactive resize policy already has.
- **Shade (05, 07):** a policy state for SSD windows only, refused for CSD windows. A shaded window shows only its shaded frame, keeps focus, and its client surface leaves the scene.
- **Cursors (13):** the xcursor theme from `cursor-theme` and `cursor-size`, loaded per output at the size times the output's rounded-up scale, downsampled on fractional outputs, and reloaded live. Resize, move and default cursors over the frame, and Camino's own cursors for cursor-shape-v1 clients.

## Acceptance criteria

- [x] Headless tests, written before the code: hover, press and release sequences on each button including a cancelled release; resizing from each border and corner; each titlebar action; shade and unshade; no resize border on the output-touching side of a maximised window.
- [x] The proptest operations include shade and the two new maximise directions, and the invariants hold.
- [x] A unit test of the cursor chosen for each hit-test zone.
- [ ] Nested: the Menta frame's buttons, titlebar double-click, drag-to-move and border resize all work on a GTK3 app, and changing `cursor-size` applies live.
