# A panel menu's submenu gives the keyboard back to the window

Epic: global-menu
Type: bug
Status: open: the fix is in, and the nested run of 121-checks with VS Code, to see Escape close the menus with a submenu open, is the developer's
Milestone: none: after v0.1
Blocked by: none

## What happens

With a menu of the panel's global menu open, the focused window has lost the keyboard focus. When a submenu of it opens, the window regains the focus, with both menus still on screen. Escape then doesn't close them.

## Seen

In [121](121-open-recent.md)'s nested run (2026-10-09): VS Code 1.140.0, the applet's package 25.04-4, a nested Caminito built from master. The editor's `onDidChangeWindowState` was logged beside the dbusmenu traffic.

- File opened: `opened` for item 1, and `focused: false` 15 to 50 ms later.
- Open Recent opened: `opened` for item 11, and `focused: true` 17 to 28 ms later. Twice, and once more with another submenu (item 53), 8 ms later.
- No change of focus when the submenu or the menu closed.
- By hand: Escape didn't close the menus with the submenu open. With only File open it did.

## Not known

- Whether it's Caminito's handling of a popup's grab when a nested popup maps, or the applet's.
- Whether a GTK application's own submenu does it, and whether it happens outside a nested session.
- Where the keys go while the submenu is open. Escape not closing the menus suggests the editor.

## To reproduce

`.scratch/issues/121-checks/up.sh` and `editor.sh`, then open File and hover Open Recent by hand, and read `<work>/driver/focus-events` beside `events.sh`'s output.

## Done (2026-10-12)

- **The cause was Caminito's order of the popup grabs, not the applet.** `XdgShellHandler::grab` set the submenu's keyboard grab, then its pointer grab. Smithay's `PopupPointerGrab::unset`, run as the menu's pointer grab is replaced, ends the keyboard grab with the menu's serial and puts the keyboard on the grab's root. GTK grabs for a submenu with its menu's serial, so it was the submenu's grab that ended; with no grab, `update_keyboard_focus` gave the keyboard to policy's focused window, which then had Escape.
- **The fix lets the menu's keyboard grab go before the pointer grab is replaced, and takes the submenu's keyboard grab last.** Both serial checks run before either grab changes, so a refused grab leaves the seat as it was.
- **A window's own submenu, as a GTK application's, went the same way**, with the window as the root: the keyboard stayed on the window, without the grab. Both cases have a test: `layer_shell::a_layer_surfaces_submenu_keeps_the_keyboard_from_the_window`, which failed with the window's `keyboard enters window 0` before the fix, and `popups::a_submenu_grabbed_with_its_menus_serial_keeps_the_grab`.
- **Not run:** 121's nested run with VS Code, and `tools/check` as a whole. Clippy's step fails here on three lines of `caminito-theme` that this ticket doesn't touch: the toolchain is `stable`, now 1.97, whose `manual_range_contains` flags them.
