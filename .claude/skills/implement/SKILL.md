---
name: implement
description: "Implement a spec or tickets in a git worktree, merge it to master, then delete the worktree."
disable-model-invocation: true
---

Implement the work described by the user in the spec or tickets, in a git worktree.

If the user passes a ticket reference, fetch it from the issue tracker and state its title before starting. If the reference is ambiguous, ask.

Then claim each ticket in Linear, before you create the worktree: assign it to `me` and set its state to **In Progress** (see `docs/agents/issue-tracker.md`).

## Set up the worktree

The checkout you start in is the **main checkout**. The work branches from `master` and merges back into `master`.

Create a branch named after the work, in a worktree that is a sibling of the repository:

    git worktree add -b <branch> ../<repo>-<branch> master

Setup is done when `cargo check --workspace --all-targets` passes in the worktree. Do all of the work below there, with absolute paths into the worktree.

## Batch the lookups

Start from the ticket. It names the files, the seams and the ADRs of the work. Read all of them in one turn, as parallel tool calls, and explore past them only where they leave a gap.

Before each step, list everything the step needs to read: files, greps, test output. Ask for all of it in one turn. A read waits for an earlier result only when it depends on that result (open the file the grep found).

## Build

Call the Skill tool with "tdd" where possible, at pre-agreed seams. Run `cargo check` regularly.

Do not run the full test suite, run only the relevant tests.

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

In the main checkout, check out `master` and `git merge <branch>`. Use /resolving-merge-conflicts on conflicts. The main checkout can hold uncommitted changes from before the run; leave them as they are. If git refuses the checkout or the merge because of them, stop and tell the user.

Once the merge is committed, delete the worktree and its branch:

    git worktree remove ../<repo>-<branch>
    git branch -d <branch>

Clean-up is done when `git worktree list` no longer shows the worktree and `git branch` no longer shows the branch. If the merge does not complete, keep the worktree and tell the user its path.

## Close the tickets

When the work came from a ticket, set the ticket's state in Linear after the merge to `master` (see `docs/agents/issue-tracker.md`):

- **Done**: every acceptance criterion is met. Add a comment that names the commits on `master`.
- **In Review**: the code is in, but part of the work stays open. Add a comment that starts `Left:` and names what is left.

If the merge to `master` does not complete, leave the ticket's state as it is.

Report in five lines: what changed, which tests cover it, what stays open.
