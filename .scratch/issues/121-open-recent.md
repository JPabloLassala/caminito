# Open Recent in the panel's menu

Epic: global-menu
Type: implementation
Status: resolved
Milestone: none: after v0.1
Blocked by: [118](118-the-menu-is-vs-codes.md)

## What to build

[ADR 0020](../../docs/adr/0020-vs-code-s-menu-shows-only-the-state-its-extension-can-read.md), under "What the menu shows" in [the spec](../vscode-global-menu/spec.md). File > Open Recent lists what VS Code's own does, and an entry opens.

- **The layout:** Reopen Closed Editor, the first ten folders and workspaces, the first ten files, More… and Clear Recently Opened, from `_workbench.getRecentlyOpened`.
- **A label** is the path with the home directory as `~`, a workspace and a remote marked as VS Code marks them, cut in the middle at 120 characters, with each underscore doubled.
- **Opening:** a folder or workspace with `vscode.openFolder`, a file with `_files.windowOpen`, and with `vscode.open` if that fails.
- **If the list can't be read,** the submenu keeps its fixed items.
- **Pushed updates:** focus gained, a change of the editor tabs, and the return of a command the extension ran to open an entry or clear the list. The list isn't re-read on focus lost. Coalescing as in [119](119-checkmarks-and-workspace-items.md), built here if this ticket comes first.

## Acceptance criteria

- [x] A label's cases, as a function of its entry: a path under home, a workspace file, a remote, a long path, a path with an underscore.
- [x] Through the seam: a client reads ten and ten in order between the fixed items, from a fake list of more.
- [x] Through the seam: an `Event` on a folder, on a file, and on a file when `_files.windowOpen` fails runs the right command.
- [x] Through the seam: the list is re-read on each of the three events, and an update is sent only if it differs.
- [x] Through the seam: with the internal command failing, the fixed items are served.

## To run, and to answer here

Both were run on 2026-10-09, under "From 121's nested run". Neither result contradicts an ADR.

- [x] Whether a change of tabs comes with every change of the recent list. **No.** It comes with a file opened through `_files.windowOpen` or from the command line. It doesn't come with an entry taken out of the list or with another window's change, and those show at the next change of tabs or focus gained.
- [x] Whether the window regains focus, and so re-reads, when a panel menu closes. **Yes,** on Escape and on a click away. With a submenu open it regains it earlier, as the submenu opens, and not again at the close ([146](146-a-panel-submenu-gives-the-keyboard-back.md)).

## Notes

- Under `--extensionTestsPath` confirmation dialogs confirm themselves and `_files.windowOpen` opens a new window, so that run tells nothing here.
- Ctrl-clicking an entry to open it in a new window isn't possible from the panel.

## Done (2026-10-09)

