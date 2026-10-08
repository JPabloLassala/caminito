# Issue tracker: Local Markdown

Issues and specs for this repo live as markdown files in `.scratch/`.

## Conventions

- One feature (an epic) per directory: `.scratch/<feature-slug>/`, which holds its spec and map
- The spec is `.scratch/<feature-slug>/spec.md`
- Tickets of every epic are one file per ticket at `.scratch/issues/<NN>-<slug>.md`, never a single combined tickets file. A new ticket takes the next number after the highest in the folder
- An `Epic: <feature-slug>` line at the top of each ticket's header names the epic it belongs to
- Triage state is recorded as a `Status:` line near the top of each issue file (see `triage-labels.md` for the role strings)
- Comments and conversation history append to the bottom of the file under a `## Comments` heading

## When a skill says "publish to the issue tracker"

Create a spec or map under `.scratch/<feature-slug>/` (creating the directory if needed), and a ticket under `.scratch/issues/` with its `Epic:` line.

## When a skill says "fetch the relevant ticket"

Read the file at the referenced path. The user will normally pass the path or the issue number directly.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a file with one **child** file per ticket.

- **Map**: `.scratch/<effort>/map.md` (the Notes / Decisions-so-far / Fog body).
- **Child ticket**: `.scratch/issues/NN-<slug>.md`, with `Epic: <effort>` in its header and the question in the body. A `Type:` line records the ticket type (`research`/`prototype`/`grilling`/`task`); a `Status:` line records `claimed`/`resolved`.
- **Blocking**: a `Blocked by: NN, NN` line near the top. A ticket is unblocked when every file it lists is `resolved`.
- **Frontier**: scan `.scratch/issues/` for files with `Epic: <effort>` that are open, unblocked, and unclaimed; first by number wins.
- **Claim**: set `Status: claimed` and save before any work.
- **Resolve**: append the answer under an `## Answer` heading, set `Status: resolved`, then append a context pointer (gist + link) to the map's Decisions-so-far in `map.md`.
