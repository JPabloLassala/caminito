# The window menu, with "Move titlebar on-screen"

Epic: caminito-v0.1
Type: implementation
Status: resolved: the checklist item's run in a Session is 173's closing run
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

- [x] Policy tests, written before the code: which items each kind of window has and which are greyed; *Move Titlebar On-screen* for a window above, beside and below every output.
- [x] Headless tests through the fixture, on xdg and XWayland windows: the menu opened from each of the five places, with its items read from the debug dump; each item chosen with the keyboard and with a click; Escape and a click outside closing it; `_GTK_SHOW_WINDOW_MENU` in `_NET_SUPPORTED`.
- [x] A drawing test, or a test of the helper's surface, as the choice below makes it: the menu's items and a greyed one.
- [x] `activate-window-menu` and the titlebar action `menu` have rows in `docs/parity.md` that say honoured, and the keyless section's window menu and *Move Titlebar On-screen* too, with the tests above.
- [x] The checklist has an item, tagged M8: the menu from Alt+Space, a right click on the titlebar, the menu button, a GTK4 CSD window's own titlebar and an X11 GTK3 app's (`GDK_BACKEND=x11`), each item chosen once. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, to settle before building:

- **How it's drawn** (ADR-worthy): in the Scene with cairo, as the Window switcher is ([108](108-window-switcher.md)), with Caminito's own menu logic, keyboard navigation and accessibility; or a GTK helper process on a Layer surface, which gets GTK's menus, theme and Orca for free but adds a process and a protocol between it and Caminito. ADR 0003's palette helper and ADR 0010's Policy bear on it.
- **Keyboard navigation and Orca:** what it must support, which follows from the choice above.
- **Greyed items for CSD and XWayland windows:** which items Caminito can't do for each, beyond *Roll Up* on CSD.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the items, their order, labels and conditions; where Alt+Space puts the menu; what *Move Titlebar On-screen* counts as on-screen.

## Work in progress (2026-10-10)

- **Stopped before any of the menu was written, at the developer's request.** The agent had only fetched Marco's source to read its menu code; there is no patch.
- **Landed from its branch:** two integration fixes unrelated to the menu: 155's one-stack ADR renumbered to 0025 (158 had taken 0024), and 156's `begin-move`/`begin-resize` rows added to the two-workspace debug-dump snapshots.

## Done (2026-10-11)

- **The developer's three questions, answered on 2026-10-11.** The menu is drawn in the Scene with cairo, as the Window switcher is, with its state in policy ([ADR 0026](../../docs/adr/0026-the-window-menu-is-drawn-in-the-scene.md)). Orca doesn't read it in v0.1. An item is greyed where Marco doesn't give the window the function, with no rule of its own for a CSD or an XWayland window.
- **Marco's menu has no *Roll Up* or *Unroll*,** so this one has none, against this ticket's list. Marco's `meta_window_show_menu` leaves them out with `#if 0` (window.c:7140-7145, read at `8522ed1`). So no item is greyed for a CSD window alone.
- **The items, their order, labels and conditions** are `menuitems` (menu.c:66-105) and `meta_window_show_menu` (window.c:7083-7208): *Minimize*, *Maximize* or *Unmaximize*, *Move*, *Resize*, *Move Titlebar Onscreen* when the titlebar is off, a separator and the submenu with more than two workspaces, *Always on Top* as a check box, the two sticky items as radio buttons with more than one workspace, the moves to the workspaces the grid has beside the active one for a window that isn't on all of them, a separator, and *Close*. The submenu's item for the window's own workspace is greyed.
- **The moves to a workspace leave the active workspace as it is,** as Marco's menu does (window.c:7063-7071), where the bindings switch there too. This is against "each item does what its binding does".
- **Greyed beyond Marco's:** *Move* and *Resize* are greyed wherever the keyboard move or resize wouldn't start, which adds a window maximised both ways to Marco's.
- **Where Alt+Space puts the menu:** at the window geometry's top left corner, under the titlebar, as `handle_activate_window_menu` does (keybindings.c:2969-2991). A right click opens it at the pointer, the menu button under the button's bottom left corner, and the two client requests at the point given. It's kept on the output that point is on.
- **What *Move Titlebar Onscreen* counts as on-screen:** more than 8 pixels of the titlebar's height, and more than half its width or 50 pixels, whichever is less, on one output's work area (`meta_window_titlebar_is_onscreen`). The item moves the frame as `meta_window_shove_titlebar_onscreen` and then `constrain_titlebar_visible` do: down to the work area's top, up until the titlebar is on its bottom edge, or sideways until a quarter of the window's width, 75 pixels at most, is on. Marco tests against the rectangles that span the screen less the struts; here it's each output's work area, so a titlebar across two outputs counts by its larger part.
- **The labels are Marco's translations,** read with gettext from Marco's catalogues, which the packages already depend on. `main` calls `setlocale` for `LC_CTYPE` and `LC_MESSAGES` alone, and `caminito` depends on `libc` for it. A locale whose character set isn't UTF-8 gets English.
- **Tested first:** `caminito-policy/src/tests/window_menu.rs`, before the policy module. Then the headless tests in `caminito/src/tests/window_menu.rs`, on both kinds of window but where a test says why not, with unit tests beside the labels and the layout. The drawing test reads the output's pixels.
- **Found in review and fixed:** a splash screen's *Move* wasn't greyed; a mnemonic chose a greyed item before a later one with the same letter; a menu whose window had no geometry stayed open with nothing drawn; a keybinding's hint didn't name Backslash as Marco does.
- **Known and left as they are.**
  - The menu has no icons, and its look is the palette's colours and the interface font, not the theme's GTK menus.
  - An item is chosen at the press of a button, so GTK's press, drag and release doesn't choose one. Moving the pointer from the submenu's parent to the submenu across another item closes the submenu. The Frame's menu button opens the menu at the release, where Marco opens it at the press.
  - The arrows skip a greyed item, where GTK lets the selection rest on one.
  - In a right-to-left language Marco exchanges the moves left and right and puts the menu at the window's right. Caminito doesn't.
  - The modifiers' names in a keybinding's hint are in English.
  - A dock's or the desktop's submenu can be opened, as in Marco, and its items do nothing, since they're on every workspace always.
  - The translations have no test with a catalogue: the checklist's step 6 covers them.
- **Not run:** the full test suite, at the developer's request on 2026-10-11. Run after the last edits: `cargo fmt --all --check`, clippy on the workspace, `caminito-policy`'s 452 tests, and `caminito`'s `window_menu` and `bindings` tests (89); before the review's fixes, 401 of `caminito`'s tests that the change touches (frames, grabs, the keyboard grab, the Window switcher, the Workspace popup, the lock, the focus modes, the window actions, xdg-shell, the debug dump, decorations, the root's properties and the parity table). The rest of `tools/check` wasn't run. The checklist's *The window menu* needs a Session and is 173's.
