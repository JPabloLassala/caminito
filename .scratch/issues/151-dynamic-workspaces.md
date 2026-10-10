# Dynamic workspaces

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, and the X11 variants of the headless tests, which need an Xwayland that starts under the fixture, are the developer's to run with `tools/check`; the checklist item is 173's
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

- [x] Policy tests, written before the code: a window opened on the last empty workspace adds one; the last window leaving a workspace removes it; the count's bounds; the active workspace removed; turning the key off and on.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: the ext-workspace client sees each workspace added and removed, with its coordinates in the Workspace grid; the key absent from the schema leaves everything as today.
- [x] `dynamic-workspaces` leaves `UNSUPPORTED` if 147 put it there, and its row in `docs/parity.md` says honoured, with the tests above.
- [x] The checklist has an item, tagged M8: with the key on (set with `gsettings` on a schema that has it, or skipped and said so with 1.29.0), opening and closing windows grows and shrinks the pager. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

The headless tests are written, and pass on xdg windows; their X11 variants weren't run, as Xwayland doesn't start where the ticket was done (see the Done section).

## Open questions

**For the developer**, to settle before building:

- **A key Arch's Marco doesn't have.** Support `dynamic-workspaces` now, read where the schema has it, or put it on the roadmap until Arch ships a Marco that has it. The proposal is to support it, as [104](104-screenshots.md) did `command-area-screenshot`.
- **Whether Caminito writes `num-workspaces` back** when it adds or removes one, as the pager's preferences show the count from that key ([97](97-pager-count-and-names.md)), or keeps the count to itself and leaves the key alone. Marco's master's answer is the default proposal.
- **With [150](150-workspace-up-down-prev-and-wrap-style.md)'s wrapping:** whether the empty last workspace is wrapped to.
- **With names:** whether `name-N` keys move when a workspace in the middle is removed.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: when Marco's master adds and removes workspaces, and where it moves the windows of one it removes.

## Comments

**Triage (2026-10-10), decided by the shipping agent without the developer, for the developer to confirm:**

- **A key Arch's Marco doesn't have:** the proposal. `dynamic-workspaces` is supported now, read where the installed schema has it, as 104 did `command-area-screenshot`; with Marco 1.29.0 it's off.
- **Whether Caminito writes `num-workspaces` back:** yes, Marco's master's answer. Its `request_workspace_count_change` (screen.c:1138-1168) sends itself `_NET_NUMBER_OF_DESKTOPS`, whose handler writes the key (display.c:2566-2575), after each workspace it adds or removes.
- **With 150's wrapping:** the empty last workspace is wrapped to like any other. Marco's master has nothing special for it: wrapping goes by the count, and the empty one is counted.
- **With names:** the `name-N` keys don't move. Marco keeps a name by its number (`meta_prefs_get_workspace_name`, prefs.c:2257-2268), and nothing in its dynamic code touches them.

## Done (2026-10-10)

- **What Marco's master does** (`dynamic_workspaces_idle_callback`, screen.c:1228-1358 at `8522ed1`), run on an idle after any window joins or leaves a workspace and when the key changes: a workspace is empty when it has no window but sticky ones, the desktop and docks being always sticky; with none empty and fewer than 36 it adds one at the end; it fills each empty workspace, from the first, with all the windows of the next one that has any, so those with windows close up in order; then it removes every empty workspace past the first of those at the end, moving what's left on them (sticky windows only) to the first remaining workspace with windows, or the first, and activates that if the active one was removed. So there's always one empty workspace, the last, and the windows of a removed workspace are never moved: it's the windows of those after an emptied one that move down.
- **Tests first:** the policy tests (`caminito-policy/src/tests/dynamic_workspaces.rs`) and the translation's two were written before the code and failed to build on the missing `Config` field. Only the first policy test was then seen to fail on an assertion, with a first slice of the code; the rule went in whole after it, so of the others only two failed on their first run: the count's, on a mistake of the test's, and the removed active workspace's, which had no focus until the default focus was given. A proptest op turns the key on and off among the other operations, and the invariants hold.
- **Where it lives:** the rule is `caminito-policy/src/policy/dynamic_workspaces.rs`, run first in `apply`, so after every event, as Marco's idle runs after the windows move. While the key is on, policy leaves `num-workspaces` unread: a stale echo of a count Caminito wrote would otherwise merge two workspaces' windows. Turning it off keeps the count, which the key has by then.
- **Decided beyond Marco:** the active workspace follows its windows when they move down, and the focus with them, as the ticket asks; Marco keeps the active index, which then shows the next workspace's windows and loses the focus. An emptied active workspace stays where it is, as in Marco, and shows the windows that move into it. An active workspace that's removed activates the first, Marco's `safe_target`, with the default focus and the Window switcher closed, as a switch does. Sticky windows don't keep a workspace, as in Marco, though sticky doesn't take effect until 155.
- **The write-back** is in the refresh pass (`Caminito::refresh_num_workspaces`): while the key is on and the count differs from `num-workspaces`, the count is written once, until the key changes. A change of `num-workspaces` meanwhile, from the pager's preferences or `_NET_NUMBER_OF_DESKTOPS`, is answered with the count, where Marco applies it until the next window comes or goes.
- **Names:** with the key on, the translation passes all 36 names on, since the count can reach any of them; policy names each workspace from them by its number.
- **The headless tests** (`caminito/src/tests/dynamic_workspaces.rs`) run on Marco master's schema, vendored under `tests/schemas/marco-master/` and compiled by the fixture's `with_marco_master_settings` only, so the other tests keep 1.29.0's as 147 decided. They see each workspace added and removed through ext-workspace with its coordinates, the windows after an emptied workspace move down with the view and the focus, and `num-workspaces` follow; on 1.29.0's schema nothing changes.
- **Not run here:** Xwayland doesn't start in the container this was done in, so the X11 variants of the three `on_both_kinds!` tests failed at "Xwayland starts", with the 238 other X11-tier tests; their snapshots are the xdg variants', which ext-workspace sends the same for either kind. The tests run: `caminito-policy`'s whole suite (its dynamic-workspaces tests and the proptest invariants among them), the translation's tests, `caminito/src/tests/dynamic_workspaces.rs`, and the `parity`, `settings`, `ext_workspace` and `root_properties` modules beside what this touched; the workspace's whole suite was also run, under `dbus-run-session`, before and after merging 148, 154 and 157 in, with every failure an X11-tier one at "Xwayland starts". After that the developer asked for the ticket's own tests only. Clippy is clean but for caminito-theme's three `manual_range_contains` lints, untouched here. The checklist item *Dynamic workspaces grow and shrink the switcher* is for 173's closing run, and is skipped with Marco 1.29.0.
- **The review** was done by the shipping agent itself, on both axes in turn, as it had no sub-agents to run them in: the switcher left open when a removed workspace moves the view, and two wrong citations of Marco's lines, were fixed.
