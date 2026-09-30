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

- [ ] Fixture tests, written before the code: a test client's drag with an icon surface puts the icon at the pointer location plus its offset in the rendered scene, moves it with the pointer, and removes it on drop and on cancel.
- [ ] Nested, dragging a file from a native caja shows the file's icon under the pointer until the drop.
