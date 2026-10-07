# Alt+F1 and Alt+F2

Type: implementation
Status: open
Milestone: M6
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

Alt+F1 opens the panel's main menu and Alt+F2 its Run dialog, as they do under Marco ([06](06-v01-protocol-and-integration-set.md)'s *Alt+F1, Alt+F2* row, [17](17-milestone-slicing.md)).

- **The bindings:** both join the hard-coded set of Marco's defaults, which stays until M8.
- **The call:** Camino calls `MainMenu(u ts)` and `RunDialog(u ts)` on `org.mate.Panel` at `/org/mate/Panel`, with the key event's time.
- **The popup grab:** the panel never saw the keypress, so it has no serial for its menu's grab. Camino accepts one popup grab without a valid serial from the panel's client for about a second after the call, and no other ([63](63-layer-shell.md) left this here). The panel's client is the one that owns `org.mate.Panel`.
- **An unpatched panel:** when the name has no owner or the call fails, Alt+F2 spawns `mate-panel --run-dialog` and Alt+F1 logs once.
- **The carried patch:** `mate-panel-camino` exports `org.mate.Panel` when `MATE_WM_NAME=Camino`, and both methods reuse the `_MATE_PANEL_ACTION` code paths: drop the menu from the Menu Bar, else anchor it to the Menu Button, else open it at the pointer ([16](16-packaging.md), ADR 0011).
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [ ] Headless tests with a D-Bus client that owns `org.mate.Panel`, written before the code:
  - Alt+F1 and Alt+F2 call the right method, and the key reaches no client;
  - a popup grab without a valid serial from that client is accepted within the second, once;
  - the same grab is refused after the second, a second time, and from another client;
  - with no owner, Alt+F2 spawns the run dialog and Alt+F1 logs once.
- [ ] `mate-panel-camino` builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [ ] The checklist has the item, and it was run in a Session: the menu opens, takes the keyboard and closes with Escape; the Run dialog opens focused. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).

## Notes

- **`KILL_DIALOG` is dropped** for Camino's own Force Quit, which is M8's (06, 17).
- **How Camino tells the panel's client:** the bus knows the owner's pid, and Camino knows each Wayland client's. If more than one client has that pid, any of them may take the grab.
