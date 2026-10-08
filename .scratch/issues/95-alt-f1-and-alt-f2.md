# Alt+F1 and Alt+F2

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M6
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Alt+F1 opens the panel's main menu and Alt+F2 its Run dialog, as they do under Marco ([06](06-v01-protocol-and-integration-set.md)'s *Alt+F1, Alt+F2* row, [17](17-milestone-slicing.md)).

- **The bindings:** both join the hard-coded set of Marco's defaults, which stays until M8.
- **The call:** Caminito calls `MainMenu(u ts)` and `RunDialog(u ts)` on `org.mate.Panel` at `/org/mate/Panel`, with the key event's time.
- **The popup grab:** the panel never saw the keypress, so it has no serial for its menu's grab. Caminito accepts one popup grab without a valid serial from the panel's client for about a second after the call, and no other ([63](63-layer-shell.md) left this here). The panel's client is the one that owns `org.mate.Panel`.
- **An unpatched panel:** when the name has no owner or the call fails, Alt+F2 spawns `mate-panel --run-dialog` and Alt+F1 logs once.
- **The carried patch:** `mate-panel-caminito` exports `org.mate.Panel` when `MATE_WM_NAME=Caminito`, and both methods reuse the `_MATE_PANEL_ACTION` code paths: drop the menu from the Menu Bar, else anchor it to the Menu Button, else open it at the pointer ([16](16-packaging.md), ADR 0011).
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [x] Headless tests with a D-Bus client that owns `org.mate.Panel`, written before the code:
  - Alt+F1 and Alt+F2 call the right method, and the key reaches no client;
  - a popup grab without a valid serial from that client is accepted within the second, once;
  - the same grab is refused after the second, a second time, and from another client;
  - with no owner, Alt+F2 spawns the run dialog and Alt+F1 logs once.
- [x] `mate-panel-caminito` builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [x] The checklist has the item, and it was run in a Session: the menu opens, takes the keyboard and closes with Escape; the Run dialog opens focused. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).

## Notes

- **`KILL_DIALOG` is dropped** for Caminito's own Force Quit, which is M8's (06, 17).
- **How Caminito tells the panel's client:** the bus knows the owner's pid, and Caminito knows each Wayland client's. If more than one client has that pid, any of them may take the grab.

## Resolution

Done on 2026-10-06:

- **Caminito checked no popup grab's serial before this**, so there was nothing for the panel's grab to be an exception to. It does now: a grab is accepted when its serial is of one of the last 32 key or button events Caminito sent, and that client got it. GTK names the later of its last key event and its last button press, and Qt its last input event, so both name one of those. A grab that isn't accepted has its popup dismissed with `popup_done`.
- **A submenu's grab needs no serial of its own** while its menu holds the grab. GTK opens a submenu under the pointer with the serial it had for the menu, and the panel has none for a menu Alt+F1 opened, so without this the main menu's submenus would be dismissed.
- **A grab asked for while another menu is open is dismissed.** Caminito's bindings work while a menu has the keyboard, so Alt+F1 can have the panel ask then, and Smithay answers a grab that isn't the open menu's child with a protocol error, which would disconnect the panel.
- **The call** is in `dbus/panel.rs`, on zbus's thread: it asks the bus for the name's owner and the owner's pid, tells the event loop, waits for its answer, and only then calls the owner by its unique name. So Caminito knows whose grab to accept before the panel can ask for it. The second runs from there, and a call that fails ends it. `RunDialog` opens no menu, so it's called without any of that and allows no grab.
- **An unpatched panel:** with no owner, a failed call or no bus, Alt+F2 spawns `mate-panel --run-dialog` with `Caminito::environment`, and Alt+F1 warns once for Caminito's lifetime.
- **The carried patch** is `mate-panel-caminito`'s ninth, and `pkgrel` is 8. What the two actions do moved from `panel-action-protocol.c`, which is only built with X11, to a new `panel-action.c`, and the X11 message and the D-Bus methods both call it. A call brings no event, so the menu's third place is the panel's window under the pointer, and the first panel's corner when the pointer is over none: a Wayland client doesn't know where the pointer is outside its own windows.
- **Open, for the Session run:** after Alt+F1 drops *Applications* from the Menu Bar, moving the pointer to *Places* has GTK open a new menu that is no submenu, with the serial it had before. If that serial isn't of a recent event the panel got, Caminito dismisses it, since the one grab is spent. The arrow keys don't have this: the panel gets them, and names them. The checklist's step 3 looks for it. If it fails, the fix is to let the panel's client keep the serial it was allowed, which is more than the *one grab, and no other* this ticket asks for, so it's the developer's call.
- **The calls have no timeout.** A panel that hangs leaves each call waiting until it answers, and a menu that arrives after its second is dismissed.
- **The checklist item** is *Alt+F1 opens the main menu, and Alt+F2 the Run dialog*, under *The panel*. It wasn't run in a Session where the ticket was done, which had no Session to log into, so it's left to [106](106-packaging-and-closing-m6.md).
