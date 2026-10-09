# A panel menu's submenu gives the keyboard back to the window

Epic: global-menu
Type: bug
Status: needs-triage
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
