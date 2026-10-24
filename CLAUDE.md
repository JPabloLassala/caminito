# Caminito

## Tickets

The issue tracker is Linear: the project **camino**, team **Camino**, through the Linear MCP tools, or the `linear` CLI where they aren't loaded.

- An epic is a milestone of the project; its spec and map are project documents.
- A migrated ticket's title starts with its old number, as `[217] …`, so "ticket 217" means that issue. The numbers 01 to 13 are both `caminito-v0.1`'s and `vscode-global-menu`'s: take the milestone into account for those.
- Only a `ready-for-agent` ticket blocks another.
- `.scratch/` is the read-only archive of the old markdown tickets, specs and check scripts, which docs and ADRs still link to.

The full conventions, with the states and the operations, are in [issue-tracker.md](docs/agents/issue-tracker.md).

## Agent skills

### Issue tracker

Linear, project camino. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default triage roles, as labels of Linear's "Triage" group, and `wontfix` as the Canceled state. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
