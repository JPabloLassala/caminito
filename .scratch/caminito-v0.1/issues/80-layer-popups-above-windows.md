# A layer surface's popups are drawn above the windows

Type: implementation
Status: resolved
Milestone: M5 (proposed)
Blocked by: 63
Map: [Caminito v0.1](../map.md)

## What to build

A popup of a layer surface on the background or bottom layer is drawn above the windows, as sway and niri draw it. Today [63](63-layer-shell.md) draws a popup in its surface's layer, so the menu of caja's desktop, which is on the bottom layer, is cut off wherever a window overlaps it. [76](76-tty-login-left-over.md) saw that in the session on 2026-10-05, and the developer decided the same day that the menu goes above the windows.

- **Drawing:** the popups of background and bottom layer surfaces are drawn above the windows of the workspace, and below the top layer's surfaces, so a panel still covers them. The popups of top and overlay surfaces stay where they are.
- **Input follows the picture:** a press on the part of the menu that's over a window goes to the menu, not to the window under it.
- **Fullscreen:** what a fullscreen window does to such a popup is to decide here. The desktop isn't reachable under one, so it may not come up.

## Acceptance criteria

- [x] A headless test, written before the code: a popup of a bottom-layer surface that overlaps a window is above it in the render order, and a press on the overlap goes to the popup.
- [x] A popup of a top-layer surface is drawn as before.
- [ ] In the session, caja's desktop menu opened beside a window is whole, and its items under the pointer answer a click where they're over the window.

## Notes

- 63's notes have what it decided and why; its line on this question can point here.
- Whether this is M5's or later needs the developer. It doesn't block [73](73-packaging-and-closing-m5.md) as filed.

## Resolution

The scene has one more layer, `LayerPopups`, between the top layer and the windows. It holds the popups of the bottom and background layers' surfaces, the bottom layer's in front. The bottom and background layers hold their surfaces without the popups. `scene::layer_surface_part` says which part of a layer surface each scene layer has, and both the drawing and the pointer's focus read it, so the input follows the picture. The tests are in `caminito/src/tests/layer_shell.rs`.

- **Fullscreen:** a fullscreen window that has the focus covers these popups, as it covers the top layer, since they're below it in the scene. A menu that's open when the window goes fullscreen stays open under it, and one with a grab keeps the grab, and the keyboard with it, until a press ends it. A fullscreen window without the focus is among the other windows, so the popups are above it.
- **The top and overlay layers** are as 63 left them: each surface under its own popups, in its layer.
- **The keyboard** is as before. A popup with a grab, as caja's menu has, holds the keyboard through the grab, whose root is the layer surface. A press on a popup without one is a press on its layer surface, so an `on_demand` one takes the keyboard; no test has that.
- **Not tested:** a popup of a background surface, which goes the same way as a bottom one's, and a popup of an overlay surface.

The third criterion, caja's desktop menu in the session, needs eyes and hands, and is the developer's.
