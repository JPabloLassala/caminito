# A Camino nested in the Session is too slow to use with foot and xterm in it

Type: implementation
Status: open
Milestone: none (proposed: it gates no milestone, since a nested run is the development loop's and not a user's)
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

A Camino started with `tools/dev-nested` inside the Session, so Camino on winit in a window of Camino on the tty, was too slow to use once foot and xterm were open in it. The developer saw it on 2026-10-06 in [73](73-packaging-and-closing-m5.md)'s closing run, on `camino-git r225.c221860-1`, and moved that run's last two items to the Session itself.

- **What's known:**
  - It was usable earlier in the same run, with pluma, caja and gtk3-demo in it.
  - Measured while it was slow: the nested Camino at 0.9% of a CPU and the Session's at 9%, so neither was busy.
  - Nothing repeated in the nested Camino's log. It had `Xwayland glamor: GBM Wayland interfaces not available` once, at its start.
  - The nested runs before that day had KDE as their host, and weren't slow.
- **Not known:** what was slow, typing, the pointer or redrawing; whether it's foot, xterm or the two together; and whether it's slow with another host.
- **To find out:**
  - What is slow, and with which client alone.
  - Whether the nested Camino's frames wait on the Session's frame callbacks, and at what rate they come: a nested window that isn't drawn isn't sent them.
  - The same nested run with KDE or another compositor as its host, which says whether it's the nested Camino's fault or the host's.

## Acceptance criteria

- [ ] What is slow is described: which input or drawing, with which clients, and on which hosts.
- [ ] The cause is named, and whether it's the nested Camino's, the host Camino's, or a client's.
- [ ] If Camino changes, a test written before the change where a seam can show it, and the nested run with foot and xterm is usable in the Session, by the developer.
