# A dragged window shrinks to the scale of the workspace it goes to

Epic: overview
Type: implementation
Status: ready-for-agent
Blocked by: 246
Spec: [Overview](../overview/spec.md)

## Why

A window dragged in the Overview stays at its layout size all the way to the bar, where it hides the cell it's aimed at. Dropped on a cell, it vanishes from under the pointer and appears in the cell's picture at once (245: "drawn back at its place at once, with no animation"). The developer wants it to shrink to the cell's scale, keeping its share of the workspace: a window that covers 90% of the output covers 90% of the cell, with its aspect.

Decided in a grilling session with the developer on 2026-10-14.

## What to build

Under "The pointer", "The transition" and "Settings" in [the spec](../overview/spec.md).

- **The scale of a cell** is its width over its output's width. A cell has its output's aspect (`bar.rs`), so a rectangle scaled by it on both axes covers the same share of the cell as it did of the output.
- **Over a cell**, the dragged window eases from its layout size to its real size times that cell's scale, under the pointer.
  - It shrinks about the point it was grabbed by: that point stays under the pointer.
  - Off the cell it eases back to its layout size. Entering or leaving part-way reverses from the size it's at, as the Overview's opening and closing do.
  - From one cell to another of the same bar, its size stays. Onto a cell of another output's bar, it eases to that bar's scale.
- **On the drop**, the window eases from where it's drawn under the pointer to where it's drawn next:
  - on another workspace's cell: its place in that cell's picture, its real rectangle on that workspace scaled into the cell. On another output's cell that's its new geometry there (246);
  - on the active workspace's cell, on nothing, after Esc, or onto another output's layout: its slot in that layout.
  - The cell's picture, or the slot, leaves it out until it's there, so it's never drawn twice.
- **Timing:** 250 ms on the Workspace slide's ease-out (`LENGTH`, `way_left`). With `overview-animation` off, or the minimise animation's three keys not as the transition needs them, the window takes each size and place in one frame, as today.
- **Policy moves the window at the release**, as now; only the drawing takes time. The drop is cut to its end by the Overview closing, a switch to the workspace it was dropped on, or a press on the window.
- **The dump** shows, for the drag and for a drop in progress, the rectangle the window is drawn at.

## Acceptance criteria

- [ ] Unit tests of the pure geometry: a window's rectangle scaled into a cell covers the same share of the cell as of the output, with its aspect; the shrink about the grab point keeps that point under the pointer; the target in another output's cell is its new geometry there.
- [ ] Headless tests that read the dump: a drag over a cell is at the cell's scale at the end and in between part-way; off the cell it's back at its layout size; reversed part-way it eases back from where it was.
- [ ] A drop on another workspace's cell ends at the window's place in that cell's picture, and the picture doesn't draw it before then; a drop on nothing ends at its slot.
- [ ] With two outputs: a drag onto the other output's bar takes that bar's scale, and its drop ends at its new geometry scaled into that cell.
- [ ] With `overview-animation` off, every size and place is reached in one frame.
- [ ] The Overview closing, a switch to the drop's workspace and a press on the window each cut the drop to its end.
- [ ] A break script shows each new test can fail.
- [ ] The tests of 241 to 248 pass unchanged, except where a drawn rectangle of the drag is now animated.
