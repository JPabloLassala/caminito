# Placement modes

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's placement modes for new windows ([05](05-v01-marco-feature-cut.md)). Today Policy places every window as Marco's `automatic` mode does.

- **`placement-mode`:**
  - `automatic`: as today;
  - `pointer`: centred on the pointer, kept within the work area of the output it's on;
  - `manual`: the new window follows the pointer until a click, or a key, puts it down.
- **`center-new-windows`:** new windows go in the middle of the work area.
- **Windows that aren't placed:** a window the client positions (an XWayland window with a position hint, 05), a transient placed on its parent, and a fullscreen or maximised window keep today's rules, as in Marco.

## Acceptance criteria

- [ ] Policy tests, written before the code: each mode for a first window, a window on a busy workspace, and one near an output's edge; `center-new-windows` with each mode; the windows that aren't placed.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: each mode from its key; a manual placement put down with a click, and with Return; a key change applying to the next window.
- [ ] `placement-mode` and `center-new-windows` leave `UNSUPPORTED`; their rows in `docs/parity.md` say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: each mode with a native and an X11 window, on two outputs. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section:

- **How manual placement takes the pointer and the keyboard:** which keys move it, put it down, and cancel it, and what a cancel leaves (the window where the automatic mode would put it, or where the pointer is).
- **Other outputs:** whether a manual placement can cross to another output, and which output `pointer` and `center-new-windows` use.
- **What Marco's `center-new-windows` does to a transient.**

**For the developer:** none.
