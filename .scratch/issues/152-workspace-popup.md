# The workspace popup

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M8
Blocked by: 150, 108
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's workspace popup ([05](05-v01-marco-feature-cut.md), [17](17-milestone-slicing.md)): a workspace switch made with a binding shows the Workspace grid, with the workspaces' names and the new one selected, while the binding's modifier is held.

- **When it shows:** on a `switch-to-workspace-*` or `move-to-workspace-*` direction binding ([150](150-workspace-up-down-prev-and-wrap-style.md)), and further presses while the modifier is held move through the grid as the bindings do. Releasing the modifier closes it.
- **What it shows:** each workspace's cell, in the grid's rows and columns, with its name ([97](97-pager-count-and-names.md)), the active one marked, as Marco's popup does. A small outline of each workspace's windows is drawn if Marco's popup draws them.
- **Where and how:** centred on the output the pointer is on, as the Window switcher is ([108](108-window-switcher.md)), and drawn the same way: a layer of the Scene over everything a client draws, its chrome drawn with cairo in the GTK theme's Palette.
- **When it doesn't:** while the session is locked, during a move or resize, and while the Window switcher is open.
- **The debug dump** has the popup's output, cells and selection once it shows.

## Acceptance criteria

- [x] Headless tests through the fixture, written before the code: a held Ctrl+Alt with Right and Down shows the popup on the pointer's output with the right cell selected, a release closes it, and nothing shows while locked or with the switcher open. They read the popup's rectangles from the debug dump, as 108's do.
- [x] A drawing test: the grid's cells for a 2 × 2 and a part-filled grid, and a name in its cell, read from the output's pixels. Marco's popup has no goldens.
- [x] The rows in `docs/parity.md` for the direction actions say what shows, with these tests.
- [x] The checklist has an item, tagged M8: switching with Ctrl+Alt+arrows in a 2 × 2 grid with named workspaces, on each output. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, decided when the ticket is done and listed under *What was decided* for confirmation, as [108](108-window-switcher.md) did:

- **Its look,** with no goldens to match: the cell size, the selected cell's mark, and whether windows are outlined.
- **Whether a switch from the pager or over D-Bus shows it.** Proposed: no, only a binding, since the pager already shows the change.
- **Its timing** against the switcher's 90 ms delay: shown at once, as Marco does, or after the same delay, so that a quick switch shows nothing.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: which bindings show the popup (whether `switch-to-workspace-N` and `-prev` do), whether it follows `move-to-workspace-*`, and what closes it besides the release.

## What was decided

The ticket left these to the developer, and they were decided when it was done, without them. Each is the developer's to confirm or change.

- **Its timing:** shown at once, as Marco does, with no delay. A quick Ctrl+Alt+arrow shows the popup from the press to the release, so it can flash.
- **A switch from the pager or over D-Bus** shows nothing, as proposed: only a binding's key opens it.
- **A cell** is a small picture of the workspace over its name. The picture has the shape of the popup's output. The pictures of all the cells together are a third of the output's width, as Marco's are, but none is narrower than 96 logical pixels, a floor Marco doesn't have, so that a name has room. They shrink below that only for the popup to fit in nine tenths of the output, as the Window switcher's does.
- **Each name is in its own cell,** under the picture, ellipsised at its end. Marco shows the selected workspace's name alone, under the grid.
- **The selected cell** is filled as the Window switcher's selected cell is: the palette's `Selected` background, or a quarter of the text colour where that's the popup's background. Its name is in the palette's selected text colour on the first. Marco draws a 2 pixel line round it.
- **Windows are outlined,** since Marco's popup draws them: each window showing on the workspace as a rectangle in the popup's background inside a line, where its frame is. Minimised windows, the desktop and docks are left out, and so is a window on every workspace but on the active one, as in Marco. No icon is drawn in an outline, which Marco does.
- **A picture shows the popup's output only.** Marco's shows the whole X screen, every monitor, in the screen's shape. A window on another output isn't in the picture.
- **The pointer** reaches what's under the popup until a press, which closes the popup and reaches no client. The Window switcher's popup takes the pointer's place; this one doesn't.

## Done (2026-10-11)

