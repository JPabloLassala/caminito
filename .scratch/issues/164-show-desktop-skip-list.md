# `show-desktop-skip-list`

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Windows listed in `show-desktop-skip-list` stay on screen when the desktop is shown ([05](05-v01-marco-feature-cut.md), [09](09-gsettings-mapping.md)). [94](94-show-desktop.md) built Show Desktop, shared by Ctrl+Alt+D and the panel's button with Marco's per-workspace semantics, and left the key to M8.

- **Matching:** an entry matches a native window's app id, or an XWayland window's `WM_CLASS`, as 05 decided.
- **Whichever way the desktop is shown:** the binding, the panel's button through `org.mate.Caminito`'s `ShowingDesktop`, and any other path 94 has.
- **A listed window** keeps its place and its focus while the others are hidden, and isn't restored with them, as it wasn't hidden.
- **A change of the key** applies at the next Show Desktop.

## Acceptance criteria

- [ ] Policy tests, written before the code: a listed and an unlisted window shown and restored; an empty list; an entry that matches no window; the list changing while the desktop is shown.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: a native window listed by app id and an X11 one by `WM_CLASS` stay through Ctrl+Alt+D and through `ShowingDesktop` set over D-Bus.
- [ ] `show-desktop-skip-list` leaves `UNSUPPORTED` if 147 put it there; its row in `docs/parity.md` says honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: a terminal's app id and `xterm`'s class in the list, then the panel's Show Desktop and Ctrl+Alt+D. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the list's syntax (its separator, whitespace, case), whether Marco matches the class's instance or class part, or the title, and whether a listed window's transients stay with it.

**For the developer:** none.
