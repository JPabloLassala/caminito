# The Overview's bar of workspaces

Epic: overview
Type: implementation
Status: resolved
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

- [x] Unit tests of the bar's layout: one workspace, four, and so many that the cells shrink to fit; a grid of two rows gives one row of cells in workspace order.
- [x] Headless tests that read the dump: the bar's cells, the active one marked, and the windows' layout beneath the bar.
- [x] A click on a cell switches the workspace, the Overview is still open, and the layout is the new workspace's windows. With `workspace-slide` on, the two layouts are both drawn part-way.
- [x] A workspace binding switches the workspace with the Overview open, and no Workspace popup shows.
- [x] A window of a workspace that isn't active shows in its cell, and gets frame callbacks at about 10 a second, read from Caminito's clock.
- [x] A change of `num-workspaces` while the Overview is open changes the bar.
- [x] The spec's "The bar" and "Live content" have the two values, and "Not decided" hasn't.

## Done (2026-10-14)

- **The bar** (`caminito/src/overview/bar.rs`) is a pure function of the output's size and the workspace count, so a grid of two rows gives one row of cells. The bar is a band 15% of the output's height. The cells are inside it with 12 px of padding, have the output's aspect, and are made smaller alike when they don't fit. So a cell is 24 px less than 15% high: I took "the bar's height" as the band. 247 checks the look in a Session.
- **A cell** shows that workspace's windows on the output, minimised ones aside, at their real places over the wallpaper (`mate-background` alone), on a black back where the wallpaper doesn't cover. The bar is above the layout, so a vertical slide goes under it. The active cell has a 3 px mark in the theme's selection colour, as the tile preview has (`selection_color`, shared now).
- **Policy keeps the Overview's workspace.** When it changes, by a click on a cell (`overview_switch`) or by a binding, the list is the new workspace's, and the selection is the seat's focus if it's listed, otherwise none. A click on a cell with the left button switches. Another button on a cell does nothing. A click between the cells is on the empty area and closes the Overview.
- **The workspace bindings that switch** (`switch-to-workspace-N`, left, right, up, down, previous) work with the Overview open. They go through `run_action`, whose Workspace popup policy refuses under an Interaction. The `move-to-workspace-*` bindings are still swallowed. They move the focused window, which is closer to 245's drag. My choice.
- **The Workspace slide runs under the Overview.** A slide records whether it started under the Overview. Opening the Overview cuts a slide that started without it, as before, and closing the Overview cuts one that started under it, until 243. The scene draws each workspace's layout where the slide has that workspace (`workspace_shifts`), the wrap of 239 too. The slide goes the grid's way, as a binding's does, not the bar's row. The spec doesn't say which, so I chose this.
- **A click during a slide** acts on the new layout's final places, as 243 has policy's state ahead of the drawing. The review noted that a click can then hit a window that isn't drawn there yet. I left it, for 243 and 247.
- **The reduced rate** is a deadline holder of its own: every 100 ms of Caminito's clock it sends the frame callbacks of the windows that only the bar shows, and `post_render` sends them none. "Only the bar shows" is: the output is their main output, and they're in none of the layouts, the leaving one included (from the review). A minimised window of another workspace is among them, although no cell shows it. That is my choice, since full rate for a window nobody sees is worse.
- **The dump** has the bar's cells (workspace, rectangle, active), the layout's `area`, the Overview's `workspace`, and `sliding`: each window of a layout that a slide draws away from its place. The scene and the dump take the shifts from one method (`overview_shifts`).
- **Tests:** policy's (9 new, and an op and an invariant in the proptest) were written before the code but first run after it. The bar's unit tests and the headless tests came with the code. To check that the headless tests can fail, `244-break.sh` breaks the frame skip, the slide under the Overview, the bindings and the cells' windows on purpose. Each break failed its tests (1, 4, 1 and 6).
- **What ran:** `caminito-policy`'s whole suite (721), and `caminito`'s tests filtered on `overview`, `workspace_slide`, `deadlines`, `debug`, `state_dump`, `switcher`, `minimi`, `tile`, `xdg_shell`, `drag_icons`, `per_output`, `workspace_popup` and `scene` (439), with fmt and clippy. Not the full suite, by the standing rule. Not run in a real Session: that's 247's.
- **Merged with 243's transition**, which came into `master` while this was built. The bar fades in and out with the labels, as 243's ticket asked once 244 is in. Each layout is drawn by 243's `overview_drawn_windows`, with the slide's shift added to each slot, so a transition and a slide can be drawn together. The transition dump reads the active layout. During the closing's frames the windows that only the bar showed keep the reduced rate until the Overview is gone. No test reads the bar's fade. After the merge: `caminito-policy`'s whole suite (721), and `caminito`'s tests filtered on `overview`, `workspace_slide`, `deadlines`, `debug`, `state_dump`, `scene`, `switcher`, `minimi`, `tile`, `panel`, `layer`, `xdg_shell`, `drag_icons`, `per_output`, `workspace_popup` and `settings` (737), with fmt and clippy at `-D warnings`.
- **From the review, left as it is:** the `usize` for a workspace, as the rest of the repo has it. The `Vec` that `overview_bar_only_windows` builds at each frame and deadline. The extra dump fields, which the tests read.
