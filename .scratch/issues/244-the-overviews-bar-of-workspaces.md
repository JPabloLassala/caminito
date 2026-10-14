# The Overview's bar of workspaces

Epic: overview
Type: implementation
Status: ready-for-agent
Blocked by: 241
Spec: [Overview](../overview/spec.md)

## What to build

The bar of the workspaces along the top of the Overview, on one output, under "The bar", "The pointer", "The keys" and "Live content" in [the spec](../overview/spec.md). It builds on ticket 241's Overview. The drag onto a workspace is 245 and two outputs are 246.

- **One row along the top of the output**, in workspace order, whatever the Workspace grid's shape. The layout of the windows takes the area beneath it.
- **Its size** (the developer's decision of 2026-10-13, a start value): the bar's height is 15% of the output's height. Each cell has the output's aspect. When the cells at that height are wider than the output together, they are all made smaller by the same amount to fit.
- **Each cell is a small live picture** of that workspace on this output, with its wallpaper and its windows at their real places. The active workspace's cell is marked.
- **No buttons to add or remove a workspace.** The count is `num-workspaces`'s or `dynamic-workspaces`'s, and the bar follows a change of it while the Overview is open.
- **A click on a cell** switches the output to that workspace. The Overview stays open, and the Workspace slide moves the laid-out windows: the old workspace's layout leaves and the new one's arrives. With the slide off, it is a cut.
- **The workspace bindings** switch the Active output's workspace in the same way, with the Workspace slide and without the Workspace popup, since the seat already has an Interaction.
- After a switch the selection is the new workspace's focused window, or none.
- **A window that only a cell shows** keeps getting frame callbacks, at a reduced rate: 10 a second (the developer's decision of 2026-10-13, a start value). A window in the layout gets them at the output's rate.
- The debug dump shows the bar: each cell's rectangle and workspace, and which is active.

## Acceptance criteria

- [ ] Unit tests of the bar's layout: one workspace, four, and so many that the cells shrink to fit; a grid of two rows gives one row of cells in workspace order.
- [ ] Headless tests that read the dump: the bar's cells, the active one marked, and the windows' layout beneath the bar.
- [ ] A click on a cell switches the workspace, the Overview is still open, and the layout is the new workspace's windows. With `workspace-slide` on, the two layouts are both drawn part-way.
- [ ] A workspace binding switches the workspace with the Overview open, and no Workspace popup shows.
- [ ] A window of a workspace that isn't active shows in its cell, and gets frame callbacks at about 10 a second, read from Caminito's clock.
- [ ] A change of `num-workspaces` while the Overview is open changes the bar.
- [ ] The spec's "The bar" and "Live content" have the two values, and "Not decided" hasn't.
