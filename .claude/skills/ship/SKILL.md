---
name: ship
description: "Implement a ticket in its own worktree, merge it to master, and resolve it."
disable-model-invocation: true
---

Ship one Linear ticket (see `docs/agents/issue-tracker.md`): build it on a branch of its own, land it on `master`, and close it. For several tickets, ship them one at a time, blockers first.

`<id>` below is the ticket's identifier in lower case, as `cmn-12`. Linear links a branch whose name holds the identifier to the ticket.

## 1. Claim and worktree

Fetch the ticket with `get_issue`, assign it to `me` and set it In Progress. Then, from the main checkout, branch from `master` into a sibling worktree:

```sh
git worktree add ../camino-<id> -b <id> master
```

Every later step but the Linear calls runs inside `../camino-<id>` until step 3. Done when `git -C ../camino-<id> status` shows branch `<id>`, clean.

## 2. Build

Follow [`implement`](../implement/SKILL.md) in the worktree, with the ticket as the spec, but skip its ticket-state step: step 4 does it after the merge. Check scripts go in `.scratch/checks/<ID>/`. Done when every acceptance criterion is met or named as unmet with its reason, the relevant tests are green, and the work is committed on `<id>`.

## 3. Merge

1. In the worktree, merge `master` into `<id>`. On conflict, use /resolving-merge-conflicts. If anything came in, rerun typechecking and the tests relevant to the ticket's change and to what came in; done when green.
2. In the main checkout, on `master` with a clean tree (otherwise stop and ask the human):

   ```sh
   git merge --no-ff <id> -m "Merge branch '<id>' (<ID>)"
   ```

3. Remove the worktree and the branch: `git worktree remove ../camino-<id>` then `git branch -d <id>`.
4. Remove the ticket's build directory in the OrbStack machine, which is several GB and nothing uses after the merge: `orb -m camino bash -c 'rm -rf ~/target-<id>'`. Remove only this ticket's `target-<id>`: other sessions build in their own at the same time.

Done when `master`'s log shows the merge commit, `git worktree list` no longer shows the worktree, and `~/target-<id>` is gone from the machine.

## 4. Resolve the ticket

With `save_issue` on the ticket:

1. Tick the met acceptance criteria, and append a `## Done (<YYYY-MM-DD>)` section with `patch`: one bold-led bullet per decision a later reader needs (what was tested first, what was chosen and why, what wasn't run), and the merge commit.
2. Set the state:
   - Every acceptance criterion met: Done.
   - Any left unmet (it needs a Session, a person, or another ticket): In Review, with a comment `Left: <what's left and whose it is>`, as in `Left: the checklist item is CMN-19's`.

Done when `get_issue` shows the section and the state. Report the merge commit and the ticket's state; pushing `master` is the human's call.