- **Built test first at the seam, with the label as a function.** `test/recent-label.test.mts` has the label's cases. `test/recent.test.mts` covers what a client reads, what a click runs, the three events and the fallback, through the fake editor and a private bus. Breaking the focus and command-return re-reads by hand failed four of those tests. In `caminito-vscode/`, `npm run typecheck` is clean, `npm test` passes all 91 tests on Node 22, and `npm run build` bundles.
- **Where the code is.** The list, its labels and its items are in `src/recent.mts`. `fit` in `src/menu-table.mts` takes a `before` that puts the entries in front of More…. Its separator collapsing then takes care of an empty folders or files group. The reader and the three events are in `src/global-menu.mts`, on 119's 50 ms coalescing, so a focus change and a tab change together read the list once.
- **Ids.** An entry's id is fixed by its kind and URI for as long as the extension runs, numbered after the table's last entry. A reordered list, as when an opened entry moves to the top, is a `LayoutUpdated`, and a click on an id from an older layout still opens that entry. Because of this, an entry the list holds twice is shown once, which VS Code's own menu doesn't do. The id map only grows, with one number for each distinct entry seen.
- **Opening.** A folder or a workspace is `vscode.openFolder` with its URI. A file is `_files.windowOpen` with `[[{ fileUri }], { remoteAuthority: … ?? null }]`, the openable and options VS Code's own menubar passes to `openWindow`, as read and not run. If that command fails, the line `can't run _files.windowOpen, so vscode.open is run: …` is logged and `vscode.open` opens the URI. The list is re-read when the clicked command returns, for `vscode.openFolder`, `_files.windowOpen` and `workbench.action.clearRecentFiles`, whether it succeeded or not. `vscode.open` doesn't trigger it, since only the fallback runs it, after `_files.windowOpen` has.
- **When the list can't be read** (the command fails, or its answer has no `workspaces` and `files` arrays), Open Recent has its fixed items. One line is logged, and it isn't logged again until a read has succeeded. An entry that isn't shaped as one is left out.
- **Labels, and what's a guess.** An entry's own `label`, if it has one, is shown as it is. Otherwise: a local path with home as `~`; a workspace file's path without `.code-workspace`, then ` (Workspace)`; a remote's path as it is, since its home can't be known, then ` [<kind>]`; and any other scheme as its whole URI. The cut keeps 59 characters, `…`, and the last 60, which is VS Code's `truncateMiddle`. It counts UTF-16 units, so a surrogate pair can be split. Each underscore is doubled after the cut. **The remote suffixes are a guess.** They come from each remote extension's label formatter, which an extension can't read. The table is `SSH: <host>`, `WSL: <distro>` and `Tunnel: <name>`, and `Dev Container`, `Container` and `Codespaces` with no name, since their authorities are hex. An unknown authority is shown whole. VS Code's formatters probably name the container, so a nested run with a remote should compare the two.
- **Not handled:** an untitled workspace in the list, which VS Code labels "Untitled (Workspace)". It shows here as its path plus ` (Workspace)`. As read, VS Code doesn't add untitled workspaces to the list, which hasn't been run either.
- **Focus gained re-reads only the recent list.** The window's profile in the spec's "focus gained" line is [120](120-keybinding-labels.md)'s.
- **Not run here:** both questions under "To run". They need VS Code, a Wayland desktop and the applet, and none is in this container. They're this slice's in the spec's "What has to be run", so they stay here and aren't handed to [124](124-vs-code-in-a-real-session.md).

## From 119's nested run (2026-10-09)

VS Code 1.140.0 and the applet's package 25.04-4 in a nested Caminito, with the scripts in [119-checks](119-checks/). The two questions under "To run" are still open.

- **A change of the list never reached the panel, and it's fixed.** The `LayoutUpdated` named parent 0, and the importer fetches again only the menu the signal names, so the panel's Open Recent kept an entry the served menu had dropped. It now names Open Recent ([119](119-checkmarks-and-workspace-items.md), branch `ticket-119`), and the panel's list was the served one after `vscode.removeFromRecentlyOpened` and a change of tabs. The test of the focus-gained read pins the parent.
- **An open Open Recent keeps its old entries.** Held open by hand while an entry was taken out, the submenu stayed open and went on showing it. Opened again, it didn't have it.
- **Toward the first question, one case:** a file opened with `showTextDocument` brought a change of tabs and the claims that follow it, and no `LayoutUpdated` in the 2 s after. Whether the list had the file by then wasn't looked at.
- **The list isn't the profile's alone.** With a new `--user-data-dir`, Open Recent listed folders of the developer's own editor and of other nested runs. `vscode.removeFromRecentlyOpened` from the scratch editor took entries out of that list, about a dozen of old `/tmp` runs. Where the list is kept wasn't looked into.

## From 121's nested run (2026-10-09)

VS Code 1.140.0 and the applet's package 25.04-4 in a nested Caminito, with the scripts in [121-checks](121-checks/). Its driver logs each change of the tabs and of the window's focus, and a poller in the extension host logged each change of the list. The panel's menus were opened by hand: GTK doesn't pop a menu up from an accessibility click ("no trigger event for menu popup"), and nothing here injects a pointer.

**A change of tabs and the list:**

- **A file opened in the editor doesn't join the list.** `vscode.open`, a pinned tab, an edit and save, and closing the editor each changed the tabs and left the list as it was.
- **A file opened through `_files.windowOpen` joins it, and the menu had it 32 times out of 32.** The tabs change first and the list 12 to 474 ms later, which is after the 50 ms of coalescing more often than not. The read still returns the new list, because the editor answers it after the open: a read made as the command returned took 44 to 182 ms and had the file. So the read on the command's return is safe too.
- **`code --reuse-window <file>` is the same:** tabs, then the list, then a `LayoutUpdated` naming Open Recent.
- **An entry taken out with `vscode.removeFromRecentlyOpened` brings no change of tabs.** The served menu kept it until the next change of the active tab. This is what the remove button of More…'s picker does, as read; the button itself wasn't pressed.
- **A second window opening a folder changed the list and the first window's menu didn't have it** until that window was clicked: focus gained, and a `LayoutUpdated` 120 ms later.
- **Not run:** Clear Recently Opened from the command palette, which would bring no change of tabs either. The list is shared with the developer's own editor (below), so it wasn't cleared.

**Focus and the panel's menu:**

- **The window loses focus as a panel menu opens,** 15 to 130 ms after its `opened`, and regains it when the menu closes, on Escape and on a click on the editor. Five menus, five times.
- **End to end:** with File open, an entry among the ten shown was taken out. On Escape the window gained focus, a `LayoutUpdated` naming Open Recent followed 110 ms later, and the panel's list was the served one, without the entry.
- **With a submenu the focus comes back early.** As Open Recent opened, and once as another submenu did, the window regained focus within 30 ms, with the menus still up, and nothing more came at the close. Escape then didn't close the menus. Three times out of three. It's [146](146-a-panel-submenu-gives-the-keyboard-back.md)'s. For this ticket it means the re-read happens as the submenu opens, and an entry clicked in it is covered by the read on its command's return.

**Left behind:** the run's own entries were taken out of the list at the end. Those of the earlier run under `/tmp/caminito-121.lx2jEp` are still in it.
