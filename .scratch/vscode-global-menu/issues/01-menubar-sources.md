# What an extension can read of VS Code's menubar

Type: research
Status: resolved
Blocked by: 
Map: [VS Code's global menu](../map.md)

## Question

Where can the structure of VS Code's menubar (File, Edit, Selection, View, Go, Run, Terminal, Help, their items, separators, submenus and order) come from, for an extension that has to rebuild it? Check against VS Code 1.140, installed at `/usr/share/code`, and its source at that tag.

- **At run time:** is there any API or command, public or underscore-prefixed, that returns menu items, the commands in a menu, or the menubar data the workbench sends its main process under a native title bar?
- **Other extensions' items:** which menubar menus take `contributes.menus` entries, and can they be read from `vscode.extensions.all[].packageJSON`, built-in extensions included?
- **At build time:** can the core items be extracted from the source (`MenuRegistry.appendMenuItem` on the `Menubar*` menu ids) or from the installed bundle, with their titles, groups, order and `when` clauses? How much moves between two releases, say 1.130 and 1.140?
- **State:** can an extension read a context key, or evaluate a `when` clause? Which toggled items mirror a setting?
- **Forks:** does any of this differ in VSCodium, Code - OSS or Cursor?

## Answer

Full findings are on branch `research/vscode-menubar-sources` (commit a4cc169), in `research/vscode-menubar-sources.md`. Read from the source at tag 1.140.0 (commit `07f806f9`, the one the installed bundle names) and from the bundle. Nothing was run inside VS Code.

- **At run time there's nothing to read.** No API or command returns a menu's items, and `commands.getCommands()` gives ids only. The menubar data the workbench sends its main process is never built under a custom title bar, and the extension host has no handle on that channel.
- **Other extensions' items reach two menus only:** Edit > Copy As (`menuBar/edit/copy`), and File > Share (`file/share`, behind a proposed API). They can be read from `packageJSON`, built-ins included, with titles already localized. `vscode.extensions.all` shows only the caller's own extension host, so a local-side extension doesn't see the remote side's extensions without the proposed API `extensionsAny`.
- **At build time the source gives the whole table,** about 245 items in the desktop menubar with group, order, title, `when` and `toggled`. It takes the TypeScript type checker, not a syntax pass: half the command ids and most `when` clauses are constant references, the View menu's view entries come from a loop, nine Help entries depend on `product.json`, and the recent list and the update item are spliced in by code. The installed bundle is a poor source (mangled ids, titles as indexes, no source map).
- **A second route, read but not run:** start VS Code once with a native title bar and a throwaway `--user-data-dir`, and read `lastKnownMenubarData` from its `storage.json`. It's the evaluated menubar for one state, with labels, checks and default keybinding labels, without the `when` and `toggled` expressions.
- **It moves little:** 1.130 to 1.140 (10 weeks) added and removed nothing. 1.100 to 1.140 (17 months) added 16 items and removed 4, three or four of them visible to a user.
- **State can't be read:** no context key value, no `when` evaluation. Most of the menubar's 84 `when` clauses are constant on desktop Linux or follow from the workspace folders and settings. Checkmarks that mirror a setting: Auto Save, Column Selection Mode, Status Bar, Minimap, Breadcrumbs, Sticky Scroll, Render Control Characters, Menu Bar, and the radio groups Render Whitespace, Activity Bar Position, Tab Bar and Editor Actions Position. Not readable: Full Screen, Zen Mode, Centered Layout, the side bars and Panel, Title Bar, Word Wrap, Panel Position and Alignment.
- **Forks:** Code - OSS and VSCodium have the same menubar code and differ only by `product.json`, which an extension can read at `vscode.env.appRoot`. Cursor is closed source and wasn't checked. 1.140's agent sessions window has a menubar of its own, and an extension can't tell it's in one.

### Handed on

- **[07](07-source-of-the-menu-structure.md):** type-checked extraction, the native-title-bar snapshot, or extraction with the snapshot as its check; one table for each VS Code version or one table that drops items whose command is missing; whether titles are localized; whether the sessions window gets a menu.
- **[08](08-what-the-menu-shows-beyond-structure.md):** the checkmark list above; what to do with a contributed item whose `when` can't be evaluated; whether Copy As and Share are worth `--enable-proposed-api` in a remote window.
- **[10](10-thin-end-to-end-slice.md):** one real run of the snapshot route would confirm it.
