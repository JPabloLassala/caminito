# Show Desktop, shared by the panel and Ctrl+Alt+D

Type: implementation
Status: open
Milestone: M6
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

The panel's Show Desktop button and Ctrl+Alt+D show and bring back the same desktop, and each sees what the other did ([06](06-v01-protocol-and-integration-set.md)'s *Show Desktop* row, [17](17-milestone-slicing.md)).

- **Policy:** showing the desktop with Marco's semantics, per workspace. It hides the workspace's windows without minimising them for good, and ends when a window on that workspace is mapped, focused or activated, as Marco's does. Read Marco's source for the cases before writing the tests.
- **The binding:** Ctrl+Alt+D joins the hard-coded set of Marco's defaults, which stays until M8.
- **D-Bus:** the `ShowingDesktop` property of `org.mate.Camino`, read and write, for the current workspace, with a change signal whenever it changes, whoever changed it. It's refreshed in the refresh pass, as `PrimaryOutput` is ([81](81-primary-output.md)).
- **The carried patch:** `mate-panel-camino`'s Show Desktop applet reads, writes and follows the property when `org.mate.Camino` has an owner, and keeps libxfce4windowing's minimise-all when it has none ([16](16-packaging.md), ADR 0011).
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [ ] Policy tests, written before the code: showing the desktop hides one workspace's windows and leaves the others'; it ends on a new window, on activation and on a second toggle; the windows come back in their stacking order with the focus they had.
- [ ] Headless tests with a D-Bus client: writing the property hides the windows; Ctrl+Alt+D changes the property and signals it once; switching workspace signals the new workspace's value.
- [ ] `mate-panel-camino` builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [ ] The checklist has the item, and it was run in a Session: the button and Ctrl+Alt+D agree in both directions. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).

## Notes

- **`show-desktop-skip-list`** is M8's (17).
- **Windows that never hide** (the desktop, docks) are layer surfaces under Camino and aren't policy's windows, so the rule is only about XWayland windows of those types.
