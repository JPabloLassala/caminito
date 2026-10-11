# Up, down, the previous workspace, and `wrap-style`

Epic: caminito-v0.1
Type: implementation
Status: resolved: the checklist item's run in a Session is 173's closing run
Milestone: M8
Blocked by: 148
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The Workspace grid ([96](96-workspace-grid.md)) gains its other two directions and the previous workspace, and every direction follows Marco's `wrap-style`.

- **The actions:** `switch-to-workspace-up`, `-down` and `-prev`, and `move-to-workspace-up` and `-down`, with Marco's defaults (Ctrl+Alt+Up and Down for the first two).
- **`wrap-style`:** Marco's three styles, `no wrap`, `classic` and `toroidal`, for every direction, the left and right of [96](96-workspace-grid.md) included. Today the grid doesn't wrap.
- **The previous workspace:** the one active before the current one, whichever way the switch came (a binding, the pager, an activation).
- **`wrap-style` leaves `UNSUPPORTED`.** It's there today, as an example in the translation's tests too.

## Acceptance criteria

- [x] Policy tests, written before the code: up and down in a one-row grid, a full grid and a part-filled one; each wrap style at each edge; `prev` after each kind of switch; `prev` after the count shrinks below it.
- [x] Headless tests through the fixture, on xdg and XWayland windows: the five actions from their bindings; `move-to-workspace-up` carrying the focused window; a `wrap-style` change applying at once.
- [x] `wrap-style` leaves `UNSUPPORTED`; the five actions' and the key's rows in `docs/parity.md` say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: the four directions in a 2 × 2 grid set from the pager, with each wrap style. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section:

- **`classic` and `toroidal` with empty cells,** in a grid whose last row or column is part-filled: where a move into an empty cell lands.
- **`prev` across a switch Marco doesn't count,** such as one done by moving a window.

**For the developer:**

- **What `prev` remembers across a change in the workspace count:** proposed, the previous workspace is forgotten when it's removed, and `prev` then does nothing until the next switch.

## Done (2026-10-10)

