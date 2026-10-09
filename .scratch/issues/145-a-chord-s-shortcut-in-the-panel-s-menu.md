# A chord's shortcut in the panel's menu

Epic: global-menu
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by:

## What to build

[ADR 0020](../../docs/adr/0020-vs-code-s-menu-shows-only-the-state-its-extension-can-read.md), as amended on 2026-10-09. An item whose keybinding is a chord shows it in the panel's menu as VS Code's own menubar writes it: "Ctrl+K Ctrl+O" beside Open Folder.... Today it shows nothing.

- **A fourth carried patch to the applet,** in `packaging/arch/vala-panel-appmenu-mate-caminito/`. `dbus_menu_item_update_shortcut` in `subprojects/appmenu-glib-translator/item.c` makes a GTK accelerator of a shortcut's first press and drops the rest ("too many keys. Taking first."). A GTK accelerator is one press, so a chord can't go through `G_MENU_ATTRIBUTE_ACCEL` as it is. How the presses reach the menu item's accelerator text, and where they're drawn, is this ticket's to find. They sit where a one-press shortcut sits, aligned with the others, and aren't part of the label (ADR 0020's rejected alternatives).
- **A one-press shortcut is drawn as before,** for every application the translator serves.
- **The extension serves every press.** `shortcutOf` in `caminito-vscode/src/keybindings.mts` gives a chord no shortcut. It gives one for each press, in order, and none if any press has no GDK name.
- **The patch ships before the extension that needs it.** An applet without it draws a chord's first press alone, on a dozen items, each wrong. `caminito-vscode-git` and the applet's package are versioned or tied so that the extension isn't served to an older patched applet, or the reason it can't be is written here.
- **The README's line** that says the menu has no shortcut for a chord.

## Acceptance criteria

- [ ] `shortcutOf`'s cases: a chord of two presses, a chord with a press that has no GDK name, one press as before.
- [ ] Through the seam: a client reads both presses on an item bound to a chord, and a change of the user's file to and from a chord sends one `ItemsPropertiesUpdated`.
- [ ] The applet's package builds with the patch, and its three other patches still apply.
- [ ] In a nested run with VS Code 1.140, the panel draws "Ctrl+K Ctrl+O" beside File > Open Folder..., aligned with Ctrl+O beside Open File..., and a one-press shortcut of a GTK application is drawn as it was.

## To run, and to answer here

- [ ] What the panel draws for a chord of an application other than VS Code that serves one through dbusmenu, if one can be had.

## Notes

- **From [120](120-keybinding-labels.md)'s nested run:** a build that served Open Folder... as `[['Control', 'k'], ['Control', 'o']]` got "Ctrl+K" in the panel, read through AT-SPI as `<Primary>k` and seen by the developer. The change to `shortcutOf` in that build was the whole of the extension's side.
- **The nested run's scripts** are in [119-checks](119-checks/): `up.sh` takes the applet's built package, and `look.py` reads a panel item's accelerator through AT-SPI.
- [124](124-vs-code-in-a-real-session.md)'s check of the first version's bar waits on this ticket.
