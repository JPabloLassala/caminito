# The window switcher

Type: implementation
Status: open
Milestone: M8
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

The popup that Alt+Tab opens: a grid of thumbnails in the style of KDE Plasma's Thumbnail Grid, in place of Marco's popup ([ADR 0016](../../../docs/adr/0016-one-window-switcher-in-place-of-marcos-alt-tab-popup.md)). It was settled with the developer on 2026-10-07, ahead of M8, and is built there ([17](17-milestone-slicing.md)). Until then `switch-windows` activates the most recently used window with no popup.

- **The actions.** Every `switch-*` action opens the same switcher over a different set of windows, and its `-backward` twin opens it stepping the other way:
  - `switch-windows` (Alt+Tab): the active workspace's windows;
  - `switch-windows-all` (unbound): every workspace's;
  - `switch-group` (Alt+\`): the focused application's;
  - `switch-panels` (Ctrl+Alt+Tab): docks and the desktop.

  `cycle-windows`, `cycle-group` and `cycle-panels` open no popup, as in Marco.
- **The list.**
  - Most recently used first, with minimised windows after the others, as Marco orders them.
  - The selection starts on the second entry, so a quick Alt+Tab goes to the window used before this one.
  - Left out: windows that ask to skip the taskbar and, but for `switch-panels`, docks and the desktop.
  - A window with a modal dialog is listed as its dialog.
  - Windows from every output are listed.
  - An urgent window is marked and keeps its place.
- **Few windows.** With no window to list, nothing happens. With one, it's activated and no popup opens.
- **The popup.**
  - It's centred on the output the pointer is on.
  - It's a grid that wraps at `alt-tab-max-columns`, with thumbnails that shrink to fit when there are many windows.
  - The selected window's title is under the grid.
  - It appears about 90 ms after the key, so a quick Alt+Tab switches with nothing shown.
  - Its colours and font are the GTK theme's, from the palette the helper gives frames (ADR 0003).
- **A thumbnail** is the window's surface, drawn live, with the application's icon in a corner. A minimised window shows its last content. With `compositing-manager` off or `compositing-fast-alt-tab` on, the grid shows icons with no thumbnails.
- **Keys.** While the modifier is held:
  - Tab and Shift+Tab step forward and back, or the action's own key when it isn't Tab;
  - the arrow keys move in the grid;
  - releasing the modifier, or Enter, commits: the selected window is activated, and a minimised one is restored;
  - Escape cancels and leaves focus and stacking as they were.
- **The pointer.** A click on a thumbnail commits that window. Hovering does nothing: the popup often opens under the pointer, and hover would take the selection.
- **The selected window** is raised while the switcher is open when `alt-tab-raise-windows` is set, and goes back if the switcher is cancelled. Otherwise nothing shows on the window itself: no border and no fading of the others.
- **Windows that change meanwhile.** One that closes leaves the grid, and the selection moves to the next if it was selected. One that opens isn't added and isn't focused until the switcher closes, when its focus is settled as for any new window.
- **When it doesn't open.** During a move or resize, and while the screen is locked, the actions do nothing. A client's open menu is dismissed first.
- **Where it lives.** The switcher's state is policy's: the windows listed, their order, the selection, committing and cancelling (ADR 0010). The compositor crate lays the grid out, draws it in the scene and tells policy of keys and clicks.

## Acceptance criteria

- [ ] Policy tests, written before the code: each action's list and order, minimised windows last, the modal dialog for its parent, the skip-taskbar windows left out, the starting selection, stepping and wrapping both ways, commit, cancel, a window closing while selected, a window opening meanwhile, none and one window.
- [ ] Headless tests: a quick Alt+Tab switches with no popup drawn; holding Alt draws it on the pointer's output; arrows, Enter, Escape and a click each do as above; a minimised window is restored on commit; nothing opens during a move or while locked.
- [ ] A drawing test for the grid: its wrapping at `alt-tab-max-columns`, thumbnails shrinking, the icons-only grid, and the urgent mark.
- [ ] `docs/parity.md`'s rows for the `switch-*` actions and the keys below, each with its status and ADR 0016 where it's *never*.
- [ ] Checklist: live thumbnails of a playing video and of a minimised window, in a session.

## Notes

- **The keys.** Honoured: `alt-tab-max-columns`, `alt-tab-raise-windows`, `compositing-manager`, `compositing-fast-alt-tab`. Never: `show-tab-border`, as nothing is drawn round the selected window.
- **Not settled on 2026-10-07,** and to be put to the developer when the ticket starts:
  - `icon-size`: proposed as the size of the icons in the icons-only grid, in logical pixels (09), with the corner icon a fixed fraction of its thumbnail.
  - `alt-tab-expand-to-fit-title`: proposed as never, with a title longer than the popup ellipsised, since the grid's width comes from its thumbnails.
  - How an urgent window is marked, and how the selected thumbnail is.
- **Urgency** itself comes with focus-stealing prevention, also M8's (17).
- **Drawing a window away from its place** is new to the scene, which draws each window once. Capture ([103](103-image-capture.md)) is the nearest thing to it.
- **Not taken from KDE:** fading or hiding the other windows, closing a window from the switcher, and hover to select.
