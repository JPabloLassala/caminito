# The panel's menu is VS Code's own

Epic: global-menu
Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [116](116-the-extension-serves-one-menu-and-claims.md)

## What to build

[ADR 0019](../../docs/adr/0019-vs-code-s-menu-table-is-a-snapshot-of-its-own-native-menubar.md), under "The menu table" in [the spec](../vscode-global-menu/spec.md). The panel shows VS Code's eight menus with every item of an ordinary editing state, and each item runs its command.

- **The developer command** starts Microsoft's build under Xvfb with a native title bar and a throwaway profile, waits for `lastKnownMenubarData` to settle, with a folder open and with a multi-root workspace open, merges the two and strips what belongs to the run. It keeps the snapshot's keybinding labels for [120](120-keybinding-labels.md).
- **The table** is committed with the version and commit it was made from. Its format is this ticket's to set.
- **The annotations file,** written by hand and keyed by command id, with the `product.json` gates. The command fails on an annotation whose command the snapshot lacks.
- **The tolerance rule at start:** drop each item whose command isn't listed, then each submenu left empty, then collapse the separators; gate product-dependent items on the running editor's own `product.json`.
- **One log line** names the table's version and the editor's when they differ.
- **Open Recent keeps its fixed items only,** and there are no checkmarks or shortcuts yet.

## Acceptance criteria

- [ ] The command writes the committed table, and running it again changes nothing.
- [ ] Through the seam: with every command listed, a client reads the table's menus in order with their mnemonics, separators and submenus.
- [ ] Through the seam: an item whose command is missing is gone, a submenu left empty is gone, and no two separators are together or at an end.
- [ ] Through the seam: an item gated on a `product.json` key is there only when the fake's `product.json` has it.
- [ ] Through the seam: an `Event` on any item runs its command, with its arguments if the table has them.
- [ ] In a nested run with VS Code 1.140, the panel shows the eight menus and an item of each runs.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] A menu of more than one submenu, and how long the panel stalls while the importer preloads the real menu's submenus. If it's too long, say so here: the fix is a carried patch that makes the importer's calls asynchronous, a ticket of its own.
- [ ] How GTK draws the stray `action` of a submenu whose `label` is sent first, to confirm that alphabetical order is what avoids it.
- [ ] The snapshot route from the command itself. The earlier run's output wasn't kept.

## Notes

- Titles are English. There's no list of profiles, no profile name in Export Profile, and no update item.
- Copy As drops out as an empty submenu, and Share keeps what the table has.
