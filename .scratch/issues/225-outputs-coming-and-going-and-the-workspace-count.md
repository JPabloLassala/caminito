# Outputs coming and going, and the workspace count

Epic: per-output-workspaces
Type: implementation
Status: ready-for-agent
Blocked by: 231
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

"Outputs coming and going" in [the spec](../per-output-workspaces/spec.md).

- **An output goes**: each of its windows goes to the Primary output's workspace of the same number (`rehome_windows` moves them to the Primary output's work area today, with no thought for workspaces). They don't return when the output does. If the output was the Active output, the Primary output is.
- **An output appears**, new or returning, disabled then enabled: it starts on workspace 1.
- **A smaller `num-workspaces`** clamps every output's active and previous workspace, and every window.
- **Dynamic workspaces** keep one count: a workspace is in use when any output has a window on that number, an empty one in the middle goes only when it's empty on every output, and the renumbering applies to every output together. `num-workspaces` is written back as now.

## Acceptance criteria

- [ ] Policy unit tests: an output with windows on workspaces 1 to 3 is removed, and each window is on the Primary output's workspace of its number, with the Primary output's active workspace and showing windows as they were apart from the newcomers of that number.
- [ ] The output is added again: it shows workspace 1 and no window has moved back.
- [ ] A shrink of the count with two outputs on different workspaces clamps each.
- [ ] Dynamic workspaces with windows on 1 to 3 of one output and 1 of the other: the count is the same for both; emptying workspace 2 on one output keeps it while the other has a window there, and removes it when neither has, with both outputs' active workspaces renumbered.
- [ ] The property tests' invariants hold through output changes with the key on.
- [ ] With the key off, each behaves as today.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item: unplug the second monitor with windows on several of its workspaces, find them on the same numbers of the first, and plug it back in.
