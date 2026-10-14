# The Overview in a real Session, by hand

Epic: overview
Type: implementation
Status: ready-for-human
Blocked by: 242, 243, 245, 246
Spec: [Overview](../overview/spec.md)

## What to do

Add the Overview's items to [checklist.md](../../docs/testing/checklist.md), and run them in a Session on the tty backend, with one monitor and with two. An agent can write the items. The run is the developer's.

## Acceptance criteria

- [ ] [checklist.md](../../docs/testing/checklist.md) has the items: open and close with the shortcut, Esc and a click on the empty area; choose a window with a click and with the arrow keys and Enter; choose a minimised window; close a window with a middle click; open a window while it is open; switch with a click on the bar and with the bindings; drag a window onto a workspace, onto the other monitor and onto nothing; reverse the transition part-way; turn `overview-animation` off with `gsettings`; lock the session while it is open; Spanning workspaces.
- [ ] The transition is smooth at the monitor's refresh rate, and a video in a window keeps playing in the layout.
- [ ] The size of the bar and the rate of a window that only the bar shows look right, or the developer changes them and records it in the spec.
- [ ] The shortcut clashes with nothing in a MATE session.