- **Tests first:** the grid's wrap tests (`caminito-policy/src/tests/grid.rs`) and policy's (`caminito-policy/src/tests/workspace_moves.rs`: up and down in one row, a full grid and a part-filled one, each wrap style at each edge, `prev` after a binding, the pager, an activation and a window's move, `prev` after the count shrinks and as dynamic workspaces close up, and the two moves) were written before the code and failed to build on the missing `WrapStyle`, `Config::wrap_style` and actions. One then failed on an assertion: it had classic's up and down in one row go nowhere, and Marco's arithmetic takes them to the column before or after, the workspace before or after; the test was wrong and was changed to Marco's. The translation's three `wrap-style` tests were written before the code too and failed to build. The bindings table's test and the headless tests (`caminito/src/tests/workspace_moves.rs`) were written with the bindings, so they weren't seen to fail. The proptest gained the five actions and a grid and wrap change as an operation, and the invariants hold.
- **Empty cells under `classic` and `toroidal` (from Marco's source):** an empty cell is past the edge of its row or column, whichever way the move comes. Since the grid is filled in order from the top left, a row's or a column's workspaces are always at its start. `toroidal` goes round to the other end of the filled part of the same row or column; `classic` goes on to the start (right, down) or the end (left, up) of the filled part of the next or previous row or column, the last leading to the first, and passes over one with no workspace. That's what `meta_workspace_get_neighbor` (workspace.c:830-960) does for a horizontal grid with only its last row part-filled, which is what the switcher writes. Marco's fix-up for a move into an empty cell (workspace.c:921-944) works out workspace numbers as if the grid were filled along its rows, so in a vertical grid it lands elsewhere or nowhere, and in a grid with more than one empty row (keys set by hand, 3 × 3 with four workspaces) classic's up from the first stays put where Caminito goes to the bottom of the last column; Caminito does the same thing in every orientation instead. `Grid::neighbour` returns `None` when a move goes nowhere or back to where it started.
- **`prev` across a switch Marco doesn't count (from Marco's source):** Marco keeps the workspace before on every activation (`meta_workspace_activate_with_focus`, workspace.c:399-400), so the move of a window up or down, which activates the workspace with the window focused (keybindings.c:3560-3576), counts, and so does an activation of a window, the pager's switch, `_NET_CURRENT_DESKTOP` and `prev` itself (so `prev` twice comes back). A move of a window by number (149's) switches nothing and doesn't count. With `dynamic-workspaces` the previous workspace follows its windows as they close up, as the active one does (151), and isn't a switch.
- **What `prev` remembers across a change in the count (for the developer, the proposal taken):** the previous workspace is forgotten when it's removed, or when it's the active one after the change, and `prev` then does nothing until the next switch. Marco keeps a pointer to the removed workspace. When the active workspace is removed (the count shrinks, or dynamic workspaces fall back to the first), the forced switch doesn't set it, since the workspace it left is gone.
- **The moves up and down:** on the focused window, with its transient family, as `move_to_workspace`; the desktop and docks don't move, being on every workspace in Marco. The window keeps the focus and is raised. A sticky window isn't unstuck as Marco does: sticky takes effect later in M8 (17).
- **The bindings' order:** `switch-to-workspace-up` and `-down` follow `-right`, then `move-to-workspace-up` and `-down` (window keys) follow them, and `switch-to-workspace-prev` is last, as `all-keybindings.h` has them; `docs/parity.md` says so.
- **The translation's unsupported-key tests** used `wrap-style` as their example; they use a made-up key, `test-style`, now, so a ticket that honours a key doesn't take the example away.
- **Tests run** (only the ticket's and the modules it touched, under `dbus-run-session`): `caminito-policy`'s `tests::grid`, `tests::workspace_moves`, `tests::workspaces`, `tests::dynamic_workspaces` and `tests::invariants`, all passing (the whole of `caminito-policy` ran once by mistake while checking the build, 298 of 299 passing, the one failure the classic test above); `caminito`'s `tests::workspace_moves`, `tests::bindings`, `input::bindings`, `settings::translate`, `tests::parity`, `tests::ext_workspace` and `tests::dynamic_workspaces`, all passing but their `::x11` tests at "Xwayland starts", because Xwayland doesn't start in this container. So the headless tests' `::x11` halves weren't seen to pass, and the second criterion stays unticked for them. Clippy on `caminito-policy` and `caminito` is clean with the three pre-existing `manual_range_contains` lints in caminito-theme allowed. After merging 164, whose show-desktop skip list met this in the translation's imports and its defaults, the same tests were rerun, with `caminito-policy`'s `tests::show_desktop`, with the same results, and clippy again. The checklist item, *The four directions in a 2 × 2 grid, with each wrap style*, is 173's closing run.
- **The X11 halves, run on 2026-10-11:** in the OrbStack machine `camino` (Arch, aarch64), where Xwayland starts, on master at `9ffebec`: the whole of `caminito` as `tools/check` runs it (`XDG_DATA_HOME=$HOME/.local/share dbus-run-session -- cargo test -p caminito`), 1445 passed, 8 failed, 5 ignored. This ticket's tests passed, `tests::workspace_moves`' two `::x11` tests included, with `tests::bindings`, `input::bindings`, `settings::translate`, `tests::parity`, `tests::ext_workspace` and `tests::dynamic_workspaces`; `cargo test -p caminito-policy --lib` passed too (399). None of the 8 failures is this ticket's: four of `tests::keymap`, which fail before M8 too (at `741fccd`), since libxkbcommon 1.13 writes a group's name as `name[1]`, not `name[Group1]`; `tests::grabs::alt_middle_drag_resizes_from_the_nearest_corner::x11`, which passes at `741fccd` and fails on master every time; and two of `tests::lock` and one of `tests::gamma_control`, which failed under the full suite's load and passed twice alone. The rest of `tools/check` wasn't run.
