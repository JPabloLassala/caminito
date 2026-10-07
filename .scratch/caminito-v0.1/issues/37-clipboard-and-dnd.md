# Clipboard and drag-and-drop

Type: implementation
Status: resolved
Milestone: M3
Blocked by: 35
Map: [Caminito v0.1](../map.md)

## What to build

The clipboard, primary selection and drag-and-drop between X11 and Wayland clients, with cosmic-comp's focus gate ([11](11-xwayland-integration.md)).

- **Selection access:** `allow_selection_access` returns true only while an XWayland window has keyboard focus. The Wayland selection is pushed into X lazily, when an XWayland window next gains focus, so background X11 clients can't read the clipboard. The same holds for the primary selection.
- **Drag-and-drop:** Smithay's bridge. Caminito's pointer and touch focus types implement `DndFocus`, and hit-testing uses `X11Surface::surface_under`.
- **Known bugs accepted (11):** XDND Ask and Private actions, `CLIPBOARD_MANAGER` `SAVE_TARGETS`, and Smithay #1953. No workarounds; a comment in `handlers/selection.rs` lists them.
- Clipboard persistence after an X11 app quits needs msd's data-control manager, so it's M5's checklist item (17).

## Acceptance criteria

- [x] X11-tier tests, written before the code, with a Wayland test client and an x11rb client: a Wayland selection reaches the X client once one of its windows is focused, and not before; an X11 selection is readable by the Wayland client; an unfocused X client's `ConvertSelection` on `CLIPBOARD` gets nothing.
- [x] Nested: text copied in xterm pastes into foot, and back; a file dragged from an X11 GTK3 app drops into a native one.

## Notes

- **The gate** counts any of Xwayland's surfaces with the keyboard as X having it: a managed X window, or an override-redirect menu holding a keyboard grab. A menu's grab moves the keyboard off its window, so a gate on X windows alone would refuse the menu's Paste item. The tests cover both selections, and a menu pasting while it holds the keyboard.
- **The lazy push** happens in `KeyboardFocus::enter`, just before Smithay's `X11Surface` enter. `X11Wm::new_selection` doesn't flush Caminito's X connection, but the enter's `SetInputFocus` does, so X has the selection before the client hears it has the focus. A menu's grab sends X nothing, so if a menu takes the keyboard before any of its app's windows ever had it, what X is owed waits for `X11Wm`'s next flush. In practice the window is clicked, and so focused, before its menu opens. A flush in `new_selection` belongs upstream.
- **Selections X is owed** live in `RunningXwayland`, and are lost with it; 42 re-offers them after a restart.
- **Drag-and-drop:** the pointer's focus is now `input::PointerFocus`, `Surface` or `X11`, so a drag over an XWayland window speaks XDND through Smithay's `DndFocus` for `X11Surface`. `TouchFocus` stays `WlSurface`, whose `DndFocus` only speaks `wl_data_device`: Caminito has no touch input yet, and it becomes `PointerFocus`'s twin when it does. Hit-testing already used `X11Surface::surface_under`, through `Window::surface_under`. There's no X11-tier DnD test; the ticket asks only for the nested check.
- **The nested check** ran winit on Xvfb driven through XTest, as 35's and 36's did. xterm isn't installed, so PyGObject GTK3 apps stood in for xterm and foot, one with `GDK_BACKEND=x11` and one on Wayland. Text copied in each pasted into the other, through both the clipboard and the primary selection. A `file://` URI dragged from the X11 app dropped into the Wayland one, and the other way round. [46](46-nested-x11-checks-left-over.md) repeats it with xterm, foot and caja by hand.
