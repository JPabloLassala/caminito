# Moving a window to a corner, a side, the centre or another monitor

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, and the checklist item's run in a Session and the three decisions to confirm are the developer's, at 173's closing run
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

- [x] Policy tests, written before the code: each corner and side, the centre, against a work area with a panel; each monitor direction with two outputs side by side and one above, and with no output that way; a window bigger than the target work area.
- [x] Headless tests through the fixture, on xdg and XWayland windows: each binding set in the test (Marco's defaults are `disabled`), and the X11 window's position written back.
- [x] The thirteen actions' rows in `docs/parity.md` say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: the bindings, set in mate-control-center, on a native and an X11 window, with two monitors of different scales. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **A maximised window moved to another monitor:** proposed, it stays maximised there, as Plasma and GNOME do, if Marco refuses to move it; Marco's behaviour is followed if it moves it.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: whether the corners and sides count the Frame or the client's area, how a window's place on the new monitor is computed, and whether a tiled window ([158](158-edge-and-corner-tiling.md)) keeps its tile.

## Done (2026-10-11)

- **Triage (no human asked):** the ticket was `ready-for-agent`. The Marco-source questions are settled below, and three decisions are listed for the developer to confirm.
- **From Marco's source** (1.29.0: `keybindings.c:2567-2759`, `window.c:2891-2948`, `screen.c:1973-2003`, `place.c:531-537`, `constraints.c`):
  - **Frame or client's area:** the Frame. `handle_move_to_corner_backend` and `handle_move_to_center` measure `meta_window_get_outer_rect`, the visible frame without the invisible borders.
  - **The place on the new monitor:** `meta_window_transform_to_monitor` takes the client's rectangle relative to the monitor's rectangle, not its work area, and scales its place and its size by the two monitors' sizes, each rounded towards zero. Between monitors of one size the window only moves. The monitor that way is the first that shares that side and overlaps along it (`meta_screen_get_xinerama_neighbor`); with none, nothing happens.
  - **A tiled window:** a corner, a side or the centre leaves it in its tile (`constrain_tiling`). `move-to-monitor-*` tiles it on the new monitor, same tile and same width.
  - **Maximised and fullscreen:** a corner, a side or the centre doesn't move them. `move-to-monitor-*` moves them, and they are maximised or fullscreen on the new monitor, because the constraints take the monitor from the rectangle asked for (`setup_constraint_info`, `constraints.c:417-427`). A window maximised one way moves the other way alone.
  - **A window that doesn't allow moving:** Marco's handlers don't check `has_move_func`. Here the desktop, a dock and a window in a grab aren't moved by any of the thirteen.
- **To confirm, 1: a maximised window moved to another monitor.** It stays maximised there, which is both the proposal and Marco's behaviour. The geometry it goes back to moves with it.
- **To confirm, 2: `move-to-monitor-*` resizes between outputs of different sizes.** The ticket's heading says "without resizing it", and Marco scales the size, so Marco was followed. On 1920 and 1280 wide outputs a 400 wide window becomes 266 wide, and 399 on the way back. The size is kept within the window's hints. If no resize is wanted, `transform_to_output` is the one place to change.
- **To confirm, 3: two places where the ticket was followed, not Marco.** (a) A corner or a side is measured against the work area of the window's output; Marco uses the work area of the whole screen (`meta_window_get_work_area_all_xineramas`), so its window can go to another monitor. (b) After `move-to-monitor-*` the Frame is moved inside the work area, or to its top-left along an axis where it doesn't fit; Marco lets all but a strip of it lie outside (`constrain_partially_onscreen`).
- **Different from Marco, on purpose:** a tile resized by its inner sides is tiled afresh on the new monitor. Marco changes the tile's monitor and leaves the window where it was.
- **Known limits:** a client that refuses the scaled size keeps its own, and its position isn't clamped again, so it can pass the work area's right or bottom. A window wider than the work area goes past its left side for the east side and corners, as in Marco. Only a binding that moves the window up or down brings its top into the work area (`constrain_titlebar_visible`).
- **Where it is:** `caminito-policy/src/policy/move_to.rs`, with `Action::MoveTo(Anchor)` and `Action::MoveToMonitor(Direction)`; `relayout_on` in `policy.rs` lays a moved maximised window out on its new output.
- **TDD:** the policy tests were written first and seen red: the corners, sides and centre; then the windows that stay; then the monitors. The two headless tests were seen red before the bindings were added. Three tests weren't seen red: the two for the desktop and a dock passed before their guard was written, since no other code moved them, and the test added after the review (a side's binding leaves a window under a panel at its height) was written with its fix.
- **Review:** /code-review ran on both axes. Applied: the top clamp only for a move up or down, line numbers for two Marco citations, a desktop case in the tests. Left: the commit history is two commits, so it doesn't show the test-first order.
- **Tests run**, in the OrbStack machine `camino` (Arch, aarch64, Xwayland starts): `caminito-policy` `tests::move_to`, `tests::invariants` (the proptest now runs the thirteen actions), `tests::tiling`, `tests::keyboard_tiling`, `tests::outputs`, `tests::states`, `tests::window_actions`, `tests::work_area`; `caminito` `tests::move_to` (both kinds of window, the X11 position read back after each press), `tests::tiling`, `tests::window_actions`, `tests::bindings`, `input::bindings`, `tests::parity`; rustfmt, and clippy on `caminito-policy` and `caminito`.
- **Not run:** the full test suite and the rest of `tools/check`, at the developer's instruction to run only the relevant tests. The checklist item, which needs a Session with two monitors, is 173's.
