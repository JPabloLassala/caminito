# Caminito

## Tickets

The issue tracker is local markdown under `.scratch/`.

- Every ticket is one file at `.scratch/issues/<NN>-<slug>.md`, whatever its epic. Its check scripts sit beside it, in `.scratch/issues/<NN>-checks/` or as `<NN>-*.sh`.
- An `Epic:` line at the top of a ticket's header names its epic: `caminito-v0.1`, `global-menu` or `vscode-global-menu`.
- An epic's map and spec are in `.scratch/<epic>/` (`map.md`, `spec.md`).
- The numbers 01 to 13 are both `caminito-v0.1`'s and `vscode-global-menu`'s, so read the `Epic:` line when a ticket is named by one of those numbers alone.
- A new ticket takes the next number after the highest in `.scratch/issues/`.

The full conventions are in [issue-tracker-local.md](.claude/skills/setup-matt-pocock-skills/issue-tracker-local.md).
