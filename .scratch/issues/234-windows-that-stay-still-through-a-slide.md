# Windows that stay still through a slide

Epic: workspace-slide
Type: implementation
Status: ready-for-agent
Blocked by: 233
Spec: [Workspace slide](../workspace-slide/spec.md)

## What to build

"What slides" in [the spec](../workspace-slide/spec.md): the windows that are the same on both workspaces don't move.

- **A sticky window** stays still, and the others slide beneath or over it as the stack has them.
- **The window carried by a `move-to-workspace-*` binding** or by the Window menu's workspace moves stays still while the rest slide.
- **A window being dragged** at the time of the switch stays under the pointer.
- **A focused fullscreen window** slides like any other, in the place the Scene gives it, so the panel shows as it leaves.

## Acceptance criteria

- [ ] Headless tests, read part-way: a sticky window's drawn rectangle is its own while another window's is offset; the same for a carried window, by binding and by the Window menu; the same for a window in an interactive move.
- [ ] A fullscreen window's drawn rectangle is offset part-way, and the panel is drawn.
- [ ] The debug dump says which windows a slide moves and which it leaves.
