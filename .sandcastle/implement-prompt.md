# Ticket {{ID}}: {{TITLE}}

{{BODY}}

## Comments

{{COMMENTS}}

## Linked documents

The Linear documents the ticket links to, such as its spec:

{{DOCS}}

# Task

Build ticket {{ID}} above. You are in a sandbox: the repository is checked out on a branch named
after {{ID}}, in lower case, which you commit to. The host does everything else, so:

- Don't use Linear or any issue tracker. The ticket, its comments and the documents it links to
  are above, and the host claims and resolves it from your report.
- Don't create worktrees, switch branches, merge or push.
- Don't follow the repo's `ship` or `implement` skills as a whole. Follow
  `.claude/skills/implement/SKILL.md`'s "Batch the lookups" and "Build" sections only.

Read `CLAUDE.md`, `CONTEXT.md`, `CODING_STANDARDS.md` and the ADRs the ticket names first.
Build test-first with the `tdd` skill where the ticket's seams allow. Check scripts go in
`.scratch/checks/{{ID}}/`. Run `cargo check --workspace --all-targets` and the tests relevant
to the change, not the whole suite; `tools/check --deps` needs sudo and is not for here.
Run every command in the foreground, under `timeout`: don't start background tasks or wait for
them, as nothing tells you when one finishes here.

Commit the work on the branch, with messages in the style of `git log --oneline -20`.

A criterion that needs a Session, a person, real hardware or another ticket stays unmet:
name it in `left`. If you can't build the ticket at all (it's unclear, or a blocker isn't
really done), commit nothing and report `blocked`.

# Report

End with one block, the last thing you write, of JSON inside `<result>` tags:

<result>{"status": "done", "met": [1, 2, 3], "decisions": ["**Tested first**: …", "**Chose** … because …"], "left": "", "note": ""}</result>

- `status`: `done` (every acceptance criterion met), `in-review` (committed, some left), or
  `blocked` (nothing committed).
- `met`: the acceptance criteria met, numbered as the ticket's checkboxes are, from 1.
- `decisions`: one bold-led sentence per decision a later reader needs: what was tested first,
  what was chosen and why, what wasn't run.
- `left`: what's left and whose it is, as in `the checklist item is CMN-19's`, for `in-review`.
- `note`: why, for `blocked`.
