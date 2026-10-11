# Keyboard tiling and tile cycling

Epic: caminito-v0.1
Type: implementation
Status: resolved: the checklist item's run in a Session is 173's closing run
Milestone: M8
Blocked by: 158
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The tiling bindings, on [158](158-edge-and-corner-tiling.md)'s tiled state ([05](05-v01-marco-feature-cut.md)):

- **`tile-to-side-e` and `-w`,** and **`tile-to-corner-ne`, `-nw`, `-se` and `-sw`,** tile the focused window as a drag to that side or corner would, on the output it's on.
- **A second press** of a binding on a window already tiled there untiles it, if Marco's does.
- **`allow-tile-cycling`:** repeated presses of a side's binding cycle through Marco's sizes for that side.

## Acceptance criteria

- [x] Policy tests, written before the code: each binding's half or quarter; a second press; the cycle of sizes with `allow-tile-cycling` on and off; a window on a second output.
- [x] Headless tests through the fixture, on xdg and XWayland windows: each binding set in the test (Marco's defaults are `disabled`) tiling the focused window, and the xdg `tiled_*` states sent.
- [x] `allow-tile-cycling` leaves `UNSUPPORTED`; the six actions' and the key's rows in `docs/parity.md` say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: the six bindings, set in mate-control-center, on a native and an X11 window, and a side's binding pressed three times. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the cycle of sizes and where it starts, whether corners cycle, what a second press does without cycling, and whether a tile binding unmaximises a maximised window first.

**For the developer:** none.

## Done (2026-10-10)

- **Triage (no human asked):** the ticket was `ready-for-agent` with only Marco-source questions, settled below. The note from 158, a resized tile left floating at its tiled size, was taken in here, as 158's Done section left it.
- **From Marco's source** (`keybindings.c:3177-3254` `handle_toggle_tiled`, `window.c:8205-8282` `meta_window_get_current_tile_area`, `window-private.h:87-96`): the cycle is 1/2, 1/3, 1/4, 1/1, 3/4, 2/3 of the work area's width and then untiled, starting at half on a window untiled or tiled elsewhere. Corners cycle too, the width alone, as Marco's tile area applies the ratio to every tile. Without cycling a second press on a tiled window untiles it, and a press of another binding on a tiled window tiles it there at half. A tile binding on a maximised window unmaximises it and tiles it, and untiling goes back to the geometry from before the maximise. A drag tiles at Marco's `META_TILE_CYCLE_NONE`, as wide as half, so the first press on a drag-tiled window steps it to the half it's at, and the second to a third; ported as is (`TileSize::Uncycled`). The bindings don't depend on `allow-tiling`, as in Marco. A window is tiled on the output it's most on (`meta_screen_get_xinerama_for_window`), and only if `meta_window_can_tile` allows it: it can be maximised (not fullscreen, not of fixed size, a normal window or dialog), isn't shaded, and its least size fits half the work area's width; the drag's check and the binding's are now one `can_tile`, checked against the window's output where Marco checks the pointer's.
- **Widths in floating point,** `(int)(width * ratio)` as Marco's, so a third of 1280 is 426 and two thirds of 1920 is 1280.
- **Resizing a tiled window** (158's leftover): it stays tiled, and only its inner sides move, alone or together, as `can_resize_tiled` (constraints.c:986-1060) frees them; a resize by any other side changes nothing. The tile then keeps its resized geometry, through work area changes too, until it's tiled afresh by a drag or a binding, as Marco's `tile_resized` has it. 158's `resizing_a_tiled_window_leaves_it_floating_at_its_tiled_size` was replaced by tests of this.
- **TDD:** the first policy test (each binding's half or quarter) was seen red; the cycle and second-press tests were written against the first implementation, and checked by breaking the non-cycling branch, which failed two of them. The resize tests were seen red before the change. The headless tests and the settings test were written before the bindings and the key were added (the latter seen failing to compile).
- **`allow-tile-cycling`** wasn't in `UNSUPPORTED` (its other value was what Caminito did), so it only leaves the comment there; it's read into policy's `Config`, on by default. The proptest sequences now run the six actions too.
- **Tests run** (under `dbus-run-session`, with `--skip x11` for the headless ones), before the merge: `caminito-policy` `tiling` (both `tests::tiling` and `tests::keyboard_tiling`), `tests::invariants`, `tests::interactive`, `tests::window_actions`, `tests::states`, `tests::requests`, `tests::work_area`, `tests::outputs`, `tests::edge_resistance`, `tests::placement_modes`; `caminito` `tests::tiling`, `settings::translate`, `input::bindings`, `tests::parity`, `tests::bindings`, `tests::window_actions`, `tests::debug`, `tests::frames`, `tests::edge_resistance`, `tests::xdg_shell`, `tests::grabs` (bar its two X-only tests); clippy on `caminito-policy` and `caminito` (bar the three pre-existing theme lints) and rustfmt. Not run here: every `::x11` half, which fails at "Xwayland starts", and the checklist item, which needs a Session.
- **The X11 halves, run on 2026-10-11:** in the OrbStack machine `camino` (Arch, aarch64), where Xwayland starts, on master at `9ffebec`: the whole of `caminito` as `tools/check` runs it (`XDG_DATA_HOME=$HOME/.local/share dbus-run-session -- cargo test -p caminito`), 1445 passed, 8 failed, 5 ignored, and `cargo test -p caminito-policy --lib` (399 passed). `tests::tiling`'s `each_tiling_binding_once_set_tiles_the_focused_window` and `a_sides_binding_pressed_again_cycles_its_width_then_untiles` passed on both kinds of window, and `a_window_tiled_by_a_binding_is_told_the_edges_it_lies_along`, with `tests::bindings`, `tests::window_actions`, `tests::debug` and `tests::frames`. None of the 8 failures is this ticket's code; they're listed in [148](148-keybindings-from-settings.md)'s Done section. The one in `tests::grabs`, a module this ticket ran, is [157](157-edge-resistance-and-snap-move.md)'s. The rest of `tools/check` wasn't run.
