# Checkmarks and the items that follow the workspace

Epic: global-menu
Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [118](118-the-menu-is-vs-codes.md)

## What to build

[ADR 0020](../../docs/adr/0020-vs-code-s-menu-shows-only-the-state-its-extension-can-read.md), under "What the menu shows" in [the spec](../vscode-global-menu/spec.md). The panel's menu shows a checkmark where it mirrors a setting, and the items tied to the workspace state come and go with the workspace's folders. Both follow a change without the menu being reopened.

- **The annotations' other two kinds:** a setting's checkmark or radio value, and a workspace state (no folder, a folder, a multi-root workspace).
- **Twelve checkmarks, four of them radio groups,** each read with no resource or language scope. Editor Actions Position's Title Bar follows `workbench.editor.showTabs` as well. Menu Bar is on when `window.menuBarVisibility` is `classic` or `visible`.
- **An item whose state can't be read is a plain item,** and every item is enabled.
- **Pushed updates:** a change of an annotated setting, of the workspace's folders, and of the installed extensions (the list of commands). An update is sent only if something differs.
- **Coalescing:** updates within a short time are sent as one. A change of structure is a `LayoutUpdated`, and a change of a checkmark alone is an `ItemsPropertiesUpdated`. If [120](120-keybinding-labels.md) or [121](121-open-recent.md) was built first, the coalescing is there already.

## Acceptance criteria

- [x] Through the seam: each of the twelve reads its setting, and a radio group has one item on.
- [x] Through the seam: a change of an annotated setting sends one `ItemsPropertiesUpdated`, and a change that leaves the result as it was sends nothing.
- [x] Through the seam: adding a second folder shows the multi-root items with one `LayoutUpdated`, whose revision grows.
- [x] Through the seam: several changes at once are sent as one update.
- [x] Through the seam: installing an extension that adds a command the table names brings its item back.
- [x] Nothing is built on `AboutToShow` and there's no timer.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] Whether a setting read with no scope is what VS Code's `config.*` key holds.
- [ ] How the applet draws a radio item.
- [ ] What a `LayoutUpdated` does to a panel menu that's open.
- [ ] Whether an `ItemsPropertiesUpdated` sent within 100 ms of a layout reply has to be sent again. The importer drops them for that long.

## Comments

**2026-10-08, built.** The annotations' two other kinds are in `menu/annotations.json`, and `fit` in `src/menu-table.mts` reads them. The update path is in `src/global-menu.mts`, and `DbusMenu.update` in `src/dbusmenu.mts` decides which signal a change is. The tests are `test/state.test.mts`.

- **A checkmark or radio item** is `{ "check": <condition> }` or `{ "radio": <condition> }`. A condition is `{ "setting", "is" }`, `{ "setting", "not" }`, `{ "setting", "in": [...] }`, or `{ "all": [...] }` and `{ "any": [...] }` of others. They were read from VS Code 1.140.0's own `toggled` expressions, and the values (`hidden` for Activity Bar Position's Hidden, whose command ends in `.hide`; `titleBar`; `none` for hidden tabs) are its enums'. The setting is read with `workspace.getConfiguration().get(name)`, no scope. A value that's missing, or of another type than the condition's (a string where a boolean is expected), makes the condition unreadable, and the item is then plain: no `toggle-type` and no `toggle-state`.
- **Editor Actions Position.** Title Bar is VS Code's `location == titleBar || (location == default && showTabs == none)`. VS Code hides Tab Bar when the tabs are hidden, which ADR 0020 doesn't let the menu do, so Tab Bar here is `location == default && showTabs != none`, and the group keeps one item on. That condition is the menu's own, not VS Code's, and the ADR doesn't record it.
- **A workspace state** is `{ "workspace": { "<label>": ["empty" | "folder" | "workspace"] } }`, by label, since Close Folder and Close Workspace share `workbench.action.closeFolder` (118's note). An item whose label isn't named is always shown. The states are VS Code's `workbenchState` values. The window's is `workspace` when `workspace.workspaceFile` is set (an untitled workspace too), else `folder` when there's a folder, else `empty`. The snapshot command now also fails on a label its command's items lack. Close Folder and Close Workspace are the only items annotated: they're the only difference between the two snapshot states, and the empty state was never snapshotted, so an empty window may show items VS Code's own menubar hides there.
- **Ids are the table's.** Every entry of the table, dropped or not, is numbered depth first, so an item keeps its id when others come and go, and a click with an id from before a `LayoutUpdated` still runs that item or nothing.
- **Updates.** A change of an annotated setting (`onDidChangeConfiguration` with `affectsConfiguration`), of the folders (`onDidChangeWorkspaceFolders`) or of the extensions (`extensions.onDidChange`, which also lists the commands again) schedules one rebuild 50 ms later (`COALESCING_MS`), and every event before it joins it. The rebuilt tree is compared with the served one: a change of ids or of their nesting is a `LayoutUpdated` with the next revision and parent 0; a change of properties only is one `ItemsPropertiesUpdated` with the changed properties and the removed ones; else nothing. The delay is a one-shot after an event, not a timer that wakes the extension: with no event, nothing runs.
- **Report Issue and Backup and Sync Settings** ([118](118-the-menu-is-vs-codes.md)) come back at the first change of extensions after their commands are registered, not before. No event is added for them.

**Found in review, from VS Code 1.140's source and not run: a change of workspace state restarts the extension host.** `enterWorkspace` in `workspaceEditingService.ts` (electron-browser) stops the extension hosts and starts them again, and does a full reload for a remote window. A window that goes from a folder to a multi-root workspace, as when a second folder is added, gets a new host. That host builds the menu from the start and claims again. The running host never hears a folders event that changes Close Folder to Close Workspace. So the criterion "adding a second folder ... with one `LayoutUpdated`" holds through the seam but not in a real editor. There, the change comes as a reload of the menu, which empties it until the new host claims (ADR 0018). The folders event still updates the menu when the state stays the same, for example a third folder in a workspace. No item follows that today. The ADR and the spec assume the event drives the change. Whether to amend them is the developer's call. The nested run should check the restart and re-claim.

**Not run here:** the four questions above need the nested run with the applet.

