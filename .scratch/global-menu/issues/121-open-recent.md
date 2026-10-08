# Open Recent in the panel's menu

Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [118](118-the-menu-is-vs-codes.md)

## What to build

[ADR 0020](../../../docs/adr/0020-vs-code-s-menu-shows-only-the-state-its-extension-can-read.md), under "What the menu shows" in [the spec](../../vscode-global-menu/spec.md). File > Open Recent lists what VS Code's own does, and an entry opens.

- **The layout:** Reopen Closed Editor, the first ten folders and workspaces, the first ten files, More… and Clear Recently Opened, from `_workbench.getRecentlyOpened`.
- **A label** is the path with the home directory as `~`, a workspace and a remote marked as VS Code marks them, cut in the middle at 120 characters, with each underscore doubled.
- **Opening:** a folder or workspace with `vscode.openFolder`, a file with `_files.windowOpen`, and with `vscode.open` if that fails.
- **If the list can't be read,** the submenu keeps its fixed items.
- **Pushed updates:** focus gained, a change of the editor tabs, and the return of a command the extension ran to open an entry or clear the list. The list isn't re-read on focus lost. Coalescing as in [119](119-checkmarks-and-workspace-items.md), built here if this ticket comes first.

## Acceptance criteria

- [ ] A label's cases, as a function of its entry: a path under home, a workspace file, a remote, a long path, a path with an underscore.
- [ ] Through the seam: a client reads ten and ten in order between the fixed items, from a fake list of more.
- [ ] Through the seam: an `Event` on a folder, on a file, and on a file when `_files.windowOpen` fails runs the right command.
- [ ] Through the seam: the list is re-read on each of the three events, and an update is sent only if it differs.
- [ ] Through the seam: with the internal command failing, the fixed items are served.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] Whether a change of tabs comes with every change of the recent list.
- [ ] Whether the window regains focus, and so re-reads, when a panel menu closes.

## Notes

- Under `--extensionTestsPath` confirmation dialogs confirm themselves and `_files.windowOpen` opens a new window, so that run tells nothing here.
- Ctrl-clicking an entry to open it in a new window isn't possible from the panel.
