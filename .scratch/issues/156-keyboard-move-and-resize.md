# Keyboard move and resize

Epic: caminito-v0.1
Type: implementation
Status: resolved: the checklist item's run in a Session is 173's closing run
Milestone: M8
Blocked by: 148
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`begin-move` (Alt+F7) and `begin-resize` (Alt+F8), with Marco's keyboard grab ([05](05-v01-marco-feature-cut.md)).

- **The grab:** the arrow keys move or resize the window by Marco's step, a modifier makes the step finer if Marco's does, Return or a click puts it down, and Escape puts it back where it was.
- **Resizing:** the first arrow picks the edge, as in Marco.
- **The pointer** follows the window as Marco warps it, and a pointer motion moves or resizes it too.
- **X11's keyboard directions:** `_NET_WM_MOVERESIZE` directions 9 and 10 start the same grab. [39](39-x-window-properties-and-requests.md) started a pointer move or resize from them and left the keys to this ticket.
- **Maximised and fullscreen windows** aren't moved or resized, as `begin_move` refuses them today ([144](144-drag-to-unmaximize.md)).

## Acceptance criteria

- [x] Policy tests, written before the code: a move and a resize stepped with each arrow; Return; Escape putting the window back; a maximised window refused.
- [x] Headless tests through the fixture, on xdg and XWayland windows: Alt+F7 and Alt+F8 and the arrows; a `_NET_WM_MOVERESIZE` with direction 9 and 10; the keys not reaching the client during the grab.
- [x] The two actions' rows in `docs/parity.md` say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: Alt+F7 and Alt+F8 on a native and an X11 window, ended with Return and with Escape, and the pointer's place after each. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the step sizes, the finer step's modifier, how the pointer is warped at the start and during the grab, which edge a resize starts on, and what a click ends.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **With the Window switcher:** whether Alt+Tab during a keyboard grab is refused, as during a pointer grab ([108](108-window-switcher.md)).
- **With keyboard-shortcuts-inhibit:** proposed, a client that inhibits shortcuts gets Alt+F7 too, as with every other binding, and a grab already started keeps the keys.

## Done (2026-10-10)

