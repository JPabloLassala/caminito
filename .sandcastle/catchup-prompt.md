# Task

The branch `{{BRANCH}}`, which builds ticket {{ID}}, is reviewed, and the host is about to
merge it into `master`. Other tickets merged into `master` while it was being built, so catch
it up first. You are in a sandbox, on that branch. Don't use Linear, switch branches or push.

## The ticket

{{BODY}}

## What came into master

!`git log HEAD..master --oneline`

# Catch up with master

Merge `master` into the branch. On conflict, follow
`.claude/skills/resolving-merge-conflicts/SKILL.md`, keeping what both sides meant. Then run
`cargo check --workspace --all-targets` and the tests relevant to this ticket's change and to
what came in, and fix what broke. Commit the merge and any fixes.

Run every command in the foreground, under `timeout`: don't start background tasks or wait for
them, as nothing tells you when one finishes here.

If you can't make the branch green on top of `master`, run `git merge --abort` if the merge
isn't committed, and say why. Otherwise, when the branch has `master` in it and is green, output
<promise>COMPLETE</promise>.
