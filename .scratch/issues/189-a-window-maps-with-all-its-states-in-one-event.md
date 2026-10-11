# A window maps with all the states it asked for in one event, and Policy has the one list of them

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
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

- [ ] Policy tests with `TestWindow`, written before the code: a window that maps with each of `above`, `below`, `sticky` and `active` has that state after the map, and one that maps with all seven is sent its geometry once.
- [ ] A Policy table test, written before the code: each `StateRequest` on a `MapStates`, with what it leaves and whether it asks for a new configure.
- [ ] `grep -n "match request" caminito/src/window/lifecycle.rs` finds nothing.
- [ ] `tests::xdg_shell`, `tests::xwayland_windows`, `tests::placement`, `tests::window_actions`, `tests::window_menu`, `tests::xfce_foreign_toplevel` and `tests::focus_stealing` pass with no test changed.
- [ ] `cargo test -p caminito-policy` passes.
- [ ] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- **The order inside the map.** Today the four states are set after the window is placed and focused. Say whether any goes before (a sticky window is on every workspace as it's placed, or not), and that the result is the same as now.
- **`active` beside `focus`.** A window that asked to be active is activated whatever its focus claim says. Say whether the two stay two fields.
- **The seat and the time** of the managed window's request, which `activate` needs.
