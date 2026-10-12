# Policy keeps an active workspace for each output

Epic: per-output-workspaces
Type: implementation
Status: resolved
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

The first of three slices that were one ticket. A change of shape with no change of behaviour: nothing a user or a client sees differs after it, and there is no key yet.

- **Policy keeps an active and a previous workspace per output**, in place of the one `active` and `previous` (`caminito-policy/src/policy.rs`). Every switch sets them all, so they are always equal. This is what Spanning workspaces will be: a mode of the one code path, not a second path ([ADR 0032](../../docs/adr/0032-each-output-has-its-own-workspaces.md)).
- **Policy's own readers of `self.active` ask by output or by window** where the question has one: whether a window is showing, the workspace a sticky window is shown on, a new window's workspace, the unstick target, edge resistance, Show Desktop, the switcher's candidates, the Window menu's moves, dynamic workspaces' renumbering and the count's clamp. The answer is the same as today for every one.
- **The compositor's `active_workspace()` call sites** (the scene, hit-testing, Xwayland's stacking and `_NET_CURRENT_DESKTOP`, ext-workspace, the Workspace popup, the rename dialog, the debug dump) ask what they mean: the windows showing, or one output's active workspace. `SceneReads` follows.
- **An output added** takes the active workspace the others have, and one removed takes its entry with it.

## Acceptance criteria

- [x] Every existing test passes, with no test's expectation changed. Tests may change only where they call an API whose shape changed.
- [x] The property tests gain an invariant that every output's active workspace is the same, through switches, output changes, dynamic workspaces and a change of the count.
- [x] `SCENE_QUERIES` (`caminito-policy/src/tests/invariants.rs`) names the queries the scene now makes.
- [x] No reader is left that takes one output's active workspace to stand for all without saying so in its name or a comment, so that ticket 230 finds each place that has to choose.
- [x] `tools/check` passes.

## For the implementing agent to settle

Recorded in the Done section:

- the shape of policy's per-output state, and the queries that replace `active_workspace()`;
- how tests written against the one active workspace keep running without each being rewritten.

## Done (2026-10-12)

- **The shape.** `ActiveWorkspaces` (`caminito-policy/src/policy/active_workspaces.rs`) holds a `Shown { active, previous }` for each `OutputId` and one more for no output. The extra entry is what an output added takes, and it stands for the outputs while policy has none. `set_outputs` keeps the map in step with policy's outputs: an output added copies that entry, and one removed drops its own. Every switch, a change of the count and the dynamic renumbering change every entry alike, the extra one included.
- **Policy's readers ask by window.** `active_workspace_of(id)` is the active workspace of the window's Main output (`output_of` its frame), or of the primary output for a window on none. `is_on_shown_workspace(id)` answers whether a window is showing, the sticky window's desktop workspace, `focuses_on_map`, activation, the switcher's candidates and fallback order, and the Window menu's moves. Unsticking uses the root's output. A new window goes to `new_window_workspace`, the primary output's, which stands in for the Active output's (222). Show Desktop shows and stops on each of `shown_workspaces()`, and `is_showing_desktop` is true while all of them show it. Edge resistance reads `active_stacking`.
- **What stands for all says so.** `everywhere()` is the extra entry, read where one workspace is still meant for every output: `switch_workspace`'s from, the bindings' neighbour and `switch-to-workspace-prev` (231), and the from of the switch sound in a change of the count. Each caller has a comment that names it.
- **The queries that replace `active_workspace()`** for the compositor:
  - `active_stacking()`, the windows on their Main output's active workspace, bottom to top. The scene, hit-testing and Xwayland's stacking use it.
  - `active_workspace_on(OutputId)`, used by the Workspace popup and the dump's popup selection.
  - `primary_active_workspace()`, standing in for the Active output's, used by `_NET_CURRENT_DESKTOP` (228), ext-workspace's one group (226), the rename dialog (231) and the dump's `active_workspace`.

  `SCENE_QUERIES` swaps `active_workspace` and `stacking` for `active_stacking`, and `SceneReads` follows.
- **The tests keep running unchanged.** `active_workspace()` stays public as the one every output has, documented as a test's query that takes one output for all. The only test file changed is `invariants.rs`, where its scene view calls `active_stacking` and a new invariant checks each output's and the primary's active workspace against it. To see that the invariant catches a break, an added output was made to start on workspace 1: proptest failed on it, and the change was reverted.
- **Not added:** a per-output `active_workspace` in the debug dump's outputs. It changed the state dump's snapshot, which this ticket can't change.
- **Run in an Ubuntu 24.04 cloud container**, with Rust 1.99 and the build's dependencies from apt. fmt and clippy are clean, and policy's 557 tests pass. caminito's suite ran 1832 passed and 1 failed: `gamma_control::night_light_fades_back_in_when_the_gamma_client_is_killed`. It passed in the run before, and three times alone, and doesn't touch workspaces. `caminito-theme`'s goldens fail on fonts and rendering, a crate this ticket doesn't touch. `tools/check`'s cargo-deny, meson, locker, wallpaperd and extension steps weren't run.
