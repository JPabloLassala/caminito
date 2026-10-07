# The workspace grid

Type: implementation
Status: resolved
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

- [x] Unit tests for the grid, written before the code: one row, two rows of an odd count, columns given and rows derived, vertical fill, and more cells than workspaces.
- [x] Headless snapshot tests: four workspaces with `workspace-layout-rows` 2 have coordinates (0,0), (1,0), (0,1), (1,1); changing the key sends the new coordinates with one `done`.
- [x] The schema is installed by Meson, and `camino-git` builds and passes `namcap` with it.
- [x] `mate-panel-camino` builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [ ] The checklist has the item, and it was run in a Session: two rows set in the pager's preferences show as two rows, and `gsettings` shows the key written. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).

## Notes

- **The count and the names** in the pager's preferences are [97](97-pager-count-and-names.md).
- **X11 MATE** keeps the pager's own rows key: the patch's write is gated (ADR 0011).
- **Resolved on 2026-10-06 with the last criterion unticked:** the checklist has the item, *The workspace switcher's rows are Camino's grid*, and it wasn't run: there was no Session to log into where this was done. [106](106-packaging-and-closing-m6.md) runs it. Nothing here was seen in a pager, so the patch is checked only by its build.
- **Where it is:**
  - the grid is `camino_policy::Grid`, in `camino-policy/src/grid.rs`, made from the count and `WorkspaceLayout`, which is the three keys;
  - policy holds the `WorkspaceLayout` with the count and the names, and `Policy::workspace_grid` is what ext-workspace reads in the refresh pass;
  - the schema is `data/org.mate.Camino.gschema.xml`, and the tests read it through a link in `tests/schemas/`.
- **Written before the code:** the grid's unit tests, policy's two and the translation's five were written and seen not to compile first. The headless tests were written before they were run, but after the handler's change, so they were never seen to fail.
- **Decided here, beyond what the ticket and 09 say:**
  - **A grid too small for the workspaces** gets more columns when it's horizontal and more rows when it's vertical. 09 says "the fill-order dimension grows", which can be read either way. This reading keeps the rows of a horizontal panel's switcher, which are what its height is divided into, and it moves workspaces that had a cell: with rows and columns both 2 and six workspaces, the third goes from (0, 1) to (2, 0). The other reading keeps every cell and adds rows. Marco does neither and leaves the extra workspaces out of the grid. Only keys set by hand get here: the switcher always writes one of the two as 0.
  - **More cells than workspaces:** the grid is kept as the keys have it, and the last cells are empty, as in Marco.
  - **The rows and the columns have a range in the schema,** 0 to 36, and Camino clamps to it as well, for a schema that hasn't one. 09 gives them a type and a default only.
  - **Left and right stop at the ends of a row.** Nothing wraps until M8 reads `wrap-style`, and an empty cell is as an edge. Up and down aren't in policy: they come with their bindings, in M8, and `Grid::neighbour` takes them then.
  - **Without the schema,** Camino logs `the org.mate.Camino schema isn't installed; using its defaults` once, as for any optional schema (29), and the workspaces are one row.
- **XWayland:** nothing was done. [38](38-x-connection-and-root-properties.md) doesn't publish `_NET_DESKTOP_LAYOUT`, and by EWMH it's the pager's property to set, not the window manager's.
- **The patch is `0007`,** after [97](97-pager-count-and-names.md)'s two, and `mate-panel-camino` is at `pkgrel` 6:
  - **Placing by coordinates isn't gated** (ADR 0011): it's right on any compositor that sends them. libxfce4windowing 4.20.7 keeps a workspace's row and column at -1 until `coordinates` comes (`xfw-workspace-wayland.c:131-132`, `:393-410`), and its getters hide that, so the patch reads the `layout-row` and `layout-column` properties. The buttons go by coordinates only when every workspace shown has both and no two share a cell. Otherwise they go by the rows preference, as before.
  - **The buttons are built again from an idle** when a row or a column changes, once for the many notifies one change of the grid sends.
  - **The write is gated on `MATE_WM_NAME=Camino`,** on Wayland, through 97's `PAGER_WM_CAMINO`, and on the schema being installed. It's made whenever the switcher updates itself: at start-up, and when the rows or the panel's orientation change. The three keys go in one change, and a key that's as wanted already isn't written.
  - **The switcher's scroll wheel** still works out its own grid from the rows (`workspace-switcher.c`), which matches Camino's on a horizontal panel and not on a vertical one. Left as it is.
- **Checked on the developer's machine (x86_64):**
  - `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and the full suite on a private bus.
  - `meson install` into a scratch directory puts the schema in `usr/share/glib-2.0/schemas/`.
  - `mate-panel-camino` 1.28.7.r111.g58ac6c1-6 builds with `makepkg -f`, the seven patches applying; `namcap` on the PKGBUILD prints nothing, and on the package it has warnings and no error.
  - `camino-git` r249.191e45f builds with `makepkg -f`, its `check()` passing, and the package has `usr/share/glib-2.0/schemas/org.mate.Camino.gschema.xml`. `namcap` has no error on the PKGBUILD or the package. Its one new warning is `Dependency dconf detected and implicitly satisfied (needed for glib schemas)`: the session's packages bring dconf, so it isn't added to `depends`.
- **Done in a worktree,** `camino-ticket-96` on the branch `ticket-96`, because other tickets were being worked on in the main checkout at the same time. 97 was merged first, with two patches in the same files of the panel, so this one was written again on top of them as `0007`, and its gate is 97's.
- **Review left as is:** a cell is a `(usize, usize)` pair, column then row, not a type; the defaults are in the schema, in `WorkspaceLayout::default` and as the translation's builtins, as for every other key; the headless tests wait on policy's state, as the file's older ones do; an idle queued just before the switcher is destroyed is removed only when its grid is finalised.
