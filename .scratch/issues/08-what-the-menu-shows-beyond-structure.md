# What the menu shows beyond structure

Epic: vscode-global-menu
Type: grilling
Status: resolved
Blocked by: 01, 02
Map: [VS Code's global menu](../vscode-global-menu/map.md)

## Question

Within the first version's bar (the map's Notes): which checkmarks are shown, how a keybinding label is chosen, what Open Recent holds, what is done with items that depend on a state the extension can't read, and when the menu is rebuilt (on `AboutToShow`, on a settings or keybindings change, on a timer).

From [07](07-source-of-the-menu-structure.md): the table is a snapshot with a hand-written annotations file keyed by command id, where the checkmarks that mirror a setting are listed. The snapshot also holds 108 default keybinding labels. Open Recent's entries are stripped from the table and filled at run time.

## Answer

Decided on 2026-10-08 and recorded as [ADR 0020](../../docs/adr/0020-vs-code-s-menu-shows-only-the-state-its-extension-can-read.md). The developer took the recommended answer to every question without a session, so each line below is a recommendation that stood. Nothing new was run, and no glossary term changed.

- **Checkmarks:** only the ones that mirror a setting, the twelve of [01](01-menubar-sources.md)'s list, four of them radio groups. Each value is read with no resource or language scope.
- **State that can't be read:** the item is there, enabled, with no checkmark at all. After [07](07-source-of-the-menu-structure.md)'s drops, only the items the annotations tie to the workspace state come and go.
- **Other extensions' items:** not shown, and no proposed API is enabled. Copy As drops out as an empty submenu, and Share keeps VS Code's own items.
- **Keybinding labels** are computed at run time from the default keybindings and the user's file: removals first, then a binding taken over by a later entry with no `when` stops counting, then the only binding, else the last with no `when`, else the last. The snapshot's 108 labels are the fallback when the defaults can't be read, with the user's own bindings over them.
- **The profile** comes from a read of `storage.json`, then `window.newWindowProfile` for a window with no entry, then the default profile's file. The command that opens a tab is never run.
- **A chord has no shortcut,** nor has a key with no GDK name. GTK's "Shift+Ctrl+P" is left as it is.
- **Open Recent** is laid out as VS Code's own, ten and ten, with labels made by the extension. Folders and workspaces open with `vscode.openFolder`, files with `_files.windowOpen`, or `vscode.open` if that fails.
- **Rebuilding:** pushed on events and never on `AboutToShow` or a timer. The events are focus gained, a change of tabs, the extension's own opening of a recent entry or clearing of the list, a change of an annotated setting, of the user's keybindings file, of the installed extensions and of the workspace's folders. An update is sent only when something differs.

### Handed on

- **[09](09-the-in-window-menubar.md):** View > Appearance > Menu Bar is in the menu with a checkmark that mirrors `window.menuBarVisibility`. If the extension sets that setting itself, the item and its checkmark need a rule.
- **[10](10-thin-end-to-end-slice.md):** whether the patched applet cuts a chord as upstream's 25.04 does; whether a change of tabs comes with every change of the recent list; the `storage.json` lookup for a `.code-workspace` window; whether a setting read with no scope is what VS Code's `config.*` key holds; what a `LayoutUpdated` does to a panel menu that's open; whether an `ItemsPropertiesUpdated` sent within 100 ms of a layout reply has to be sent again; how the applet draws a radio item; whether the window regains focus, and so re-reads, when a panel menu closes.
- **The spec:** the annotations file's three kinds of entry (a setting's checkmark or radio value, a `product.json` gate, a workspace state); the label rule and the table from VS Code's key names to GDK's (in the research for [02](02-keybinding-labels-and-recent-files.md)); the `storage.json` lookup and its fallbacks; the events and the coalescing of updates; doubling an underscore in a recent entry's label so it isn't read as a mnemonic. A carried patch that draws chords is a later ticket of its own.
