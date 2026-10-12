# Moves, sticky windows, families and activation, per output

Epic: per-output-workspaces
Type: implementation
Status: ready-for-agent
Blocked by: 230
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

"Moving and activating" in [the spec](../per-output-workspaces/spec.md), with the sticky and family rules of "The model".

- **`move-to-workspace-N` and the beside moves** move a window within its own output's workspaces. The beside moves switch that output with the window.
- **The Window menu's workspace items** are relative to the window's output's active workspace.
- **`move-to-monitor-*`** puts the window on the target output's active workspace.
- **A sticky window** is on every workspace of its Main output, and dragged across is sticky on the other. Unsticking puts it on its output's active workspace.
- **A transient family** is on the workspace of its root's Main output: a dialog dragged to another output shows and hides with its root.
- **Activating a window that isn't showing** switches that window's output to its workspace and focuses it, from wlr-foreign-toplevel's `activate`, a client's activation request and policy's own `activate`. No other output changes.

## Acceptance criteria

- [ ] Policy unit tests with two outputs for each rule above, among them: a dialog on the second output hides when its root's output switches away and the second hasn't; activation of a hidden window on the second output leaves the first output's active workspace alone; a beside move on the second output switches only the second.
- [ ] The Window menu's items for a window on the second output name moves from the second output's active workspace.
- [ ] A headless test: `activate` over wlr-foreign-toplevel on a hidden window of the other output brings it into view there.
- [ ] With the key off, each behaves as today.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item: a Find dialog dragged to the other monitor follows its editor's workspace; a sticky window stays through its monitor's switches.
