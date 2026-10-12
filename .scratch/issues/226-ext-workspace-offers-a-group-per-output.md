# ext-workspace offers a group per output

Epic: per-output-workspaces
Type: implementation
Status: ready-for-agent
Blocked by: 230
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

"ext-workspace and the toplevel protocols" in [the spec](../per-output-workspaces/spec.md).

- **A group per output**, each with that output's workspaces: the same names and grid coordinates in each, and `active` by the group's output. A group is made when its output is enabled and removed with it, with its workspaces. Ticket 66's one group holding every output is what the key off still gives, and a change of the key swaps one for the other in a running session.
- **`activate` on a workspace** switches that workspace's output, and makes no output the Active output by itself.
- **A workspace's id** names its output and its number, stable across a restart for the same monitor.
- **The Xfce toplevel protocol** (ADR 0013): a toplevel's handle enters the workspace of its window's output's group, and leaves and enters as the window changes workspace or output. The refresh order around ext-workspace's refresh stays.

## Acceptance criteria

- [ ] Headless tests with a test client and two outputs: two groups, each with one output and N workspaces; a switch on one output changes `active` in its group only; `activate` on a workspace of the second group switches the second output.
- [ ] An output removed takes its group and workspaces away with the protocol's events in order, and one added gets a group.
- [ ] A toplevel's Xfce handle is in the right group's workspace, and follows a drag across outputs and a move between workspaces.
- [ ] With the key off the client sees one group with every output, as today, and a change of the key at run time moves a bound client from one to the other without a protocol error.
- [ ] The panel's window list, unpatched beyond `0003`, shows the windows showing on any output, checked in a Session and recorded here. If it doesn't, the fix is part of ticket 227.

## For the implementing agent to settle

Recorded in the Done section:

- the id's form, and what stands for the output in it (a saved layout knows a monitor by make, model and serial, or by its connector);
- whether a group's capabilities stay empty.
