# The Overview's keys, the middle click and windows that come and go

Epic: overview
Type: implementation
Status: ready-for-agent
Blocked by: 241
Spec: [Overview](../overview/spec.md)

## What to build

The Overview used from the keyboard, and kept right while it is open, under "The keys", "The pointer" and "Which windows" in [the spec](../overview/spec.md). It builds on ticket 241's Overview.

- **The selection** is policy's. The focused window is selected at opening, or none when no window is focused. The selected window is marked in the layout.
- **The arrow keys** move the selection to the nearest window that way in the layout. With none selected, an arrow key selects the first.
- **Enter** chooses the selected window, as a click on it does: it is activated, unminimised if it must be, and the Overview closes. With none selected, Enter does nothing.
- **A middle click on a window** closes that window. The Overview stays open.
- **A window that opens** while the Overview is open joins the layout, and **one that closes** leaves it. The others take their new places in one frame, until 243. If the selected window closes, none is selected.
- The debug dump shows the selection.

## Acceptance criteria

- [ ] Policy's tests, with no protocol objects: the selection at opening with a focused window and with none, each arrow key in a layout of two rows, an arrow key at the layout's edge, Enter with a selection and with none.
- [ ] Headless tests that read the dump: the arrow keys and Enter activate the window chosen and close the Overview.
- [ ] A middle click on a window sends it a close request, and the Overview is still open.
- [ ] A window that maps while it is open is in the layout's next frame, and one that goes isn't. The selection is none after its window goes.
