# Keyboard move and resize

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 148
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`begin-move` (Alt+F7) and `begin-resize` (Alt+F8), with Marco's keyboard grab ([05](05-v01-marco-feature-cut.md)).

- **The grab:** the arrow keys move or resize the window by Marco's step, a modifier makes the step finer if Marco's does, Return or a click puts it down, and Escape puts it back where it was.
- **Resizing:** the first arrow picks the edge, as in Marco.
- **The pointer** follows the window as Marco warps it, and a pointer motion moves or resizes it too.
- **X11's keyboard directions:** `_NET_WM_MOVERESIZE` directions 9 and 10 start the same grab. [39](39-x-window-properties-and-requests.md) started a pointer move or resize from them and left the keys to this ticket.
- **Maximised and fullscreen windows** aren't moved or resized, as `begin_move` refuses them today ([144](144-drag-to-unmaximize.md)).

## Acceptance criteria

- [ ] Policy tests, written before the code: a move and a resize stepped with each arrow; Return; Escape putting the window back; a maximised window refused.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: Alt+F7 and Alt+F8 and the arrows; a `_NET_WM_MOVERESIZE` with direction 9 and 10; the keys not reaching the client during the grab.
- [ ] The two actions' rows in `docs/parity.md` say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: Alt+F7 and Alt+F8 on a native and an X11 window, ended with Return and with Escape, and the pointer's place after each. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the step sizes, the finer step's modifier, how the pointer is warped at the start and during the grab, which edge a resize starts on, and what a click ends.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **With the Window switcher:** whether Alt+Tab during a keyboard grab is refused, as during a pointer grab ([108](108-window-switcher.md)).
- **With keyboard-shortcuts-inhibit:** proposed, a client that inhibits shortcuts gets Alt+F7 too, as with every other binding, and a grab already started keeps the keys.
