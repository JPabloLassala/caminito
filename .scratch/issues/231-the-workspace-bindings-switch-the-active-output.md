# The workspace bindings switch the Active output

Epic: per-output-workspaces
Type: implementation
Status: ready-for-agent
Blocked by: 230
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

The third of three slices, and the first a user can try: the **Active output** (CONTEXT.md) as far as focus decides it, under "The Active output" in [the spec](../per-output-workspaces/spec.md). Clicks and where a new window opens are ticket 222.

- **Each seat has an Active output**: the focused window's Main output. When a switch leaves no window focused it stays that output. While there is none, the Primary output stands in.
- **The `switch-to-workspace-*` bindings** switch the Active output only: by number, beside, and to the previous one, which is that output's previous.
- **Focus after a switch** is the top window showing on that output, or nothing. It doesn't go to a window on another output because the workspace is empty. A switch of an output the focused window isn't on leaves focus alone.
- **The Workspace popup** opens on the Active output, marks its active workspace, and outlines its windows.
- **The switch sounds** play for the output that switched.
- **Turning the key off** takes the Active output's active workspace for every output, in place of the Primary output's (ticket 230).
- The debug dump shows each output's active workspace and each seat's Active output.

## Acceptance criteria

- [ ] Policy unit tests with two outputs: a binding switches the focused window's output and not the other; a switch to an empty workspace leaves nothing focused and the Active output where it was, and a second switch acts on the same output with the pointer on the other; `switch-to-workspace-prev` goes back on that output only.
- [ ] Focus moves to a window on the other output only when the user focuses one, and the Active output follows.
- [ ] A headless test: the Workspace popup is on the Active output with the pointer on the other.
- [ ] With the key off, the bindings switch every output and focus is as today.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item, in a Session with two monitors: switch each monitor's workspace with the bindings, switch one to an empty workspace twice running, drag a window across, and turn the key off and on with `gsettings`.
