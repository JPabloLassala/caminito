---
name: ship
description: "Implement a ticket in its own worktree, merge it to master, and resolve it."
disable-model-invocation: true
---

Ship one ticket from `.scratch/issues/`: build it on a branch of its own, land it on `master`, and close it. For several tickets, ship them one at a time, in number order.

## 1. Worktree

From the main checkout, branch from `master` into a sibling worktree:

```sh
git worktree add ../camino-ticket-<NN> -b ticket-<NN> master
```

Every later step runs inside `../camino-ticket-<NN>` until step 4. Done when `git -C ../camino-ticket-<NN> status` shows branch `ticket-<NN>`, clean.

## 2. Build

Follow [`implement`](../implement/SKILL.md) in the worktree, with the ticket as the spec. Done when every acceptance criterion is met or named as unmet with its reason, the full test suite is green, and the work is committed on `ticket-<NN>`.

## 3. Resolve the ticket

In the worktree, so the merge carries it:

- Every acceptance criterion met: set `Status: resolved`.
- Any left unmet (it needs a Session, a person, or another ticket): set `Status: open: <what's left and whose it is>`, as in `Status: open: the code is in, and the checklist item is 136's`.

Then append a `## Done (<YYYY-MM-DD>)` section: one bold-led bullet per decision a later reader needs (what was tested first, what was chosen and why, what wasn't run). Commit it. Done when the ticket file on `ticket-<NN>` carries both.

## 4. Merge

1. In the worktree, merge `master` into `ticket-<NN>`. On conflict, use /resolving-merge-conflicts. If anything came in, rerun typechecking and the full suite; done when green.
2. In the main checkout, on `master` with a clean tree (otherwise stop and ask the human):

   ```sh
   git merge --no-ff ticket-<NN> -m "Merge branch 'ticket-<NN>' (ticket <NN>)"
   ```

3. Remove the worktree and the branch: `git worktree remove ../camino-ticket-<NN>` then `git branch -d ticket-<NN>`.
4. Remove the ticket's build directory in the OrbStack machine, which is several GB and nothing uses after the merge: `orb -m camino bash -c 'rm -rf ~/target-<NN>'`. Remove only `target-<NN>` of this ticket: other sessions build in their own at the same time.

Done when `master`'s log shows the merge commit, `git worktree list` no longer shows the worktree, and `~/target-<NN>` is gone from the machine. Report the merge commit and the ticket's final `Status:` line; pushing `master` is the human's call.
