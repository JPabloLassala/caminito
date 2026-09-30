# Drag icons

Type: implementation
Status: open
Milestone: M3
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

Found by [46](46-nested-x11-checks-left-over.md): dragging a file from a native caja showed no icon under the pointer. Dragging from an X11 caja did show one, because an X client draws its drag icon in its own override-redirect window. Camino ignores the icon surface a Wayland client passes to `start_drag` (`handlers/selection.rs`: "Drag icons aren't drawn yet"). The same applies to icons Xwayland passes on for X-to-Wayland drags, if it uses the Wayland path.

- **Kept on the seat** from `dnd_requested` until the grab ends, dropped or cancelled, with the surface's role set as Smithay's `DnDGrab` expects.
- **Drawn** above everything but the pointer, at the pointer location plus the icon surface's offset, with offset changes from `wl_surface.offset` applied. It gets frame callbacks like any other surface, and damage when it moves.
- **Scale:** the pointer's output scale, like the cursor surface.

## Acceptance criteria

- [x] Fixture tests, written before the code: a test client's drag with an icon surface puts the icon at the pointer location plus its offset in the rendered scene, moves it with the pointer, and removes it on drop and on cancel.
- [ ] Nested, dragging a file from a native caja shows the file's icon under the pointer until the drop.

## Notes

- **X-to-Wayland drags** don't use the Wayland path: Smithay's XDND bridge (`xwayland/xwm/dnd.rs` at `928d4a9`) starts the `DnDGrab` itself and passes no icon, and the X client draws its own in an override-redirect window, as the X11 caja does. Only `dnd_requested` keeps an icon.
- **Kept** on `Camino::drag_icon` from `dnd_requested` until `dropped` or `cancelled`. Smithay gives the surface `DND_ICON_ROLE` before calling `dnd_requested`. The icon is kept after `set_grab`, because a drag that replaces another cancels the old one inside `set_grab`, and anvil's order would lose the new icon there. An offset committed before the drag began, as GTK commits it, is taken as the icon's first. Each later commit of the icon adds its `buffer_delta` to the offset, as anvil does (`anvil/src/shell/mod.rs:226-239`), and queues a redraw.
- **Drawn** in a new `SceneLayer::DragIcon`, under the cursor and over the lock, as the ticket says. Like the cursor, it's drawn only on the output the pointer is on, at that output's scale, and that output's frames send it frame callbacks, its preferred buffer scale and its fractional scale.
- **Tested** in `tests/drag_icons.rs`: the icon at the pointer plus its offset, following the pointer and a later `wl_surface.offset`, and gone on drop; a second drag from the same press cancelling the first, whose icon goes, and showing its own elsewhere; frame callbacks; the layer order; and 32 physical pixels for a 16-pixel icon on a scale-2 output.
- **The nested check** needs eyes, so it waits for a hand run.
