# The layout's windows ease to their new slots when one leaves

Epic: overview
Type: implementation
Status: ready-for-agent
Blocked by: 249
Spec: [Overview](../overview/spec.md)

## Why

When a window leaves the Overview's layout, dropped on another workspace or onto another output, the layout is laid out again and the windows left jump to their new slots. Next to 249's dropped window easing into its cell, the jump looks like a fault.

Decided in a grilling session with the developer on 2026-10-14, kept apart from 249 to keep that one to the dragged window.

## What to build

Under "The pointer" in [the spec](../overview/spec.md).

- **When a window leaves an output's layout**, each window that stays there eases from its old slot to its new one, in 250 ms on the Workspace slide's ease-out.
- A window that joins a layout, by a drop from another output, does the same for the windows already there.
- Another change of the layout part-way eases each window from where it's drawn to its newest slot.
- It's cut to its end by the Overview closing or by a switch of that output's workspace.
- With `overview-animation` off, or the minimise animation's three keys not as the transition needs them, the windows take their new slots in one frame, as today.

## Acceptance criteria

- [ ] Headless tests that read the dump: after a drop on another workspace's cell, the windows left are part-way between their old and new slots, and at their new slots at the end.
- [ ] A second drop part-way eases from where they're drawn.
- [ ] With two outputs, the layouts of both outputs ease after a drag from one to the other.
- [ ] With `overview-animation` off, the new slots are taken in one frame.
- [ ] A break script shows each new test can fail.
- [ ] The tests of 241 to 249 pass unchanged.
