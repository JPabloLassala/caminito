# Keybinding labels in the panel's menu

Epic: global-menu
Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [118](118-the-menu-is-vs-codes.md)

## What to build

[ADR 0020](../../docs/adr/0020-vs-code-s-menu-shows-only-the-state-its-extension-can-read.md), under "What the menu shows" in [the spec](../vscode-global-menu/spec.md). Each item shows the shortcut VS Code's own menu would, the user's own bindings included, and follows a change of the user's keybindings file.

- **The label rule,** from the default keybindings (`vscode://defaultsettings/keybindings.json`) and the user's file: the user's removals first; a binding whose keys a later entry with no `when` gives to another command stops counting; then the only binding, else the last with no `when`, else the last.
- **The user's file is found through `storage.json`,** with the spec's fallbacks down to the default profile's. It's read and watched, never opened in a tab.
- **The fallback** when the defaults can't be read is the snapshot's labels, with the user's own bindings over them.
- **dbusmenu's `shortcut`:** the modifiers and then a GDK key name, from the table in the research on branch `research/vscode-keybinding-labels-and-recent-files`. A chord has no shortcut, nor has a key with no GDK name.
- **Pushed updates:** a change of the user's file, a change of the installed extensions (the defaults), and focus gained (the window's profile). A change of shortcuts alone is an `ItemsPropertiesUpdated`. Coalescing as in [119](119-checkmarks-and-workspace-items.md), built here if this ticket comes first.

## Acceptance criteria

- [ ] The label rule's cases, as a function of the two files: one binding, several, a removal, a binding taken over, a chord, a key with no GDK name.
- [ ] Every key in the defaults of 1.140 has a GDK name or is knowingly left without one.
- [ ] The profile lookup's cases: a folder window, a workspace-file window, no entry, a profile that uses the default's keybindings, an unreadable or misshapen `storage.json`.
- [ ] Through the seam: a client reads `shortcut` on an item, and a change of the user's file sends one update.
- [ ] Through the seam: with the defaults unreadable, the snapshot's labels are served with the user's bindings over them.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] Whether the patched applet cuts a chord to its first press, as upstream's 25.04 does.
- [ ] The `storage.json` lookup for a window on a `.code-workspace` file.
- [ ] The developer's own keyboard layout.

## Notes

- Drawing a chord takes a carried patch to the applet's translator, a later ticket.
- GTK writes the modifiers its own way ("Shift+Ctrl+P"), and that's left.
