# A window is dragged onto a workspace of the Overview's bar

Epic: overview
Type: implementation
Status: ready-for-agent
Blocked by: 244
Spec: [Overview](../overview/spec.md)

## What to build

A window of the Overview's layout is dragged onto a workspace of the bar to move it there, on one output, under "The pointer" in [the spec](../overview/spec.md). It builds on ticket 244's bar. The drag to another output is 246.

- **A drag starts** when the pointer, pressed on a window of the layout, moves past `drag-threshold`. Before that, the release is a click, which activates the window. The drag is policy's state.
- **The dragged window** is drawn under the pointer, over the layout and the bar, and stays live. The cell under the pointer is marked as the drop's target.
- **Dropped on a cell of the bar:** the window goes to that workspace and keeps its geometry. It leaves the layout, and shows in its new cell. Dropped on the active workspace's cell, it returns to its place in the layout.
- **Dropped on nothing:** it returns to its place in the layout.
- **The Overview stays open** after any of them.
- A sticky window dropped on a cell is no longer sticky, as with the Window menu's move to a workspace.
- Esc during a drag ends the drag and returns the window, and the Overview stays open. The session lock ends both.
- The debug dump shows the drag: its window, the pointer's place and the target cell.

## Acceptance criteria

- [ ] Policy's tests, with no protocol objects: a press and a release within `drag-threshold` is a click; a drop on a cell, on the active workspace's cell and on nothing; Esc during a drag.
- [ ] Headless tests that read the dump: a window dragged onto another workspace's cell is on that workspace with the geometry it had, isn't in the layout, and the Overview is still open.
- [ ] A window dropped on the empty area is in the layout at the place it had, on the workspace it had.
- [ ] ext-workspace and the toplevel protocols report the window's new workspace.
