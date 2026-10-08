# VS Code's global menu in a real Session, by hand

Type: verification
Status: open
Milestone: none: after v0.1
Blocked by: [117](117-the-package-installs-the-extension.md), [119](119-checkmarks-and-workspace-items.md), [120](120-keybinding-labels.md), [121](121-open-recent.md), [122](122-the-offer-to-hide-the-menubar.md)

## What to check

What nobody has run with a person at the keyboard, from "What has to be run" in [the spec](../../vscode-global-menu/spec.md). Every earlier run moved the focus through a debug call in a nested run. This one is a Session on the tty with the packages installed, and a person clicking.

A result that contradicts an ADR reopens it from here, with a ticket of its own for the fix.

## Checks

- [ ] Whether a click's focus change is ever dropped, over a hundred clicks between two windows. In the nested run 3 of 20 were. If one is, ADR 0018's dropped focus event is reopened.
- [ ] `focused`, and the menu, while a panel menu has the keyboard and after it closes.
- [ ] A floating editor window, opened and reached from its workbench window, with a text editor and with only a terminal; and whether the claim made as it opens lands on it or on the workbench window.
- [ ] A fork: the value of `focused` at activation, and the menu the tolerance rule leaves. Cursor's window, which has never been checked.
- [ ] A window attached to SSH or a container.
- [ ] A window with a menu on its own surface and a claim.
- [ ] The agents window, with more than one item tried.
- [ ] An editor older than the floor, if one can be had.
- [ ] Whether a window ever closes by itself soon after a reload, as one did once, 0.7 s after, in the prototype's runs.
- [ ] The first version's bar, by eye: the same top-level menus and items as the in-window menubar in an ordinary editing state, every item tried runs, shortcuts and recent files are there.
- [ ] The README reads right against what was seen.

## Notes

- The results go in a `124-checks` directory beside this file, as the other global menu tickets' do.
