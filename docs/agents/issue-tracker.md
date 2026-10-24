# Issue tracker: Linear

Tickets, specs and maps live in Linear, in the project **camino** of the team **Camino**. Use the `linear` CLI ([schpet/linear-cli](https://github.com/schpet/linear-cli)); it reads its key from `LINEAR_API_KEY`, and `--help` on each command gives its flags. It has no default team: pass `--team CMN` to `issue create` and `issue query`. Name a ticket by its Linear identifier, as `CMN-12`.

## Layout

- **Epic**: a milestone of the project, named by the epic's slug (`overview`, `wallpaper`, …). `linear milestone list --project camino` lists them.
- **Spec and map**: project documents, one spec and one map per epic where the epic has them. `linear document list --project camino` lists them.
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

Only a ticket with the `ready-for-agent` label blocks another. Use Linear's native relation (`linear issue relation add <ISSUE> blocked-by <BLOCKER>`) for such a ticket, and none for any other. When a blocker loses `ready-for-agent`, remove its relations (`relation delete`, with the same arguments).

## Operations

- **Create**: `linear issue create --team CMN --project camino --milestone <epic> --label <Type> --label <triage> --title <title> --description-file <file> --yes`.
- **Read**: `linear issue view <ID>`, which shows its comments too.
- **List**: `linear issue query --team CMN --project camino`, filtered by `--state`, `--label` or `--search`. `issue list` shows only your own unstarted issues.
- **Comment**: `linear issue comment add <ID> --body-file <file>`.
- **Edit part of a body**: the CLI has no patch, and `--description-file` replaces the whole body. Take the body from `linear issue view <ID> --json` (`.description`), change that copy, and write all of it back with `linear issue update <ID> --description-file <file>`.
- **Change state or labels**: `linear issue update <ID>` with `--state`, `--add-label` or `--remove-label`. `--label` replaces every label.
- **Edit a spec or map**: the same as a body: `linear document view <slug> --raw`, change the copy, and `linear document update <slug> --content-file <file> --yes`. The slug is the last part of the document's URL.

## When a skill says "publish to the issue tracker"

Create the spec or map as a project document, and the tickets as issues in the epic's milestone (create the milestone first if the epic is new).

## When a skill says "fetch the relevant ticket"

`linear issue view <ID>` with the identifier the user gives. If the user gives an old number, `linear issue query --team CMN --project camino --search "[<NN>]"`, and take the issue whose title starts `[<NN>]`: the search also matches bodies.

## Wayfinding operations

Used by `/wayfinder`.

- **Map**: the epic's map document (Notes / Decisions-so-far / Fog).
- **Child ticket**: an issue in the epic's milestone, with the question in the body and a Type label (`research`, `prototype`, `grilling`, or `implementation` for a task).
- **Blocking**: native relations, under the rule in "Blocking" above. A ticket is unblocked when every blocker is Done.
- **Frontier**: the milestone's issues in Todo that have no blocker left open and no assignee; the oldest wins.
- **Claim**: `linear issue update <ID> --assignee @me --state "In Progress"`, before any other work.
- **Resolve**: append the answer to the body under an `## Answer` heading, set Done, then append a context pointer (gist + link) to Decisions-so-far in the map document.