- **Tested first, in part.** The first headless test (Ctrl+Alt held with Right and Down, the selected cell, the release) was seen to fail before any code: it didn't build without the scene's layer. The other 33 tests were written after the code. Turning the popup's give-way rule off made `an_open_popup_closes_as_the_session_locks` and `nothing_shows_during_a_move` fail, so those can.
- **Two of the criterion's tests pass without the popup's own guards,** as 108's did: while locked no binding runs, and the open Window switcher has the arrow keys. They hold the behaviour. The same is true of `nothing_shows_during_a_keyboard_move`.
- **The dump's `selected` is the active workspace,** by design: policy has no state for the popup, since each press switches at once. That the mark is drawn on that cell is the pixel tests' to show (`the_selection_follows_each_press`).
- **The name test reads ink, not letters:** a named workspace's band has pixels that aren't the background, a workspace named with a space has none, and it gets some when its key changes.
- **Which bindings show it (Marco's source):** the eight to the workspace beside, `switch-to-workspace-left`, `-right`, `-up`, `-down` and the four `move-to-workspace-*` of the same directions (`handle_workspace_switch_or_move`, keybindings.c:3505-3586). `switch-to-workspace-N`, `-prev` and `move-to-workspace-N` activate the workspace with no popup (keybindings.c:2360-2393, 3418-3440).
- **It follows `move-to-workspace-*` (Marco's source):** a move opens it in a mode of its own (`META_GRAB_OP_KEYBOARD_WORKSPACE_MOVING`), where each further direction moves the window too, whichever of the two actions the key is bound to, since Marco looks the key up with the modifiers the popup opened with (keybindings.c:2828-2882). A popup a switch opened switches, with Shift added or not. Caminito shows none for a move with no window to move, or with the desktop or a dock focused, which 150 doesn't move.
- **What closes it (Marco's source):** the release of the binding's modifier, any key that isn't a binding to the workspace beside, which is then swallowed and runs no binding of its own (keybindings.c:2888-2898, 1416-1423), and a press of a pointer button (display.c:1859-1903). Marco closes on the release of the binding's "primary" modifier, Alt before Ctrl before Shift; Caminito closes on the release of any of the binding's modifiers, Shift among them for a move, as the Window switcher commits on any of its own.
- **It shows where the switch goes nowhere,** at an edge of a grid that doesn't wrap, as Marco's does, whose neighbour past an edge is the workspace itself. With one workspace it doesn't (keybindings.c:3516-3518), and a binding with no modifier shows none, as Marco ends that grab as it begins.
- **During a move or resize the binding still switches,** as it did before this ticket, and only the popup is held back. Marco's binding does nothing there, since its grab can't begin. Not changed: it's 150's behaviour.
- **Where the code is:** `caminito/src/workspace_popup/` (`mod.rs` for when it shows and closes, `layout.rs` for the rectangles, `draw.rs` for cairo), `SceneLayer::WorkspacePopup` beside the switcher's, and the keys in `bindings_stage`. The Window switcher's `Holding`, `popup_output`, `draw_background`, `fill_selected` and `draw_text` are shared with it and still live in `switcher`; a module for what the two popups share wasn't made.
- **Not done:** Ctrl+Alt+Fn switches VT before the popup sees the key, as with the Window switcher open, so a popup open at that moment stays until the keys are released or the host takes the keyboard. A pointer resize has no test of its own; it's the same `window_grab` the move's test holds.
- **Checked in the OrbStack machine** (aarch64), not with `tools/check`: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`, both clean, and `cargo test --workspace --no-fail-fast` under `dbus-run-session`. Of `caminito`'s 1487 tests 5 fail there, and the same 5 fail on master: four in `tests::keymap` and `alt_middle_drag_resizes_from_the_nearest_corner::x11`. `caminito-theme`'s `goldens` fail on aarch64, as on master. The rest pass, the popup's 34 on both kinds of window, with Xwayland. cargo-deny, the palette helper, the helpers' tests and the extension weren't run. `152-test.sh` runs the tests there.
- **The checklist item,** *The Workspace popup shows the grid while Ctrl+Alt is held*, wasn't run: it's 173's closing run.
