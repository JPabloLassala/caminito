# The Workspace slide in a real Session, by hand

Epic: workspace-slide
Type: implementation
Status: open: the checklist's items are in, and the run in a Session is the developer's, after 234 and 239 are resolved
Blocked by: 234, 235, 236, 239
Spec: [Workspace slide](../workspace-slide/spec.md)

## What to do

Run the Workspace slide's items of [checklist.md](../../docs/testing/checklist.md) in a Session on the tty backend, with one monitor and with two.

## Acceptance criteria

- [x] [checklist.md](../../docs/testing/checklist.md) has the items: switch each way with the bindings and the pager; hold a binding across three workspaces; carry a window; keep a sticky window in view; switch one monitor of two, key on and key off; turn `workspace-slide` off with `gsettings`.
- [ ] The slide is smooth at the monitor's refresh rate, with no frame of the old workspace after its end.
- [ ] The length and the curve feel right, or the developer changes them and records it in the spec.

## Done (2026-10-13)

- **Met: the checklist's items.** [checklist.md](../../docs/testing/checklist.md) has two items under "The panel", tagged M9: *The Workspace slide on one monitor* and *The Workspace slide on two monitors*. They have each thing the first criterion lists, and more from the spec: a jump and a diagonal from the pager, a wrap, a switch back part-way, a window being dragged, each of the three keys that the minimise animation shares, the lock, Spanning workspaces, a straddling window and dynamic workspaces.
- **Not met, and the developer's: the two criteria of the run.** If the slide is smooth at the refresh rate, and if the length and the curve feel right, are for a person to see in a Session on the tty backend. No agent ran them. The one-monitor item's "Expected" says where to record a change of the length or the curve.
- **The blockers aren't resolved.** 234 is open on the Window menu's carried window, which is the developer's to decide, and 239 isn't built. The items were written from the spec, so step 6 of the one-monitor item expects what 239 builds: until then, a workspace that the held key wraps back to jumps from one side to the other. No item has a step for the Window menu's moves, which switch no workspace today (234).
- **Not checked against the dump:** the items name `workspace_slides` and its members from `caminito/src/tests/workspace_slide.rs`. The key combinations are the defaults that the checklist's other items use.
- **What was run:** nothing. The change is to the checklist and to this ticket, with no code, so no test was run and no code review was made.
