# A weekly canary for VS Code's menubar

Epic: global-menu
Type: implementation
Status: open: the code is in; the workflow's first runs on GitHub (the first two criteria) and the Xvfb question are the developer's
Milestone: none: after v0.1
Blocked by: [118](118-the-menu-is-vs-codes.md)

## What to build

[ADR 0019](../../docs/adr/0019-vs-code-s-menu-table-is-a-snapshot-of-its-own-native-menubar.md), under "The menu table" in [the spec](../vscode-global-menu/spec.md). The developer hears within a week that a VS Code release changed its menubar.

- **A job in the weekly workflow** downloads the newest stable build, takes the snapshot with [118](118-the-menu-is-vs-codes.md)'s command, and compares its structure with the committed table.
- **On a difference it fails and opens or updates an issue** with the diff, as the workflow's other canaries report. Nothing commits by itself.
- **If Electron can't run under Xvfb in the workflow's container,** the command is run locally when the package updates, and the spec and ADR 0019 are amended to say so.

## Acceptance criteria

- [ ] The job passes against the version the table was made from.
- [ ] Against a table with an item taken out by hand, it fails and the issue has the diff. A second failing run updates that issue.
- [x] A difference in what the command strips (`enabled`, `checked`, Open Recent's entries, a profile's name, a keybinding label) isn't a difference.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] Whether Electron runs under Xvfb in the weekly workflow's container, with its sandbox on or off.

## Notes

- Forks have no canary.

## Done (2026-10-09)

- **The comparison is the snapshot command's `--check`,** so the canary takes the snapshot with 118's command as asked. It writes nothing, compares the structure of the snapshot with `menu/table.json` (`structureDiff` in `tools/menubar.mts`) and prints a unified diff of the two outlines, one line per item with its label and id. Version, commit and keybinding labels aren't structure.
- **Tested first, in `test/canary.test.mts`,** against a stand-in `code` as `snapshot.test.mts` does: the same table passes; an item cut from the table fails with the diff and leaves the table alone; `enabled`, `checked`, Open Recent's entries, the profile's name and a keybinding label make no difference. That's the third criterion. The suite is 75 tests, all passing, and typecheck is clean.
- **`--check` skips the stale-annotations check:** a dropped annotated command already shows in the diff as a missing item.
- **The job is `vscode-menubar-canary` in `weekly.yml`,** in `archlinux:latest` like the other canaries, with Electron's libraries as `visual-studio-code-bin` depends on them, Xvfb, and a 1 GB `/dev/shm`. It downloads from `update.code.visualstudio.com`. The report job copies the other canaries': it finds the open issue titled "VS Code menubar canary failing", then comments on it or creates it, and the body holds the diff, or says the run didn't get as far as comparing. That script was run here with a fake `gh`: the first failure creates the issue and later ones comment on it.
- **A dispatch input `vscode-version`** (default `latest`) lets the job run against the table's own 1.140.0, which is how the first criterion gets checked.
- **Not run here:** VS Code's download is blocked by this container's proxy, so neither Electron nor the workflow ran. Still to do by the developer on GitHub: dispatch Weekly with `vscode-version: 1.140.0` (first criterion); then do it twice on a branch with an item cut from `menu/table.json` (second criterion). The first of those runs answers the Xvfb question with the sandbox off, which is how `snapshot.mts` already runs it. The sandbox-on case needs `--no-sandbox` dropped by hand. If Electron won't run, the spec and ADR 0019 are amended as the ticket says.
- **The Rust suite wasn't run:** `pango-sys` won't build here. Nothing outside `caminito-vscode/` and the workflow changed.
