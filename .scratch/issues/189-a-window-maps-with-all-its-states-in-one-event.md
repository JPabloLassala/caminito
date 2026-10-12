# A window maps with all the states it asked for in one event, and Policy has the one list of them

Epic: caminito-v0.1
Type: implementation
Status: resolved
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

A Pending window keeps the states it asks for until it maps. Three places in `caminito/src/window/lifecycle.rs` each have the list of them:

- **`Pending`** has a field for each: `maximized`, `fullscreen`, `above`, `below`, `sticky`, `active` and `focus`.
- **`window_state_request`** matches `StateRequest` twice: once to call Policy for a managed window, once to write `Pending`.
- **`map_pending`** gives three of them to Policy in `MapStates`, then calls `set_above`, `set_below`, `set_sticky` and `activate` for the other four. Each of those runs `apply()` again, so a window that maps is laid out up to five times.

A new state that a window can ask for before it maps needs a field in `Pending`, its first value, an arm in the second match, the destructuring in `map_pending`, and a field of `MapStates` or a call after the map. The compiler asks for the arm only.

This was decided in an architecture review on 2026-10-11. There's no change in behaviour here.

## What to build

- **`MapStates` has all of them.** It gets `above`, `below`, `sticky` and `active`. `map_window_with` applies them all, and runs `apply()` once. What each means doesn't change: a state the window has no function for is one it maps without (177), and a window that asked to be active is activated as it maps.
- **`StateRequest` moves to `caminito-policy`,** with both matches:
  - Policy gets one function for a managed window's request, which is the first match as it is now.
  - `MapStates` gets one for a Pending window's request, which is the second match. It says whether the window is to be configured again, as the arms' `return false` say today: only maximised and fullscreen change the configure, and a Pending window has nothing to minimise.
- **`Pending` is the window and one `MapStates`.** `window_state_request` calls one of the two functions and has no match of its own. `window_activation_requested` writes the claim into the `MapStates`.

Left alone: `configure_pending` and what it works out, which are [190](190-policy-answers-what-a-pending-window-is-configured-with.md)'s. Neither ticket blocks the other; the one that merges second follows the first.

## Acceptance criteria

- [x] Policy tests with `TestWindow`, written before the code: a window that maps with each of `above`, `below`, `sticky` and `active` has that state after the map, and one that maps with all seven is sent its geometry once.
- [x] A Policy table test, written before the code: each `StateRequest` on a `MapStates`, with what it leaves and whether it asks for a new configure.
- [x] `grep -n "match request" caminito/src/window/lifecycle.rs` finds nothing.
- [x] `tests::xdg_shell`, `tests::xwayland_windows`, `tests::placement`, `tests::window_actions`, `tests::window_menu`, `tests::xfce_foreign_toplevel` and `tests::focus_stealing` pass with no test changed.
- [x] `cargo test -p caminito-policy` passes.
- [x] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- **The order inside the map.** Today the four states are set after the window is placed and focused. Say whether any goes before (a sticky window is on every workspace as it's placed, or not), and that the result is the same as now.
- **`active` beside `focus`.** A window that asked to be active is activated whatever its focus claim says. Say whether the two stay two fields.
- **The seat and the time** of the managed window's request, which `activate` needs.

## Done (2026-10-11)

- **Tests first.** `caminito-policy/src/tests/map_states.rs` was written before the code and failed to compile against it. It has one test each for above, below, sticky and active, a test that a window mapping with every state is sent its geometry once, and the `StateRequest` table on a `MapStates`. Then the single apply was disabled to see the geometry test fail: it was sent twice.
- **`MapStates` has eight fields, not seven.** `demands_attention` went in too, since `Pending` is now the window and one `MapStates`.
- **One apply.** `map_window_with` sets a `holding` flag, so `apply()` returns at once until the map and every state are done, and then applies once. The setters keep their own `apply()` for every other caller. Deferring it changes no outcome:
  - The activation judge reads `focus` and `focus_time`, which `map` sets directly.
  - `switch_workspace` picks its default focus from the stack, not from `mru`.
  - Restacking is order-independent.
  - The final apply lays out a maximised window again for its focus.
- **The order inside the map is as before.** The window is placed and focused, then maximised or fullscreen, then above, below, sticky, the demand for attention and the activation request. None goes before placement. A sticky window placed first would also be focused on a workspace it didn't ask for, which is a change in behaviour.
- **Only the states asked for are set.** This is the one change in behaviour. `map_pending` used to call `set_sticky(id, false)` for every window. A transient of a sticky window, which `map` makes sticky with its parent, then unstuck its whole family and moved it to the active workspace. Calling setters for unasked states would also have changed `map_window` (`a_sticky_window_takes_its_family_along`). `set_above(false)` and `set_below(false)` on a new window were no-ops anyway.
- **`active` and `focus` stay two fields.** `focus` is the xdg-activation claim that `focus-new-windows` judges as the window maps. `active` is a `_NET_ACTIVE_WINDOW` request, judged by its source after the map. An X window has both a `NoToken` claim and maybe a request.
- **The seat and the time.**
  - A managed window's request takes them as arguments: `Policy::request_state(seat, id, request, now)`. Caminito passes `SEAT` and its clock, as before.
  - A Pending window's activation is judged for `last_seat`, the seat the map focuses for, at the map's `now`. Caminito has only `SEAT`, which is always `last_seat`.
- **What ran.**
  - `cargo fmt --all --check` passed.
  - Clippy on `caminito` and `caminito-policy` passed with `-A clippy::manual_range_contains`.
  - `cargo test -p caminito-policy` passed: 536 tests.
  - The seven named caminito groups passed unchanged.
  - Caminito's full suite passed 1753 tests and failed 1. `tiling::a_sides_binding_pressed_again_cycles_its_width_then_untiles::x11` is flaky on the base commit too: 3 failures in 6 runs.
- **What failed, outside this ticket.** These are in `caminito-theme`, which this ticket doesn't touch, and were run on Ubuntu in a cloud session, not on the Arch machine:
  - Clippy 1.97 flags `manual_range_contains` there.
  - `caminito-theme`'s goldens differ by a few pixels.
- **What wasn't run.** `cargo deny`, the palette helper, the meson helpers, the locker and the VS Code extension. The full `tools/check` wants Arch.
