# Where the menu's structure comes from and how it's kept current

Epic: vscode-global-menu
Type: grilling
Status: resolved
Blocked by: 01
Map: [VS Code's global menu](../vscode-global-menu/map.md)

## Question

Is the menu's structure written by hand, extracted from VS Code's source when the extension is built, read from the installed VS Code, or a mix? What happens when the installed VS Code is newer or older than the list, or is a fork with other items? Who notices that a release moved something, and how?

## Answer

Decided in a grilling session on 2026-10-08 and recorded as [ADR 0019](../../docs/adr/0019-vs-code-s-menu-table-is-a-snapshot-of-its-own-native-menubar.md). Two runs fed it: the snapshot route tried under Xvfb, and a read of how language packs key their strings. Their outputs weren't kept.

- **The snapshot route works.** With a native title bar and a throwaway profile, `lastKnownMenubarData` appears about 5 s after start and settles at about 7 s (300 entries, separators and submenu parents included), without a clean exit. It has the parts the source registers by code. An item hidden by its `when` is absent: with no folder open, only Close Folder went.
- **The table is that snapshot,** taken by a developer command with a folder open and with a multi-root workspace open, merged, and stripped of the run's state (`enabled`, `checked`, Open Recent's entries, the profile's name). Open Recent listed the developer's real folders despite the fresh profile.
- **A hand-written annotations file,** keyed by command id, holds what a snapshot can't: the checkmarks that mirror a setting, the `product.json` gates, the items that follow the workspace state. A stale annotation fails the command.
- **It's committed,** made from Microsoft's build at the `visual-studio-code-bin` version, with that version and commit recorded.
- **One table for every version and fork.** The extension drops items whose command is missing, then empty submenus, and collapses separators. Product-dependent items follow the running editor's `product.json`. A fork's own items aren't shown, and Cursor is untested.
- **A newer VS Code is served silently,** with one log line naming both versions.
- **A weekly canary notices a release:** it snapshots the newest stable build and opens an issue with the diff. The table is remade by hand. If Electron can't run under Xvfb in the workflow's container, the command is run locally when the package updates.
- **Titles are English.** Localizing is possible later: a language pack keys its strings by module and key, and a 1.131 Spanish pack resolved all 233 `mi*` keys of 1.140. That was read, not run.
- **Filled by code:** Open Recent's entries at run time (08), no list of profiles, "Export Profile..." without the name, no update item.
- **The agent sessions window gets the ordinary menu,** with no detection.

### Handed on

- **[08](08-what-the-menu-shows-beyond-structure.md):** the snapshot holds 108 default keybinding labels, which could stand in where ticket 02's route fails; the annotations file is where the checkmark list lives; Open Recent's entries are stripped from the table and filled at run time; Copy As was absent from the snapshot because nothing contributes to it.
- **[10](10-thin-end-to-end-slice.md):** whether the extension runs in the agent sessions window, and what the ordinary menu does there.
- **The spec:** the developer command, the table and annotations formats, the tolerance rule and the canary are the extension's side. Whether Electron runs under Xvfb in the weekly workflow's container is to be tried when the canary is built.
