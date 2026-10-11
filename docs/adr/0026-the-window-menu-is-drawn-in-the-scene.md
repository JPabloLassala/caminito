# 26. The window menu is drawn in the scene, with its state in policy

Date: 2026-10-11
Status: Accepted

## Context

Ticket 161 brings Marco's window menu: the menu of a window's actions that Alt+Space, a right click on a titlebar, the Frame's menu button, xdg_toplevel's `show_window_menu` and X11's `_GTK_SHOW_WINDOW_MENU` open. Marco's is a GtkMenu, built in its own process (`ui/menu.c`).

Caminito links no toolkit (ADR 0003). The ticket left two ways open:

- Caminito draws the menu itself, in the scene with cairo, as it draws the Window switcher (ADR 0016) and the Workspace popup, with its own menu logic and keyboard navigation.
- A GTK helper on a Layer surface shows a real GtkMenu. That gives GTK's look, its keyboard navigation and Orca, and adds a helper and a private protocol between it and Caminito for the items, the place and the choice.

The acceptance criteria read the menu's items from the debug dump and choose each one through the headless fixture.

## Decision

- **Caminito draws the window menu itself, in the scene,** with cairo, in the GTK theme's colours and font (ADR 0003), as the Window switcher's chrome is drawn. It has a scene layer of its own, over everything a client draws.
- **Its state is policy's** (ADR 0010): which items a window has and which are greyed, as Marco's `meta_window_show_menu` has them, the selection, the open submenu, and what choosing an item does. The compositor crate has its words, its rectangles and its drawing, and tells policy of keys and clicks.
- **While it's open it has the keyboard and the pointer,** as a GtkMenu's grab has: every key but a modifier is the menu's, and no client or Frame has the pointer. A press outside it closes it and reaches no client.
- **Its labels are Marco's own translations,** read with gettext from Marco's catalogues, which are installed with Marco's schemas. `main` sets the locale's message and character-set categories for it, and no other.
- **Orca doesn't read it in v0.1.** Caminito has no AT-SPI tree of its own. The developer accepted this on 2026-10-11.
- **It has no icons.** Marco's *Minimize*, *Maximize* and *Close* items have one where `menus-have-icons` is on.

## Consequences

- The menu's items, the greyed ones and the selection are tested in policy with no protocol objects, and the headless tests read the whole menu from the debug dump.
- There's no new process, no protocol and no start-up delay before the first menu.
- The menu doesn't look exactly like the theme's GTK menus: it has the palette's colours and the interface font, and none of the theme's borders, paddings or check marks.
- Caminito keeps its own menu logic: the selection that skips separators and greyed items, the submenu, the mnemonics, and the place of the menu and its submenu on the output. A greyed item can't be selected with the keys, where GTK lets the selection rest on one.
- The pointer has no GTK niceties: moving it from the submenu's parent to the submenu across another item closes the submenu, and an item is chosen at the press of a button on it, so pressing on one item, dragging and releasing on another chooses the first.
- `caminito` depends on `libc`, for `setlocale`, and has one more `unsafe` block.
- A user who needs Orca can't use this menu with it. Every item but *Move Titlebar Onscreen* and the submenu's has a keybinding that does the same.
- Rejected:
  - a GTK helper on a Layer surface (a helper and a protocol for one menu, and the headless tests would need the helper built and running; ADR 0016 rejected a client on a Layer surface for the Window switcher for the same reason);
  - an AT-SPI tree for the menu in v0.1 (much more than the menu itself).
