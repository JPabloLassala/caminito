# The workspace popup

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 150, 108
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's workspace popup ([05](05-v01-marco-feature-cut.md), [17](17-milestone-slicing.md)): a workspace switch made with a binding shows the Workspace grid, with the workspaces' names and the new one selected, while the binding's modifier is held.

- **When it shows:** on a `switch-to-workspace-*` or `move-to-workspace-*` direction binding ([150](150-workspace-up-down-prev-and-wrap-style.md)), and further presses while the modifier is held move through the grid as the bindings do. Releasing the modifier closes it.
- **What it shows:** each workspace's cell, in the grid's rows and columns, with its name ([97](97-pager-count-and-names.md)), the active one marked, as Marco's popup does. A small outline of each workspace's windows is drawn if Marco's popup draws them.
- **Where and how:** centred on the output the pointer is on, as the Window switcher is ([108](108-window-switcher.md)), and drawn the same way: a layer of the Scene over everything a client draws, its chrome drawn with cairo in the GTK theme's Palette.
- **When it doesn't:** while the session is locked, during a move or resize, and while the Window switcher is open.
- **The debug dump** has the popup's output, cells and selection once it shows.

## Acceptance criteria

- [ ] Headless tests through the fixture, written before the code: a held Ctrl+Alt with Right and Down shows the popup on the pointer's output with the right cell selected, a release closes it, and nothing shows while locked or with the switcher open. They read the popup's rectangles from the debug dump, as 108's do.
- [ ] A drawing test: the grid's cells for a 2 × 2 and a part-filled grid, and a name in its cell, read from the output's pixels. Marco's popup has no goldens.
- [ ] The rows in `docs/parity.md` for the direction actions say what shows, with these tests.
- [ ] The checklist has an item, tagged M8: switching with Ctrl+Alt+arrows in a 2 × 2 grid with named workspaces, on each output. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, decided when the ticket is done and listed under *What was decided* for confirmation, as [108](108-window-switcher.md) did:

- **Its look,** with no goldens to match: the cell size, the selected cell's mark, and whether windows are outlined.
- **Whether a switch from the pager or over D-Bus shows it.** Proposed: no, only a binding, since the pager already shows the change.
- **Its timing** against the switcher's 90 ms delay: shown at once, as Marco does, or after the same delay, so that a quick switch shows nothing.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: which bindings show the popup (whether `switch-to-workspace-N` and `-prev` do), whether it follows `move-to-workspace-*`, and what closes it besides the release.
