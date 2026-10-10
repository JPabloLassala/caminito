# A chord's shortcut in the panel's menu

Epic: global-menu
Type: implementation
Status: open: the code and the patch are in; the package's makepkg build and the nested run with VS Code 1.140 are the developer's, and the question under "To run" waits on an application that serves a chord
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

- [x] `shortcutOf`'s cases: a chord of two presses, a chord with a press that has no GDK name, one press as before.
- [x] Through the seam: a client reads both presses on an item bound to a chord, and a change of the user's file to and from a chord sends one `ItemsPropertiesUpdated`.
- [ ] The applet's package builds with the patch, and its three other patches still apply.
- [ ] In a nested run with VS Code 1.140, the panel draws "Ctrl+K Ctrl+O" beside File > Open Folder..., aligned with Ctrl+O beside Open File..., and a one-press shortcut of a GTK application is drawn as it was.

## To run, and to answer here

- [ ] What the panel draws for a chord of an application other than VS Code that serves one through dbusmenu, if one can be had.

## Notes

- **From [120](120-keybinding-labels.md)'s nested run:** a build that served Open Folder... as `[['Control', 'k'], ['Control', 'o']]` got "Ctrl+K" in the panel, read through AT-SPI as `<Primary>k` and seen by the developer. The change to `shortcutOf` in that build was the whole of the extension's side.
- **The nested run's scripts** are in [119-checks](119-checks/): `up.sh` takes the applet's built package, and `look.py` reads a panel item's accelerator through AT-SPI.
- [124](124-vs-code-in-a-real-session.md)'s check of the first version's bar waits on this ticket.

## Done (2026-10-10)

- **Tests first, through the seam:** `shortcutOf`'s cases in `caminito-vscode/test/keybindings.test.mts` (a chord of two presses, a chord with a press that has no GDK name, one press as before; every snapshot label now has a press per key), and in `labels.test.mts` a client reads `[['Control', 'k'], ['f']]` on Close Folder, and a change of the user's file to Ctrl+K Ctrl+S on Save and back sends one `ItemsPropertiesUpdated` each. `npm run typecheck`, `npm test` (139 pass) and `npm run build` ran on Node 22.
- **How the presses reach the menu:** GTK 3 has no way to give a menu item's `GtkAccelLabel` text of its own: it draws only what `_gtk_accel_label_class_get_accelerator_label` makes of one key and its modifiers. So the translator writes a chord's accelerator as each press's with a space between them, `<Control>k <Control>o`. GTK parses no key from it, so its accel label draws nothing, and an applet with the translator half alone draws no shortcut rather than a wrong one. A one-press accelerator is the string it was, and GTK draws it as before for every application.
- **Where they're drawn:** `lib/menu-chords.vala`, new in the patch, watches every menu GTK builds from the model (`GtkMenuShell::insert`, from the menubar down). GTK keeps the model's item on each widget under the data key `GtkMenuTrackerItem`, whose `accel` property is the attribute unparsed (gtkmenushell.c and gtkmenutrackeritem.c at 3.24.43). For a chord it adds a widget after the label, in the box GtkModelMenuItem uses for an icon, that draws each press as `gtk_accelerator_get_label` writes it, joined by spaces ("Ctrl+K Ctrl+O"; GTK's order of modifiers is kept, as ADR 0020 says). Its CSS node is `accelerator`, so a theme styles it as an accel label's, and it keeps an accel label's 3-pixel padding at its end, so its text ends where a one-press accelerator's does. It isn't part of the label.
- **Not seen through AT-SPI:** a chord's item has no accelerator for AT-SPI, since ATK takes the item's key binding from its accel label, which has none. 119-checks' `look.py` reads nothing for Open Folder...; the nested run has to look at the panel.
- **The patch is one,** `0004-applet-Draw-every-press-of-a-chord-s-shortcut.patch`, since its translator half alone draws no shortcut for a chord. An empty shortcut, which the translator read past the end of, is now an empty accelerator.
- **Tied by a conflict:** `vala-panel-appmenu-mate-caminito` is 25.04-5, and `caminito-vscode-git` has `conflicts=('vala-panel-appmenu-mate-caminito<25.04-5')`. A depends would pull the applet onto a machine with VS Code and no panel, and only the patched applet draws Caminito's menus, so the stock one needs no constraint.
- **Built and drawn here, not packaged:** [145-checks](145-checks/)`/check.sh` clones nothing. Given a clone of the upstream source, it resets it to 25.04, applies the four patches with `patch -Np1` as `prepare()` does (all four apply), and builds with `build()`'s meson options. `meson install` and `package()`'s removals ran too. Then `serve.py` serves File with Open File... (Ctrl+O), Open Folder... (Ctrl+K Ctrl+O) and Close Folder (Ctrl+K F), and `look.vala` imports it through the patched translator and draws it with `menu-chords.vala` under Xvfb. All three shortcuts end at the same x, and Open Folder... moved to one press and back draws Ctrl+O, then the chord again. This ran on Ubuntu 24.04 with GTK 3.24.41, not with makepkg on Arch, so the package's build is left unchecked.
- **Not run:** the nested run with VS Code 1.140 and the panel, and an application other than VS Code that serves a chord through dbusmenu: none was at hand here. Qt's dbusmenu serves a multi-key `QKeySequence` as several presses, so a Qt app with such a shortcut is a candidate. The Rust workspace's suite wasn't run, since nothing outside `caminito-vscode/`, `packaging/`, the README and this ticket changed.
