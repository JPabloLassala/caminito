# Issue tracker: Linear

Tickets, specs and maps live in Linear, in the project **camino** of the team **Camino**. Use the Linear MCP tools (`mcp__linear-server__*`; load their schemas with ToolSearch). Name a ticket by its Linear identifier, as `CMN-12`.

## Layout

- **Epic**: a milestone of the project, named by the epic's slug (`overview`, `wallpaper`, …). `list_milestones` on the project lists them.
- **Spec and map**: project documents, one spec and one map per epic where the epic has them. `list_documents` on the project lists them.
- **Ticket**: one issue in the project, in its epic's milestone, with one label of the "Type" group (`implementation`, `grilling`, `research`, `verification`, `prototype`, or `Bug`).
- **Migrated tickets** keep their old number in the title, as `[217] Dynamic wallpapers from HEIC files`. "Ticket 217" in a commit, a doc or another ticket means the issue whose title starts `[217]`. The numbers 01 to 13 are both `caminito-v0.1`'s and `vscode-global-menu`'s, so take the milestone into account for those. New tickets have no number in the title.
- **The archive**: `.scratch/` holds the tickets as they were before the move, with the 140 resolved tickets that weren't migrated, and every check script. Read it for history; write new tickets in Linear only.
- **Check scripts** of a new ticket go in `.scratch/checks/<identifier>/`, and the ticket links to them.

## Status

| State | Meaning |
| --- | --- |
| Todo | Not started. Its triage role is a label (see `triage-labels.md`) |
| In Progress | Claimed: assigned and being worked on |
| In Review | The code is in, and what's left (a Session run, the developer's check, another ticket) is named in a comment that starts `Left:` |
| Done | Every acceptance criterion met |
| Canceled | Won't be done (the `wontfix` role) |

## Blocking

Only a ticket with the `ready-for-agent` label blocks another. Use Linear's native relation (`blockedBy` / `blocks` on `save_issue`) for such a ticket, and none for any other. When a blocker loses `ready-for-agent`, remove its relations.

## Operations

- **Create**: `save_issue` with `team: "Camino"`, `project: "camino"`, `milestone: <epic>`, the Type label, a triage label, and the body as Markdown.
- **Read**: `get_issue` with the identifier; `list_comments` for its comments.
- **List**: `list_issues` with `project: "camino"`, filtered by `state`, `label` or `query`.
- **Comment**: `save_comment`.
- **Edit part of a body**: `save_issue` with `patch` (`append`, `replace`), so the rest stays as it is.
- **Change state or labels**: `save_issue` with `state`, `addLabels` or `removeLabels`.
- **Edit a spec or map**: `save_document` with the document's URL as `id` and a `patch`.

## When a skill says "publish to the issue tracker"

Create the spec or map as a project document, and the tickets as issues in the epic's milestone (create the milestone first if the epic is new).

## When a skill says "fetch the relevant ticket"

`get_issue` with the identifier the user gives. If the user gives an old number, `list_issues` with `query: "[<NN>]"` in the project.

## Wayfinding operations

Used by `/wayfinder`.

- **Map**: the epic's map document (Notes / Decisions-so-far / Fog).
- **Child ticket**: an issue in the epic's milestone, with the question in the body and a Type label (`research`, `prototype`, `grilling`, or `implementation` for a task).
- **Blocking**: native relations, under the rule in "Blocking" above. A ticket is unblocked when every blocker is Done.
- **Frontier**: the milestone's issues in Todo that have no blocker left open and no assignee; the oldest wins.
- **Claim**: assign it to `me` and set In Progress, before any other work.
- **Resolve**: append the answer to the body under an `## Answer` heading, set Done, then append a context pointer (gist + link) to Decisions-so-far in the map document.
