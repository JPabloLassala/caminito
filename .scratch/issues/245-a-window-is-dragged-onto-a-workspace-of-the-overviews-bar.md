# A window is dragged onto a workspace of the Overview's bar

Epic: overview
Type: implementation
Status: resolved
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

- [x] Policy's tests, with no protocol objects: a press and a release within `drag-threshold` is a click; a drop on a cell, on the active workspace's cell and on nothing; Esc during a drag.
- [x] Headless tests that read the dump: a window dragged onto another workspace's cell is on that workspace with the geometry it had, isn't in the layout, and the Overview is still open.
- [x] A window dropped on the empty area is in the layout at the place it had, on the workspace it had.
- [x] ext-workspace and the toplevel protocols report the window's new workspace.

## Done (2026-10-14)

- **Policy has the press and the drag** (`OverviewDrag` on policy's `Overview`): the window, where the button went down, where the pointer is, whether it's a drag and the cell under the pointer. It's past `drag-threshold` as the titlebar's drag is: `|dx|` or `|dy|` greater than it. Once a drag, it stays a drag wherever the pointer goes. The compositor tells policy each move with the cell under the pointer (`overview_pointer`), and the release (`overview_release`).
- **A click is now the release**, not the press: the left button's press on a window holds it, and its release within the threshold chooses the window. The middle click, a click on a cell and one on the empty area are as before, on the press.
- **The release is the Overview's first.** `overview_button` is now first in `on_pointer_button`'s chain, since `switcher_button` swallows every suppressed release. With the Overview open none of the other three can be open, so the order changes nothing else.
- **A drop on another workspace's cell** is `move_to_workspace`, the Window menu's move, so the window's transient tree goes with it and a sticky window stops being sticky. A drop on the active workspace's cell, on nothing or on a workspace past the count leaves it where it was, and a sticky window stays sticky there. The window is drawn back at its place at once, with no animation: the ticket asks for none.
- **Esc** ends a drag that is past the threshold, and the Overview stays open. With a press that isn't a drag yet, Esc closes the Overview, as before. My choice: the ticket says "during a drag". The release after is no drop and no click. The lock closes the Overview, which ends the drag. A drag ends too when its window leaves the list (it closes, or a workspace binding switches away from it).
- **Drawing:** the dragged window is drawn in front of the bar and the layout at its layout size, moved as far as the pointer moved, live as every window of the layout is (it stays in the layout, so it gets frame callbacks at the output's rate). Its place and its label are left empty. The cell under the pointer is marked with the theme's selected background at 40% over its picture (`TARGET_MARK`), so it differs from the active cell's border. My choice of look; 247 checks it in a Session.
- **The dump** has `drag`: `window`, `pointer`, `dragging`, `target` and `rect`, where it's drawn.
- **The protocols:** the headless test reads the workspace from an Xfce panel's toplevel handles, whose `workspace_enter` names ext-workspace's workspace handles, so one test covers both. foreign-toplevel itself has no workspace.
- **Tests:** policy's 14 new tests and the proptest's 4 new ops and invariant were written with the code, not before it, and passed first time. The headless tests too. To check that they can fail, `245-break.sh` breaks the drawing of the dragged window, the move on a drop, Esc and the pointer's moves on purpose: each break failed its tests (2, 4, 1 and 6).
- **What ran:** `caminito-policy`'s whole suite (735), and `caminito`'s tests filtered on `overview`, `switcher`, `window_menu`, `workspace_popup`, `button_bindings`, `modifier_click`, `grabs`, `frames`, `drag_icons`, `popups`, `input`, `xfce_foreign_toplevel`, `debug`, `state_dump`, `scene`, `lock`, `workspace_slide` and `deadlines` (810), with fmt and clippy at `-D warnings`. Not the full suite, by the standing rule. Not run in a real Session: that's 247's.
- **From the review, left as it is:** a drag that starts during the opening's 250 ms takes the window from its layout place, not from where the transition draws it, and a minimised one at full opacity. `overview_dragged` runs three times a frame. The tests read `workspace_of` from policy, as the file does elsewhere. No test has two outputs: that's 246's.
