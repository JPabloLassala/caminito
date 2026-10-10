# Edge and corner tiling, with the tile preview

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in and its xdg tests pass; the headless tests' `::x11` halves and `an_x11_configure_request_is_refused_while_the_window_is_tiled` need Xwayland, which doesn't start in the build container, and the checklist item is 173's closing run
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

- [x] Policy tests, written before the code: each side, the top and each corner; `allow-tiling` and `allow-top-tiling` off; the floating geometry restored on untile; a tiled window's titlebar drag untiling it as 144's tests do for maximised windows; the work area changing under a tiled window.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: a drag to each edge showing the preview in the debug dump and tiling on release; the xdg `tiled_*` states sent; an X11 configure request refused while tiled; a drag out untiling.
- [x] A drawing test for the preview, read from the output's pixels: Marco's has no goldens.
- [x] `allow-tiling` and `allow-top-tiling` leave `UNSUPPORTED`; their rows in `docs/parity.md` and the keyless section's edge and corner tiling say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: dragging a native GTK3 window, a GTK4 CSD window and an X11 window to each edge and corner, and back out. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, to settle before building:

- **Tiled as its own state, or half-maximise** (ADR-worthy). Policy has horizontal and vertical maximise, and 144 calls a window maximised on one axis "snapped". A left half is vertical maximise plus a place and a width, but a quarter isn't any maximise. Proposed: a tiled state of its own, beside maximise, as Marco has, with 144's half-maximised windows staying what they are.
- **The preview's look:** Marco draws a theme-coloured rectangle; proposed, the Palette's selected background at a fixed opacity.
- **An edge between two outputs:** whether a drag past the shared edge tiles to the first output's side, or crosses to the second, as Marco decides on X11 with Xinerama.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the size of the edge zones and corners, how a maximised window's top drag interacts with `allow-top-tiling`, and what Marco does with a tiled window's size hints.

## Done (2026-10-10)

- **Triage (no human asked, the recommended options taken):** a tiled state of its own beside maximise, recorded as ADR 0024, with 144's half-maximised windows left as they are (tiling one clears its maximise and keeps its floating geometry); the preview in the palette's selected background, translucent at GTK's default `selection-box-alpha` (64 of 255, which Marco fills at) inside an opaque line of it one pixel wide, and in the normal text colour where the palette selects in its normal background (the neutral palette does), as the Window switcher's selection; at an edge between two outputs the pointer's output decides, as Marco's `calculate_tiling_mode` does with Xinerama: just inside the first's right edge tiles to its right half, just past it to the second's left half.
- **Started from an earlier agent's uncommitted work,** merged onto the newer base (153's `moved` flag and 164's skip list conflicted in `policy.rs`). Kept: `Tile`, `TiledEdges`, `Managed::tile` beside `floating`, `TileTarget` on the grab, the zones, `tile_area`, the tiled `relayout`, and untiling through `set_maximized`. Changed: a move (other than a titlebar drag) holds a tiled window until the pointer is twice the drag threshold away, as Marco's shake threshold, where the earlier work untiled it at the first motion; tiling a half-maximised window keeps its floating geometry and clears its maximise; the frame state comes from `WindowStates::frame_state`, shared by policy and `Mapped`. Most of the policy code predates its tests, so not every test was seen red; the shake threshold and the untiled window's vertical place were written test first.
- **From Marco's source** (`window.c:7380-7629`, `constraints.c:905-980`, `screen.c:1503-1570`): a side's zone reaches from the output's edge to twice `drag-threshold` inside the work area's, and a corner is where it meets the same strip along the work area's top or bottom; the top's zone is the output's top edge down to the work area's top, inclusive, and it maximises with `allow-top-tiling` only if no side zone holds the pointer. Tiling is computed only while the window is neither maximised both ways nor tiled, and never while snapping (Shift). A window tiles only if it can maximise (normal or dialog, not of fixed size) and isn't shaded, and its least size fits half the work area's width and all its height, frame included; Marco checks a corner no harder. A tiled window ignores its size hints but the least size, as a maximised one does. A maximised window's top drag: 144 unmaximises it at the drag threshold, and from then on it's an ordinary move, so letting go at the top maximises it again with `allow-top-tiling` (Marco's re-maximise during the drag, `shaken_loose`, isn't ported).
- **Untiling:** a titlebar drag past `drag-threshold` untiles at once through 144's `begin_titlebar_move`, and any other move (Alt+drag, an xdg `move`, X's `_NET_WM_MOVERESIZE`) after twice it. The window gets its floating size back with the pointer at the same fraction across and down its frame as it was across and down the tile, so the titlebar stays under a titlebar press (144's maximised windows keep their floating top, unchanged). Maximising untiles and unmaximising then goes back to the floating geometry, which is where the move that tiled the window began. A resize leaves the window floating at its tiled size (Marco keeps it tiled and lets its free side move; left for 159, with tile cycling).
- **What each client is told:** xdg's `tiled_*` states for the edges the tile lies along, and the tiled states count as Caminito's sizing, as maximised does; the Frame in the theme's `tiled_left` and `tiled_right` styles for a side tile only, as in Marco, with no vertical resize (window-private.h:446), the extents table growing to 24 states; X configure requests refused through `Managed::constrained`. An X client isn't told a tile in `_NET_WM_STATE` (ADR 0024).
- **Scene:** the preview is drawn in the Windows layer just beneath the window being moved, as Marco lowers its preview beneath the focused window, rather than as a layer over every window, so the moved window isn't tinted; CONTEXT.md's *Scene* says so and gains *Tile*. Marco's 200 ms delay before the preview shows isn't ported: it shows at once.
- **Fixture and defaults:** `allow-tiling` is on by default, as in Marco, in the fixture too; policy's `Config::default()` keeps it off for the older policy tests, and the proptest sequences run with it on. `drag-threshold` is now read once and goes to policy's `Config` as well as the mouse settings.
- **Tests run** (under `dbus-run-session`, with `--skip x11` for the headless ones), before the merge: `caminito-policy` `tests::tiling`, `tests::interactive`, `tests::edge_resistance`, `tests::invariants`, `tests::states`, `tests::requests`, `tests::work_area`, `tests::outputs`, `tests::pointer_focus`, `tests::placement_modes`; `caminito` `tests::tiling`, `settings::translate`, `decoration::`, `window::x11::tests`, `tests::debug`, `tests::frames`, `tests::edge_resistance`, `tests::xdg_shell`, `tests::preferred_scale`, `tests::grabs`, `tests::placement`, `tests::xwayland_windows`; clippy on the workspace (bar the three pre-existing theme lints) and rustfmt. Not run here: every `::x11` half and the X-only tests, as Xwayland doesn't start ("Xwayland starts"), and the checklist item, which needs a Session.

