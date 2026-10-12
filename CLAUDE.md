# Caminito

## Tickets

The issue tracker is local markdown under `.scratch/`.

- Every ticket is one file at `.scratch/issues/<NN>-<slug>.md`, whatever its epic. Its check scripts sit beside it, in `.scratch/issues/<NN>-checks/` or as `<NN>-*.sh`.
- An `Epic:` line at the top of a ticket's header names its epic: `caminito-v0.1`, `global-menu`, `vscode-global-menu`, `night-light`, `about-caminito`, `system-shortcuts`, `debian-packaging`, `wallpaper` or `per-output-workspaces`.
- An epic's map and spec are in `.scratch/<epic>/` (`map.md`, `spec.md`).
- The numbers 01 to 13 are both `caminito-v0.1`'s and `vscode-global-menu`'s, so read the `Epic:` line when a ticket is named by one of those numbers alone.
- A new ticket takes the next number after the highest in `.scratch/issues/`.

The full conventions are in [issue-tracker.md](docs/agents/issue-tracker.md).

## Agent skills

### Issue tracker

Issues are local markdown: one file per ticket in `.scratch/issues/`, with each epic's spec and map in `.scratch/<epic>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default triage roles, each written under its own name on a ticket's `Status:` line. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
