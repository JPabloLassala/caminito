# ext-workspace offers a group per output

Epic: per-output-workspaces
Type: implementation
Status: open: the code is in, and the check of the panel's window list in a Session with two monitors is the developer's
Blocked by: 230
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

"ext-workspace and the toplevel protocols" in [the spec](../per-output-workspaces/spec.md).

- **A group per output**, each with that output's workspaces: the same names and grid coordinates in each, and `active` by the group's output. A group is made when its output is enabled and removed with it, with its workspaces. Ticket 66's one group holding every output is what the key off still gives, and a change of the key swaps one for the other in a running session.
- **`activate` on a workspace** switches that workspace's output, and makes no output the Active output by itself.
- **A workspace's id** names its output and its number, stable across a restart for the same monitor.
- **The Xfce toplevel protocol** (ADR 0013): a toplevel's handle enters the workspace of its window's output's group, and leaves and enters as the window changes workspace or output. The refresh order around ext-workspace's refresh stays.

## Acceptance criteria

- [x] Headless tests with a test client and two outputs: two groups, each with one output and N workspaces; a switch on one output changes `active` in its group only; `activate` on a workspace of the second group switches the second output.
- [x] An output removed takes its group and workspaces away with the protocol's events in order, and one added gets a group.
- [x] A toplevel's Xfce handle is in the right group's workspace, and follows a drag across outputs and a move between workspaces.
- [x] With the key off the client sees one group with every output, as today, and a change of the key at run time moves a bound client from one to the other without a protocol error.
- [ ] The panel's window list, unpatched beyond `0003`, shows the windows showing on any output, checked in a Session and recorded here. If it doesn't, the fix is part of ticket 227.

## For the implementing agent to settle

Recorded in the Done section:

- the id's form, and what stands for the output in it (a saved layout knows a monitor by make, model and serial, or by its connector);
- whether a group's capabilities stay empty.

## Done (2026-10-13)

- **Tested first.** The headless tests on two outputs in `caminito/src/tests/ext_workspace.rs` (from `pager_on_two_outputs` on) and `caminito/src/tests/xfce_foreign_toplevel.rs` (from `group_placements` on) were written before the code. At the first run one of them failed because the test dragged a window that was not showing; the test was wrong, not the code.
- **The id's form.** With the key on, a workspace's id is the output's connector and the workspace's number, as `DP-1:3`. With the key off the ids are `1` to `N`, as before. The connector stands for the output, not the make, model and serial: an id must be different for every workspace, since the Xfce protocol finds a toplevel's workspace by id, and two monitors of the same model often have the same serial or none. A connector is always different. The cost: the id is stable across a restart only while the monitor stays on the same connector, and a dock that numbers its connectors again changes it. No client that the panel uses stores an id.
- **A group's capabilities stay empty.** ext-workspace is read-only (06), so `create_workspace` does nothing, as before.
- **The protocol module takes groups.** `ExtWorkspaceState::refresh` takes a list of `Group`, each with an id, its outputs and its workspaces, and does not know if there is one group or one for each output. The handler makes one group with the id `""` when the key is off, and a group with the connector as its id for each enabled output when it is on.
- **The order of events when a group goes.** Each of its workspaces is sent the group's `workspace_leave` and then `removed`. Then the group is sent `output_leave` for each output, and then `removed`. Then come the new groups, and one `done`. A change of the key does all of this in one `done`. An output that output management disables leaves its group before its `wl_output` global goes, as ticket 91 needs.
- **`activate`.** In one `commit`, the last activation of each group counts. A workspace of an output's group switches that output through `Policy::switch_output_workspace`, so the Active output and the focus of a seat on another output stay. With the key off the path is the same as before (`Action::SwitchToWorkspace`). An `activate` with an id of the other kind, from before the key changed, is ignored.
- **The Xfce handle goes by the output the window shows by.** That is `Policy::output_showing`: the window's Main output, or its root's Main output for a transient. The window list compares a window with its group's active workspace, so this output gives the same answer as policy's own "is showing". A dialog that is dragged to another output stays in its parent's group. No headless test covers the dialog.
- **The key's change and the Xfce handle.** The handle leaves the old group's workspace before ext-workspace's refresh and enters the new one after it, with one `done` for each, as ADR 0013 has it for a workspace that goes. For a moment the panel has the window on no workspace.
- **Not covered: a different output with the same connector in one refresh pass.** If an output goes and another comes on the same connector before the next refresh, the group stays and the client gets only `output_leave` and `output_enter`. The headless backend gives each output a new name, so no test shows it.
- **From the code review.** The tests read the result through the pager's view and not through policy, a test destroys a group's handle before its output goes, and the public fields got their docs.
- **What ran.** In the OrbStack machine: `cargo test -p caminito --lib -- ext_workspace xfce_foreign_toplevel per_output_workspaces` (80 tests, all passed, the `::x11` ones included), `cargo fmt --all` and `cargo clippy --workspace --all-targets` (clean). The full suite was not run.
- **Not run: the Session check.** The last criterion needs the panel in a Session with two monitors. The OrbStack machine has no tty and the nested backend has one output, so it was not possible here. To check it: with a window on each monitor and the two monitors on different workspaces, the window list must show the windows that are showing on the two monitors and no others. If it does not, the fix is part of ticket 227.
