# A layer surface's popups are drawn above the windows

Type: implementation
Status: open
Milestone: M5 (proposed)
Blocked by: 63
Map: [Camino v0.1](../map.md)

## What to build

A popup of a layer surface on the background or bottom layer is drawn above the windows, as sway and niri draw it. Today [63](63-layer-shell.md) draws a popup in its surface's layer, so the menu of caja's desktop, which is on the bottom layer, is cut off wherever a window overlaps it. [76](76-tty-login-left-over.md) saw that in the session on 2026-10-05, and the developer decided the same day that the menu goes above the windows.

- **Drawing:** the popups of background and bottom layer surfaces are drawn above the windows of the workspace, and below the top layer's surfaces, so a panel still covers them. The popups of top and overlay surfaces stay where they are.
- **Input follows the picture:** a press on the part of the menu that's over a window goes to the menu, not to the window under it.
- **Fullscreen:** what a fullscreen window does to such a popup is to decide here. The desktop isn't reachable under one, so it may not come up.

## Acceptance criteria

- [ ] A headless test, written before the code: a popup of a bottom-layer surface that overlaps a window is above it in the render order, and a press on the overlap goes to the popup.
- [ ] A popup of a top-layer surface is drawn as before.
- [ ] In the session, caja's desktop menu opened beside a window is whole, and its items under the pointer answer a click where they're over the window.

## Notes

- 63's notes have what it decided and why; its line on this question can point here.
- Whether this is M5's or later needs the developer. It doesn't block [73](73-packaging-and-closing-m5.md) as filed.