- **Tests first:** `caminito-policy/src/tests/keyboard_grab.rs` was written before the code, against an API that didn't exist yet (`begin_keyboard_move`, `begin_keyboard_resize`, `grab_key`, `GrabKey`, `GrabStep`, `take_pointer_warp`, `keyboard_grab`, `grab_edges`, `Action::BeginMove` and `BeginResize`), then made to pass. The invariants proptest now takes the two actions and the grab's keys. The headless tests are `caminito/src/tests/keyboard_grab.rs`, through `on_both_kinds!`, and the `_NET_WM_MOVERESIZE` 9 and 10 tests in `window_properties.rs`, which are X11 only. Their xdg variants pass; the X11 ones fail here only because Xwayland doesn't start, and need a run on the developer's machine. That the keys don't reach the client is checked on an xdg window alone, since only a Wayland test client reports the keys it gets.
- **Steps, from `process_keyboard_move_grab` and `process_keyboard_resize_grab` (keybindings.c:1480-2015):** 10 pixels (`NORMAL_INCREMENT`), 1 with Ctrl (`SMALL_INCREMENT`). Shift is Marco's snap: a 1-pixel step that goes on to the next edge the window meets its way (`find_nearest_position` with `only_forward`), or nowhere if there's none. Marco's size increments don't arise, as policy's size hints have none.
- **Edges (157's note):** a key's step stops at any edge of the work areas or the other windows it would cross, whatever way it holds, and the next key goes on past it, as `apply_edge_resistance`'s `keyboard_op` branch; `Edges::for_move` and `for_resize` now take `By::Pointer` or `By::Keys`.
- **Which edge a resize starts on:** none. The first arrow picks the edge it points at (Up the top, Down the bottom, Left the left, Right the right), and an arrow across the edge being resized later picks its own edge in place of it, as `process_keyboard_resize_grab_op_change`. The pointer's first motion picks the edge or corner it goes towards, as `update_resize`. A keypad diagonal, which Marco's resize doesn't take, puts the window down.
- **The pointer:** warped as `warp_grab_pointer` does it, onto the screen: to the middle of the window's frame at the start of a move or resize, after each step of a move, and to the middle of the edge or the corner being resized once one is picked and after each step. The grab goes on from the warp, so the pointer moves or resizes the window from there. Unlike Marco, the motion that picks a resize's edge doesn't also resize the window: Marco warps the pointer to the edge and loses that change at its next motion anyway.
- **What ends it:** Return or any other key but a modifier puts the window down, Escape puts it back as it was (place and size), and a click puts it down, as a button press ends Marco's keyboard grab. Caminito ends it on the button's release, as its other window grabs, and the press reaches no client. Modifiers reach the client as in 154; every other key is the grab's.
- **Refused:** a maximised or fullscreen window, a desktop or a dock, as `begin_move` refuses them, and for a resize a window Marco gives no `has_resize_func` (a fixed size). A keyboard grab doesn't start over another grab of the seat.
- **Manual placement (154) is a keyboard move**, as in Marco (window.c:2323), so its keys go through `grab_key` and get the edges and Shift's snap; it keeps 154's choice not to warp the pointer, since the window is already centred on it. `nudge_grab`, `cancel_grab` and `PlacementKey` are gone.
- **With 158's tiling:** a keyboard move tiles nothing, as Marco tiles only on a button release; its first step untiles a tiled window round the pointer, as a pointer move does once it shakes it loose (`tests::tiling::a_keyboard_move_untiles_…`). Escape is `cancel_grab`, which 158's cancelled move uses too, and now puts a resize's size back as well. A keyboard resize of a tiled window follows 159's rule, moving only the tile's inner sides.
- **For the developer to confirm:** *with the Window switcher*, Alt+Tab during a keyboard grab is refused: Tab is a key of the grab, which puts the window down, and the switcher doesn't open, as during a pointer grab (108) and as Marco's grab takes every key. *With keyboard-shortcuts-inhibit*, as proposed: a client that inhibits shortcuts gets Alt+F7 and Alt+F8, as every other binding, and a grab already started keeps the keys.
- **Not run:** the full suite (by instruction); the X11 headless variants; the checklist item, which goes to 173's closing run. Run: `caminito-policy`'s `tests::keyboard_grab`, `placement_modes`, `interactive`, `edge_resistance`, `invariants`, `states`, `shade`, `pointer_focus`, `window_actions`, `actions` and `switcher`, `tiling` and `keyboard_tiling`; `caminito`'s `tests::keyboard_grab`, `tests::placement`, `tests::bindings`, `input::bindings`, `tests::window_properties`, `tests::grabs`, `tests::edge_resistance`, `tests::tiling` and `tests::parity`, before and after merging the integration branch. `clippy` is clean apart from caminito-theme's three pre-existing `manual_range_contains` lints.
- **Triage:** the ticket was `ready-for-agent`; the questions for the implementing agent were settled from Marco's source as above, and the two for the developer take the proposed or recommended answer, with no human asked.
- **The X11 variants, run on 2026-10-11:** in the OrbStack machine `camino` (Arch, aarch64), where Xwayland starts, on master at `9ffebec`: the whole of `caminito` as `tools/check` runs it (`XDG_DATA_HOME=$HOME/.local/share dbus-run-session -- cargo test -p caminito`), 1445 passed, 8 failed, 5 ignored, and `cargo test -p caminito-policy --lib` (399 passed). `tests::keyboard_grab`'s eleven tests passed, its five `::x11` variants included, and `tests::window_properties`' three for `_NET_WM_MOVERESIZE` directions 9 and 10. None of the 8 failures is this ticket's code; they're listed in [148](148-keybindings-from-settings.md)'s Done section. The one in `tests::grabs`, a module this ticket ran, is [157](157-edge-resistance-and-snap-move.md)'s. The rest of `tools/check` wasn't run.
