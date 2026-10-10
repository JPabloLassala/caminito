# The window menu, with "Move titlebar on-screen"

Epic: caminito-v0.1
Type: implementation
Status: needs-triage
Milestone: M8
Blocked by: 149, 155, 156
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's window menu, with every item ([05](05-v01-marco-feature-cut.md), [17](17-milestone-slicing.md)).

- **The items,** in Marco's order, with its labels, translations and accelerator hints: *Minimize*, *Maximize* or *Unmaximize*, *Move*, *Resize*, *Move Titlebar On-screen*, *Roll Up* or *Unroll*, *Always on Top*, *Always on Visible Workspace* and *Only on This Workspace*, the moves to the workspace left, right, up and down, *Move to Another Workspace* with a submenu of the workspaces by name, and *Close*.
- **What they do:** each item does what its binding does ([149](149-window-and-workspace-actions.md), [155](155-above-below-and-sticky.md), [156](156-keyboard-move-and-resize.md)). *Move Titlebar On-screen* moves a window whose titlebar is off every output back so that it's on its output, as Marco's does.
- **Greyed items:** an item a window doesn't allow is greyed, as in Marco: *Maximize* on a fixed-size window, *Roll Up* on a CSD window ([05](05-v01-marco-feature-cut.md)).
- **Where it opens from:**
  - `activate-window-menu` (Alt+Space), under the window's top left corner;
  - the titlebar action `menu` (a right click, by default), which logs and does nothing today;
  - the Frame's menu button;
  - xdg_toplevel's `show_window_menu`, at the point the client gives;
  - X11 `_GTK_SHOW_WINDOW_MENU`, which today is logged, and which then joins `_NET_SUPPORTED` ([39](39-x-window-properties-and-requests.md) left it out, since GTK shows its own menu unless it's listed).
- **The keyboard and the pointer:** the arrows, Return and Escape work in it, as do the items' mnemonics, and a click outside closes it.

## Acceptance criteria

- [ ] Policy tests, written before the code: which items each kind of window has and which are greyed; *Move Titlebar On-screen* for a window above, beside and below every output.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: the menu opened from each of the five places, with its items read from the debug dump; each item chosen with the keyboard and with a click; Escape and a click outside closing it; `_GTK_SHOW_WINDOW_MENU` in `_NET_SUPPORTED`.
- [ ] A drawing test, or a test of the helper's surface, as the choice below makes it: the menu's items and a greyed one.
- [ ] `activate-window-menu` and the titlebar action `menu` have rows in `docs/parity.md` that say honoured, and the keyless section's window menu and *Move Titlebar On-screen* too, with the tests above.
- [ ] The checklist has an item, tagged M8: the menu from Alt+Space, a right click on the titlebar, the menu button, a GTK4 CSD window's own titlebar and an X11 GTK3 app's (`GDK_BACKEND=x11`), each item chosen once. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, to settle before building:

- **How it's drawn** (ADR-worthy): in the Scene with cairo, as the Window switcher is ([108](108-window-switcher.md)), with Caminito's own menu logic, keyboard navigation and accessibility; or a GTK helper process on a Layer surface, which gets GTK's menus, theme and Orca for free but adds a process and a protocol between it and Caminito. ADR 0003's palette helper and ADR 0010's Policy bear on it.
- **Keyboard navigation and Orca:** what it must support, which follows from the choice above.
- **Greyed items for CSD and XWayland windows:** which items Caminito can't do for each, beyond *Roll Up* on CSD.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the items, their order, labels and conditions; where Alt+Space puts the menu; what *Move Titlebar On-screen* counts as on-screen.
