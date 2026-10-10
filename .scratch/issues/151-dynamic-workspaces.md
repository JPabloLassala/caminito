# Dynamic workspaces

Epic: caminito-v0.1
Type: implementation
Status: needs-triage
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`dynamic-workspaces`, a key of Marco's master that Arch's Marco 1.29.0 doesn't have: with it on, workspaces are added and removed as windows come and go, as Marco's master does ([05](05-v01-marco-feature-cut.md), [09](09-gsettings-mapping.md)).

- **Where the schema has it.** The key is read where the installed schema has it, as the settings translation reads `command-area-screenshot` ([104](104-screenshots.md)). With Marco 1.29.0 it's off.
- **The count** stays within 1 to 36, and the workspaces it adds or removes reach ext-workspace and the pager as a change of `num-workspaces` does today ([97](97-pager-count-and-names.md)).
- **Windows** on a workspace that's removed go where Marco's master puts them, and the focus with them.
- **Names** follow their workspace, not their number, if Marco's master does.

## Acceptance criteria

- [ ] Policy tests, written before the code: a window opened on the last empty workspace adds one; the last window leaving a workspace removes it; the count's bounds; the active workspace removed; turning the key off and on.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: the ext-workspace client sees each workspace added and removed, with its coordinates in the Workspace grid; the key absent from the schema leaves everything as today.
- [ ] `dynamic-workspaces` leaves `UNSUPPORTED` if 147 put it there, and its row in `docs/parity.md` says honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: with the key on (set with `gsettings` on a schema that has it, or skipped and said so with 1.29.0), opening and closing windows grows and shrinks the pager. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, to settle before building:

- **A key Arch's Marco doesn't have.** Support `dynamic-workspaces` now, read where the schema has it, or put it on the roadmap until Arch ships a Marco that has it. The proposal is to support it, as [104](104-screenshots.md) did `command-area-screenshot`.
- **Whether Caminito writes `num-workspaces` back** when it adds or removes one, as the pager's preferences show the count from that key ([97](97-pager-count-and-names.md)), or keeps the count to itself and leaves the key alone. Marco's master's answer is the default proposal.
- **With [150](150-workspace-up-down-prev-and-wrap-style.md)'s wrapping:** whether the empty last workspace is wrapped to.
- **With names:** whether `name-N` keys move when a workspace in the middle is removed.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: when Marco's master adds and removes workspaces, and where it moves the windows of one it removes.
