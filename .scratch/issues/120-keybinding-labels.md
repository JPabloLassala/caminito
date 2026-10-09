# Keybinding labels in the panel's menu

Epic: global-menu
Type: implementation
Status: open: the code is in, and the three checks under "To run" need VS Code and the patched applet in a Session; the developer's
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

- [x] The label rule's cases, as a function of the two files: one binding, several, a removal, a binding taken over, a chord, a key with no GDK name.
- [x] Every key in the defaults of 1.140 has a GDK name or is knowingly left without one.
- [x] The profile lookup's cases: a folder window, a workspace-file window, no entry, a profile that uses the default's keybindings, an unreadable or misshapen `storage.json`.
- [x] Through the seam: a client reads `shortcut` on an item, and a change of the user's file sends one update.
- [x] Through the seam: with the defaults unreadable, the snapshot's labels are served with the user's bindings over them.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] Whether the patched applet cuts a chord to its first press, as upstream's 25.04 does.
- [ ] The `storage.json` lookup for a window on a `.code-workspace` file.
- [ ] The developer's own keyboard layout.

## Notes

- Drawing a chord takes a carried patch to the applet's translator, a later ticket.
- GTK writes the modifiers its own way ("Shift+Ctrl+P"), and that's left.

## Done (2026-10-09)

- **The rules are functions of their inputs,** tested first and directly: `labelsOf`, `fallbackLabelsOf` and `shortcutOf` in `caminito-vscode/src/keybindings.mts`, and `keybindingsFileOf` in `src/profiles.mts`. The core keeps them in a `Keybindings` class in `src/global-menu.mts`, and `test/labels.test.mts` drives it through the seam: a client reads `shortcut`, a change of the user's file is one `ItemsPropertiesUpdated`, the snapshot's labels are served when the defaults can't be read, focus gained follows a change of profile, and a change of extensions reads the defaults again. `npm run typecheck` is clean and `npm test` passes 91 tests on Node 22, and the bundle builds. The Rust workspace's suite wasn't run: nothing outside `caminito-vscode/` changed, and `pango-sys` doesn't build in this container (118's Done).
- **The research branch the ticket names isn't on the remote,** so its table of GDK names couldn't be read. The table was made again from VS Code 1.140.0's `src/vs/base/common/keyCodes.ts`: every name keybindings.json can give a key, US and general. 122 have a GDK name, and each was checked against X11's `keysymdef.h` and `XF86keysym.h` here; that check isn't a test, since CI may lack the headers. Ten are knowingly without one, each with its reason in `WITHOUT_GDK_KEY`: keys whose keysym depends on the layout (`oem_102`, `abnt_c1`, `abnt_c2`, `oem_8`), a modifier alone, `unknown` and `keyincomposition`. A bare scan code has none, as the ADR says.
- **The second criterion is met by a superset,** not by the defaults file itself. VS Code 1.140 couldn't be downloaded here, so the test holds the tables to every key 1.140 can bind, which covers every key its defaults can hold. The key list in the test was taken from the same reading of `keyCodes.ts`, so a key that reading missed wouldn't be caught.
- **A key and a chord that starts with it take each other over,** as VS Code's resolver does (`_addKeyPress` in `keybindingResolver.ts`). The ADR says only "gives its keys". It's one step further than the ADR, in the same direction, so the ADR isn't reopened. A label can differ from the ADR's wording when a user binds a chord's first press.
- **Defaults that can't be read again keep the last read.** The snapshot's labels are used only while no read has succeeded. A failed read after a change of extensions says so in the log.
- **A tie in `storage.json` to a profile that's gone is no tie,** so the window gets `window.newWindowProfile`'s profile, as VS Code itself drops such a tie. The agents profile always uses the default's keybindings, as VS Code forces it to. `when` clauses are compared as written, with their spaces evened out, where VS Code compares them parsed; this matters only for a removal with a `when`.
- **The default profile's directory comes from the extension's global storage,** two levels up, which ticket 02 found is the default profile's in every window. A profile's file whose directory isn't there yet is watched again at the next focus.
- **Not run here:** the three checks under "To run". They need VS Code and the patched applet in a Session.

