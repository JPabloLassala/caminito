# Policy answers what a Pending window is configured with before it maps

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

A Pending window's first configure says what the window will get when it maps: the bounds to fit in, and its size if it asked to be maximised or fullscreen. Policy decides all of that at the map. Before the map the compositor predicts it, in `configure_pending` (`caminito/src/window/lifecycle.rs`):

| What | The compositor's prediction | Policy at the map |
| --- | --- | --- |
| The output | `placement_bounds` (`state.rs`): the first output the focused window's geometry overlaps, or the primary | `place` (`caminito-policy/src/policy.rs`): the parent's output for a dialog, the pointer's in the pointer placement modes, or `placement_output`, the one the focused window's frame overlaps most |
| The functions | `pending_functions`, through Policy's `functions()` (177) | `Managed::functions()` |
| The frame extents | `pending_extents`, which builds the `FrameState` itself: "a new window is focused" | Policy builds the `FrameState` (ADR 0012) |

The two rules for the output aren't the same rule, and nothing keeps them alike: 154 added the pointer modes to `place` and not to `placement_bounds`. By reading, not run:

- **Fullscreen with no output named.** The configure's size is the bounds, the work area less the frame's extents (`fullscreen_size`'s `.or(bounds)`). Policy then gives the window the whole output. With a panel on the output, the client draws its first buffer at the wrong size. `tests::xdg_shell`'s `fullscreen_before_mapping_starts_fullscreen` has no panel, so it can't see it.
- **The bounds are another output's** for a dialog whose parent is on another output, and for a window in a pointer placement mode with the pointer on another output.

This was decided in an architecture review on 2026-10-11. Policy doesn't hold Pending windows: the compositor keeps them, and Policy answers a question. A Pending window is still one Caminito doesn't manage yet (`CONTEXT.md`).

## What to build

- **One question on Policy, with no state kept.** The compositor gives the facts it has before the map: the window's type, its size hints, its parent, whether it'll want a frame, and the `MapStates` it has asked for. Policy answers with what the configure says: the bounds, the size if it'll map maximised or fullscreen, and those two states if the window has the function for them. `PreMapConfigure` moves to Policy, or Policy's type takes its place.
- **One choice of output.** One function in Policy says which output a new window goes on, from its type, its parent, its states and the pointer. `place` and the answer both call it. A window that maps maximised or fullscreen keeps the automatic choice, as now (154). A window with a position of its own (`requested_position`) keeps `place`'s rule; the answer uses the automatic choice for it, since the position isn't known before the map.
- **The frame extents come as ADR 0012 has them.** The compositor gives the question a function from a `FrameState` to extents, and Policy builds the `FrameState`. "A new window is focused" is then Policy's line. ADR 0012 gets a line for the window that isn't managed yet.
- **The pointer.** The compositor tells Policy where the pointer is before it asks, with `set_pointer`, as it does before the map; or the question takes the pointer.
- **The compositor's predictions go:** `placement_bounds`, `pending_functions` and `pending_extents`. `configure_pending` asks Policy and hands the answer to the window's adapter.
- **The XWayland window that asks for its frame extents before it maps** (`xwayland/mod.rs`, the caller of `functions_before_managed`) asks Policy too, with this question or a smaller one beside it, and `functions_before_managed` goes. If it can't, the function stays with that one caller.

Left alone: the output a window's first buffer is scaled for, `main_output_until_drawn` (`window/mod.rs`): the parent's, or the one under the pointer. It's a third choice of output, which can differ from the bounds' with the automatic placement, and a managed window that isn't drawn yet uses it too (`output.rs`). It waits for a run on two outputs of different scales.

[189](189-a-window-maps-with-all-its-states-in-one-event.md) changes `Pending` and `MapStates`. Neither ticket blocks the other; the one that merges second follows the first.

## What the user sees change

- A window that starts fullscreen on an output with a panel draws its first buffer at the output's size.
- A dialog's bounds are those of its parent's output, and in a pointer placement mode a new window's are those of the pointer's output.

## Acceptance criteria

- [ ] Headless tests through the fixture, written first and run before the code changes:
  - with a panel that reserves an edge, a toplevel that asks for fullscreen with no output before it maps is first configured with the output's size;
  - with two outputs of different sizes, a dialog whose parent is on the second is first configured with the second's bounds;
  - with two outputs and `placement-mode` pointer, a window is first configured with the bounds of the output the pointer is on.

  If one passes before the change, the Done section says what made it right.
- [ ] Policy table tests, written before the code, for the choice of output: a dialog with a parent, a dialog with none, each pointer mode, the automatic mode, and a window that asks to map maximised in a pointer mode.
- [ ] A proptest invariant in `caminito-policy/src/tests/invariants.rs`: for a window that asks to map maximised or fullscreen, the size in the answer is the size Policy sends it at the map, when nothing changed between the two. An exception that holds on purpose is written into the rule and the Done section.
- [ ] `grep -rn "placement_bounds\|pending_extents\|pending_functions" caminito/src` finds nothing.
- [ ] `grep -rn "functions_before_managed" caminito/src` finds nothing, or the Done section says why its one caller stays.
- [ ] ADR 0012 has the new line.
- [ ] `tests::xdg_shell`, `tests::placement`, `tests::xwayland_windows`, `tests::preferred_scale` and `tests::decoration` pass; a snapshot that changes is one of the cases above, and the Done section names it.
- [ ] `cargo test -p caminito-policy` passes.
- [ ] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- **The pointer:** `set_pointer` before the question, or an argument of it.
- **The shape of the facts:** a struct with a function for the extents, or a small trait. It's one adapter either way, with `TestWindow`'s side in the tests.
- **An xdg toplevel that asks for fullscreen on an output Policy doesn't have,** which today falls back to the bounds.
- **What "nothing changed between the two" means** in the invariant: the focus, the pointer, the outputs and the config.
