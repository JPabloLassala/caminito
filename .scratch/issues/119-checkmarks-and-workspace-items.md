# Checkmarks and the items that follow the workspace

Epic: global-menu
Type: implementation
Status: resolved
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

- [x] Whether a setting read with no scope is what VS Code's `config.*` key holds. It is (2026-10-09, below).
- [x] How the applet draws a radio item. As a checkmark: it's a radio menu item to AT-SPI, with the right one checked, and on screen it looks like a check item (2026-10-09).
- [x] What a `LayoutUpdated` does to a panel menu that's open. The menu stays open with its old items, and has the new ones when it's opened again (2026-10-09, below).
- [x] Whether an `ItemsPropertiesUpdated` sent within 100 ms of a layout reply has to be sent again. The importer drops them for that long. It has to, and now is (2026-10-09, below).

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


## Done (2026-10-09)

- **The code was already on `master`** (`e468cae`, `62a64f5`), so shipping it added nothing to `caminito-vscode/`. At `f6e3003`, `npm run typecheck` is clean and `npm test` passes all 68 tests, on Node 22 here.
- **The Rust workspace's suite wasn't run:** as for 118, `pango-sys` won't build without pango's development files, which this container lacks. Ticket 119 changes nothing outside `caminito-vscode/` and this file.
- **The nested run wasn't done here.** There's no VS Code, no Wayland compositor and no applet, so the four questions under "To run" stay unanswered. The same run should check the restart that review found: a folder window that becomes a multi-root workspace gets a new extension host, which claims the menu again.
- **Left open on the developer's call:** the folders criterion holds through the seam, but in a real editor the change comes as a restart, not a `LayoutUpdated`. Whether ADR 0020 and the spec should say so isn't decided here.

## The nested run (2026-10-09)

VS Code 1.140.0 and the applet's package 25.04-4 in a nested debug build of Caminito, driven by the scripts in [119-checks](119-checks/): `up.sh` leaves the session up, a driver extension runs timed calls of the editor's API in the extension host, and `look.py` reads the served menu and, through AT-SPI, the panel's.

- **A `LayoutUpdated` with parent 0 reached no submenu, and it's fixed.** The importer has one model for each menu, and a model fetches its layout again only when the signal names its own menu. For parent 0 it called `GetLayout(0, 1)` and nothing else, so the panel's Open Recent kept an entry the served menu had dropped. `DbusMenu.update` now sends one `LayoutUpdated` for each menu whose own items came, went or moved, all with the one new revision. With it the importer called `GetLayout(11, 0)` and `GetLayout(11, 1)` for Open Recent, and the panel's list was the served one. This was [121](121-open-recent.md)'s too.
- **A change of structure no longer covers the properties changed with it.** The menus not named aren't fetched again, so the same update sends an `ItemsPropertiesUpdated` first for the items that stay. The test that asked for one `LayoutUpdated` alone now asks for both.
- **An `ItemsPropertiesUpdated` soon after a layout reply is lost, and is now sent again.** `119-checks/dropped.sh` took an entry out of Open Recent and then gave Reopen Closed Editor, an item of that menu, another keybinding. Signals 91 ms and 98 ms after the importer's `GetLayout` never reached the panel's item; ones 116 ms, 154 ms, 212 ms and 438 ms after did. The drop is each model's own, for 100 ms after its own layout reply, and nothing reads the item again. `DbusMenu` now notes when each menu's layout was asked for, and a property that changes within 200 ms of it (`DROPPED_MS`) is sent a second time at the end of that time, as it is then. It's a one-shot after a change, like the coalescing. With it, signals 81 ms and 96 ms after reached the panel.
- **Radio items.** AT-SPI gives the four groups' items the role "radio menu item", with the served one checked, and the checks "check menu item". By eye, the developer's, the mark beside Activity Bar Position's Top is the same as the one beside Sticky Scroll.
- **The setting's scope, by eye.** A folder window with a text file in front, where the user's settings and the folder's give plain text the opposite value of Minimap, Sticky Scroll and Render Control Characters, and the folder's set Auto Save, Breadcrumbs and Render Whitespace over the user's. The panel's menu and the window's own menubar agreed on all of them, with Status Bar: what's read with no scope is what `config.*` holds, not the front editor's value. A folder's settings in a multi-root workspace weren't tried. `119-checks/scope.sh`, which was to compare the served toggles with `lastKnownMenubarData` of a native title bar, failed three times on its own waits and leftovers and gave no result.
- **An open menu, by hand.** A menu clicked through AT-SPI isn't mapped (`Failed to create popup because parent is not mapped`), so the developer held the panel's Open Recent open while an entry was taken out of the list. The `LayoutUpdated` for it went out 5 s after it opened and the importer fetched its layout at once. The submenu stayed open and went on showing the entry. Opened again, it didn't have it.
- **A change of workspace state restarts the host, as review found.** `updateWorkspaceFolders` added a second folder to the folder window. A new extension host started 1.2 s later and claimed, with no `LayoutUpdated` from the old one. The panel had no menu of the window 1 s after the call and had Close Workspace 2 s after.
- **Seen and not looked into:** with a new `--user-data-dir`, Open Recent listed folders of other profiles' runs. It may be [121](121-open-recent.md)'s to explain.
- **`nested/run.sh`** failed on a machine with `caminito-vscode-git` installed, whose link is among the built-ins it copies; its `ln` now replaces it.

At this point `npm run typecheck` is clean and `npm test` passes all 136 tests.

## Resolved (2026-10-09)

- **Closed by the developer** after the nested run: the four questions under "To run" are answered, and the two fixes are on branch `ticket-119`.
- **ADR 0020 and the spec aren't amended.** They still say the folders event drives the change from Close Folder to Close Workspace. In a real editor that change comes as a restart of the extension host and a new claim, which the run saw. No ticket is filed for it.
- **Not tried, and handed to nobody:** a folder's own settings in a multi-root workspace, for the setting's scope.
