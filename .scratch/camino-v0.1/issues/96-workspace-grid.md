# The workspace grid

Type: implementation
Status: open
Milestone: M6
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

The workspace switcher shows the workspaces in rows and columns, and its rows preference changes the grid for Camino too ([06](06-v01-protocol-and-integration-set.md)'s *Pager grid layout* row, [09](09-gsettings-mapping.md), [17](17-milestone-slicing.md)).

- **The schema:** `org.mate.Camino` at `/org/mate/camino/`, with the three `workspace-layout-*` keys as 09 has them, installed by the top-level Meson project and compiled by the package's hook ([16](16-packaging.md)). It's Camino's first schema. `locker-command` joins it in M7.
- **Settings:** the three keys are read on the settings thread and applied live, as the Marco keys are ([29](29-settings-thread.md)). A development run without the schema installed keeps the defaults and says so once.
- **The grid:** a pure function from the count, the orientation, the rows and the columns to each workspace's column and row, mirroring `_NET_DESKTOP_LAYOUT` and libwnck's `try_set_workspace_layout`, with 0 meaning "derive it from the other".
- **ext-workspace:** `coordinates` come from the grid in place of [66](66-ext-workspace.md)'s single row, still always two values, and change live with the keys and with `num-workspaces`.
- **Policy:** moving to the workspace above, below, left and right goes by the grid, where policy already has those moves.
- **XWayland:** `_NET_DESKTOP_LAYOUT` on the root follows the grid, if [38](38-x-connection-and-root-properties.md) publishes it.
- **The carried patch:** `mate-panel-camino`'s Wayland pager places workspaces by coordinates when all of them have coordinates. Under Camino (`MATE_WM_NAME=Camino`) its rows preference writes Camino's keys.
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [ ] Unit tests for the grid, written before the code: one row, two rows of an odd count, columns given and rows derived, vertical fill, and more cells than workspaces.
- [ ] Headless snapshot tests: four workspaces with `workspace-layout-rows` 2 have coordinates (0,0), (1,0), (0,1), (1,1); changing the key sends the new coordinates with one `done`.
- [ ] The schema is installed by Meson, and `camino-git` builds and passes `namcap` with it.
- [ ] `mate-panel-camino` builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [ ] The checklist has the item, and it was run in a Session: two rows set in the pager's preferences show as two rows, and `gsettings` shows the key written. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).

## Notes

- **The count and the names** in the pager's preferences are [97](97-pager-count-and-names.md).
- **X11 MATE** keeps the pager's own rows key: the patch's write is gated (ADR 0011).
