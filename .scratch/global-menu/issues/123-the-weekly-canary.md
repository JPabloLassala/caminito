# A weekly canary for VS Code's menubar

Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [118](118-the-menu-is-vs-codes.md)

## What to build

[ADR 0019](../../../docs/adr/0019-vs-code-s-menu-table-is-a-snapshot-of-its-own-native-menubar.md), under "The menu table" in [the spec](../../vscode-global-menu/spec.md). The developer hears within a week that a VS Code release changed its menubar.

- **A job in the weekly workflow** downloads the newest stable build, takes the snapshot with [118](118-the-menu-is-vs-codes.md)'s command, and compares its structure with the committed table.
- **On a difference it fails and opens or updates an issue** with the diff, as the workflow's other canaries report. Nothing commits by itself.
- **If Electron can't run under Xvfb in the workflow's container,** the command is run locally when the package updates, and the spec and ADR 0019 are amended to say so.

## Acceptance criteria

- [ ] The job passes against the version the table was made from.
- [ ] Against a table with an item taken out by hand, it fails and the issue has the diff. A second failing run updates that issue.
- [ ] A difference in what the command strips (`enabled`, `checked`, Open Recent's entries, a profile's name, a keybinding label) isn't a difference.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] Whether Electron runs under Xvfb in the weekly workflow's container, with its sandbox on or off.

## Notes

- Forks have no canary.
