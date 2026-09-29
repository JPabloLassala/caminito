# Clipboard and drag-and-drop

Type: implementation
Status: open
Milestone: M3
Blocked by: 35
Map: [Camino v0.1](../map.md)

## What to build

The clipboard, primary selection and drag-and-drop between X11 and Wayland clients, with cosmic-comp's focus gate ([11](11-xwayland-integration.md)).

- **Selection access:** `allow_selection_access` returns true only while an XWayland window has keyboard focus. The Wayland selection is pushed into X lazily, when an XWayland window next gains focus, so background X11 clients can't read the clipboard. The same holds for the primary selection.
- **Drag-and-drop:** Smithay's bridge. Camino's pointer and touch focus types implement `DndFocus`, and hit-testing uses `X11Surface::surface_under`.
- **Known bugs accepted (11):** XDND Ask and Private actions, `CLIPBOARD_MANAGER` `SAVE_TARGETS`, and Smithay #1953. No workarounds; a comment in `handlers/selection.rs` lists them.
- Clipboard persistence after an X11 app quits needs msd's data-control manager, so it's M5's checklist item (17).

## Acceptance criteria

- [ ] X11-tier tests, written before the code, with a Wayland test client and an x11rb client: a Wayland selection reaches the X client once one of its windows is focused, and not before; an X11 selection is readable by the Wayland client; an unfocused X client's `ConvertSelection` on `CLIPBOARD` gets nothing.
- [ ] Nested: text copied in xterm pastes into foot, and back; a file dragged from an X11 GTK3 app drops into a native one.
