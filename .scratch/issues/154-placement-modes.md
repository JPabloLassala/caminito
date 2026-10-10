# Placement modes

Epic: caminito-v0.1
Type: implementation
Status: open: the code, tests and docs are in; the headless tests' X11 variants need a run on the developer's machine, where Xwayland starts, and the checklist item is 173's to run
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's placement modes for new windows ([05](05-v01-marco-feature-cut.md)). Today Policy places every window as Marco's `automatic` mode does.

- **`placement-mode`:**
  - `automatic`: as today;
  - `pointer`: centred on the pointer, kept within the work area of the output it's on;
  - `manual`: the new window follows the pointer until a click, or a key, puts it down.
- **`center-new-windows`:** new windows go in the middle of the work area.
- **Windows that aren't placed:** a window the client positions (an XWayland window with a position hint, 05), a transient placed on its parent, and a fullscreen or maximised window keep today's rules, as in Marco.

## Acceptance criteria

- [x] Policy tests, written before the code: each mode for a first window, a window on a busy workspace, and one near an output's edge; `center-new-windows` with each mode; the windows that aren't placed.
- [x] Headless tests through the fixture, on xdg and XWayland windows: each mode from its key; a manual placement put down with a click, and with Return; a key change applying to the next window.
- [x] `placement-mode` and `center-new-windows` leave `UNSUPPORTED`; their rows in `docs/parity.md` say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: each mode with a native and an X11 window, on two outputs. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section:

- **How manual placement takes the pointer and the keyboard:** which keys move it, put it down, and cancel it, and what a cancel leaves (the window where the automatic mode would put it, or where the pointer is).
- **Other outputs:** whether a manual placement can cross to another output, and which output `pointer` and `center-new-windows` use.
- **What Marco's `center-new-windows` does to a transient.**

**For the developer:** none.

## Done (2026-10-10)

- **Tests first:** `caminito-policy/src/tests/placement_modes.rs` was written before the code, against an API that didn't exist yet (`PlacementMode`, `Config::placement_mode` and `center_new_windows`, `set_pointer`, `map_window_with`, `placing`, `nudge_grab`, `cancel_grab`), then made to pass. The headless tests are `caminito/src/tests/placement.rs`, through `on_both_kinds!`; their `::xdg` variants pass, and their `::x11` variants fail here only because Xwayland doesn't start in this environment (as about 238 others do). They need a run on the developer's machine.
- **Manual placement's keys, from Marco's `process_keyboard_move_grab` (keybindings.c:1480-1627):** the arrows, the keypad's arrows and diagonals, and `hjkl` move the window by 10 pixels, or 1 with Ctrl or Shift (Marco's Shift is a 1-pixel snap move; there's no edge resistance yet). Escape cancels; any other key, Return among them, puts the window down where it is; modifiers are let by. A click puts it down too, as a button ends Marco's keyboard move. The pointer moves the window throughout. Unlike Marco, Caminito doesn't warp the pointer to the window's centre or after a nudge: the window keeps its offset from the pointer.
- **What a cancel leaves:** the window where it was first placed, centred on the pointer as it was when the window mapped, as Marco's Escape goes back to `grab_initial_window_pos`, not where the automatic mode would have put it.
- **Other outputs:** a manual placement can cross to another output, as a Marco keyboard move only keeps the window partly on screen. `pointer` and `manual` use the output the pointer is on, keeping the window within its work area. `center-new-windows` uses the output the automatic mode does, the focused window's, else the primary output (Marco's current Xinerama).
- **`center-new-windows` and the other modes:** as in `place.c`, `pointer` and `manual` come first, so `center-new-windows` changes only the automatic mode. Centred windows ignore the windows already there; one that doesn't fit the middle of the work area goes to the output's corner with no cascade, as Marco's origin placement.
- **What Marco's `center-new-windows` does to a transient:** nothing. A dialog with a parent is centred over it, and a parentless dialog or splash screen on the output, in every mode, as before 154.
- **A window that maps maximised or fullscreen:** placed by the automatic mode as if `center-new-windows` were off, for the geometry it goes back to, and never placed by hand, which keeps today's rule. Policy learns of these states at map through `Policy::map_window_with` and `MapStates`, which now applies them as `map_pending` did.
- **Too big for its work area:** as Marco's pointer placement skips its auto-maximise, a window placed at the pointer isn't maximised for being too big; the automatic mode still maximises one.
- **Where Policy gets the pointer:** `Policy::set_pointer`, which the compositor calls just before each map, so Policy still reads no input itself. Manual placement starts only when the seat has no other grab; the compositor holds the pointer for it in the refresh pass, and ends it where it began when the session is locked or something else holds the pointer.
- **Marco's default for `center-new-windows` is on**, which Caminito now follows in a Session. `Config::default` and the test fixture keep it off, since the existing Policy and headless tests were written for the first-fit tile; the tests of centring set it on.
- **Not run:** the checklist item, which needs a Session with two monitors and goes to 173's closing run; the X11 headless variants; `clippy --workspace -D warnings` is clean apart from caminito-theme's three pre-existing `manual_range_contains` lints. One locker test (`a_killed_locker_comes_back_…`) failed once in the loaded full run and passed three times alone.
- **Triage:** the ticket was `ready-for-agent` with the open questions above for the implementing agent; each was settled from Marco's source as recorded here, with no human asked.
