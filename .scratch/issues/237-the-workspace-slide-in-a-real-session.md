# The Workspace slide in a real Session, by hand

Epic: workspace-slide
Type: implementation
Status: ready-for-human
Blocked by: 234, 235, 236, 239
Spec: [Workspace slide](../workspace-slide/spec.md)

## What to do

Run the Workspace slide's items of [checklist.md](../../docs/testing/checklist.md) in a Session on the tty backend, with one monitor and with two.

## Acceptance criteria

- [ ] [checklist.md](../../docs/testing/checklist.md) has the items: switch each way with the bindings and the pager; hold a binding across three workspaces; carry a window; keep a sticky window in view; switch one monitor of two, key on and key off; turn `workspace-slide` off with `gsettings`.
- [ ] The slide is smooth at the monitor's refresh rate, with no frame of the old workspace after its end.
- [ ] The length and the curve feel right, or the developer changes them and records it in the spec.
