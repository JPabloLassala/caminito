# Open Recent in the panel's menu

Epic: global-menu
Type: implementation
Status: open: the code is in, and the two questions under "To run" are a nested run's, by hand
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

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] Whether a change of tabs comes with every change of the recent list.
- [ ] Whether the window regains focus, and so re-reads, when a panel menu closes.

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
