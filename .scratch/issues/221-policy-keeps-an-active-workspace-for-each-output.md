# Policy keeps an active workspace for each output

Epic: per-output-workspaces
Type: implementation
Status: ready-for-agent
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

The first of three slices that were one ticket. A change of shape with no change of behaviour: nothing a user or a client sees differs after it, and there is no key yet.

- **Policy keeps an active and a previous workspace per output**, in place of the one `active` and `previous` (`caminito-policy/src/policy.rs`). Every switch sets them all, so they are always equal. This is what Spanning workspaces will be: a mode of the one code path, not a second path ([ADR 0032](../../docs/adr/0032-each-output-has-its-own-workspaces.md)).
- **Policy's own readers of `self.active` ask by output or by window** where the question has one: whether a window is showing, the workspace a sticky window is shown on, a new window's workspace, the unstick target, edge resistance, Show Desktop, the switcher's candidates, the Window menu's moves, dynamic workspaces' renumbering and the count's clamp. The answer is the same as today for every one.
- **The compositor's `active_workspace()` call sites** (the scene, hit-testing, Xwayland's stacking and `_NET_CURRENT_DESKTOP`, ext-workspace, the Workspace popup, the rename dialog, the debug dump) ask what they mean: the windows showing, or one output's active workspace. `SceneReads` follows.
- **An output added** takes the active workspace the others have, and one removed takes its entry with it.

## Acceptance criteria

- [ ] Every existing test passes, with no test's expectation changed. Tests may change only where they call an API whose shape changed.
- [ ] The property tests gain an invariant that every output's active workspace is the same, through switches, output changes, dynamic workspaces and a change of the count.
- [ ] `SCENE_QUERIES` (`caminito-policy/src/tests/invariants.rs`) names the queries the scene now makes.
- [ ] No reader is left that takes one output's active workspace to stand for all without saying so in its name or a comment, so that ticket 230 finds each place that has to choose.
- [ ] `tools/check` passes.

## For the implementing agent to settle

Recorded in the Done section:

- the shape of policy's per-output state, and the queries that replace `active_workspace()`;
- how tests written against the one active workspace keep running without each being rewritten.
