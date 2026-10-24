# Task

Review the branch `{{BRANCH}}`, which builds ticket {{ID}}, and fix what you accept. You are
in a sandbox, on that branch. Don't use Linear, switch branches or push.

## The ticket

{{BODY}}

## Linked documents

The Linear documents the ticket links to, such as its spec:

{{DOCS}}

## Commits on the branch

!`git log master..HEAD --oneline`

# Review

Follow `.claude/skills/code-review/SKILL.md`, with:

- the fixed point: `master`
- the diff command: `git diff master...HEAD`
- the spec: the ticket above, and the documents it links to
- the standards sources: `CLAUDE.md`, `CONTEXT.md`, `docs/adr/` and `CODING_STANDARDS.md`
- the output of `cargo check --workspace --all-targets` and of the tests relevant to the change,
  which you run first

Run every command in the foreground, under `timeout`: don't start background tasks or wait for
them, as nothing tells you when one finishes here.

Fix the findings you accept, rerun those tests, and commit the fixes as one commit.

# Catch up with master

Then merge `master` into the branch. On conflict, follow
`.claude/skills/resolving-merge-conflicts/SKILL.md` if it's there, or resolve it by hand. If
anything came in, rerun `cargo check --workspace --all-targets` and the relevant tests, and fix
what broke.

When the branch is reviewed, merged with master and green, output
<promise>COMPLETE</promise>.
