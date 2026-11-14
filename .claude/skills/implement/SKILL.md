---
name: implement
description: "Implement a spec or tickets in a git worktree, merge it to master, then delete the worktree."
disable-model-invocation: true
---

Implement the work described by the user in the spec or tickets, in a git worktree.

The tickets named in the arguments, fetched when this skill loaded: each ticket with its comments, parent and sub-issues, its relations, and the last comment of each ticket that blocks it. That comment names what the blocker built.

```!
.claude/skills/implement/scripts/fetch-tickets.sh <<'IMPLEMENT_ARGUMENTS'
$ARGUMENTS
IMPLEMENT_ARGUMENTS
```

State each ticket's title before starting. If a reference is ambiguous, or a ticket could not be fetched, ask. An old number, as "ticket 217", names the issue whose title starts `[217]`: find its identifier as `docs/agents/issue-tracker.md` says. To fetch another ticket later, run `.claude/skills/implement/scripts/fetch-tickets.sh CMN-<NN>...`.

The Linear commands of this skill are all you need from the tracker. For any other operation, see `docs/agents/issue-tracker.md`.

## Set up the worktree

The checkout you start in is the **main checkout**. The work branches from `master` and merges back into `master`.

Name a branch after the work. In one turn, as parallel calls, claim each ticket and create the worktree from the main checkout:

    linear issue update CMN-<NN> --assignee @me --state "In Progress" --no-input
    .claude/skills/implement/scripts/start-worktree.sh <branch>

Skip this if the worktree already exists. The script creates the worktree as a sibling of the repository, seeds its `target/` from `~/.cache/camino-target-seed` so the dependencies are not built again, and runs `cargo check --workspace --all-targets` in it. Its last line of output is the worktree path.

Never point two worktrees at one target directory with `CARGO_TARGET_DIR`: Cargo takes one worktree's build as fresh for the other, and its tests run the other's code.

Setup is done when the script exits 0. Do all of the work below in the worktree, with absolute paths into it.

After the first test build, refresh the seed if it is missing or older than a week. Run this in the worktree:

    S=~/.cache/camino-target-seed; [ -n "$(find "$S" -maxdepth 0 -mtime -7 2>/dev/null)" ] || { cp -a target "$S.tmp.$$" && rm -rf "$S" && mv "$S.tmp.$$" "$S" && touch "$S"; }

## Batch the lookups

Start from the ticket. It names the files, the seams and the ADRs of the work. Read all of them in one turn, as parallel tool calls, and explore past them only where they leave a gap.

Before each step, list everything the step needs to read: files, greps, test output. Ask for all of it in one turn. A read waits for an earlier result only when it depends on that result (open the file the grep found).

This holds for the whole run, not only the start. A turn with one read is the exception: before you send it, name what else you will read next, and send that too. In particular:

- When a grep or `ls` returns several files you will open, open them all in the next turn.
- Read several paths from another branch in one Bash call: `for p in a.c b.c; do echo "== $p"; git show <branch>:$p; done`.
- When a test fails, read the test and the code it exercises in the same turn.

## Build

Call the Skill tool with "tdd" where possible, at pre-agreed seams. The seams that the ticket names are pre-agreed.

Do not run the full test suite. In each red and green run, run only the test of the slice: `cargo test -p <crate> <test name>`. Before each commit, run the tests of the modules that the work changed, with a filter on the module names.

Do not run `cargo check` before a test run: the test build compiles the same code and reports the same errors. Use `cargo check` only where you run no test, and once before you commit, always as `cargo check --workspace --all-targets`: `-p` makes Cargo check many of the dependencies again.

A green that fails: read the whole failure output before the next edit. After two failed greens on the same test, stop the edits. Re-read the test and the code path it exercises, name the cause, then edit.

## Commit and review

Commit your work to the worktree branch.

Call the Skill tool with "code-review". Give it:

- the fixed point: `master`
- the diff command: `git diff master...HEAD`, run in the worktree
- the spec: the ticket text you fetched at the start, or the spec the user gave
- the standards sources: `CLAUDE.md`, `CONTEXT.md`, `docs/adr/` and `CODING_STANDARDS.md`, at the repo root
- the output of the last test run and of the last `cargo check`, for both sub-agent prompts

Fix the findings you accept, and commit the fixes as a second commit.

## Merge and clean up

From the main checkout, run:

    .claude/skills/implement/scripts/finish-worktree.sh <branch>

The script checks out `master`, merges the branch, deletes the worktree and the branch, and prints the commits it added to `master`. Keep that list for the ticket comment.

Clean-up is done when the script exits 0. If it stops, the worktree stays and the output says why:

- Conflicts: use /resolving-merge-conflicts in the main checkout, then run the script again.
- The main checkout can hold uncommitted changes from before the run; leave them as they are. If git refuses the checkout or the merge because of them, stop and tell the user the worktree path.

## Close the tickets

When the work came from a ticket, set the ticket's state in Linear after the merge to `master`. Set the state and add the comment in one turn, as parallel calls:

    linear issue update CMN-<NN> --state "<state>" --no-input
    linear issue comment add CMN-<NN> --body-file - --yes --no-input

- **Done**: every acceptance criterion is met. The comment names the commits on `master`.
- **In Review**: the code is in, but part of the work stays open. The comment starts `Left:` and names what is left.

If the merge to `master` does not complete, leave the ticket's state as it is.

Report in five lines: what changed, which tests cover it, what stays open.
