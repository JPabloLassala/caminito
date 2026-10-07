# xdg-shell and input

Type: implementation
Status: resolved
Milestone: M1
Blocked by: 19, 21
Map: [Caminito v0.1](../map.md)

## What to build

Enough protocol and input to meet M1's exit ([17](17-milestone-slicing.md)): real apps map, take focus, stack, move, resize, maximise and switch workspaces. It's written test-first against [21](21-test-fixture-and-client.md)'s fixture.

- **Protocols (06):** Smithay's core set, except data-control (which comes in M5) and drm-syncobj (M4):
  - xdg-shell, xdg-output, viewporter, fractional-scale, xdg-activation, primary-selection;
  - pointer-constraints, relative-pointer, pointer-gestures, tablet-v2, keyboard-shortcuts-inhibit, text-input-v3, input-method-v2, cursor-shape;
  - xdg-foreign, xdg-dialog, xdg-toplevel-icon, xdg-system-bell;
  - presentation-time, single-pixel-buffer, content-type, alpha-modifier, fifo, commit-timing.
- **Decorations until M2:** xdg-decoration and KDE server-decoration answer *client side*. This is a stand-in (17).
- **Handlers:** `handlers/` files wire each protocol into `State`, and xdg toplevels reach policy as `Mapped` through map and unmap events. Popups are positioned with Smithay's `PopupManager` and are constrained to the output.
- **Input:** the keyboard path is one function with 15's five stages in order. Only `input::bindings` and the client stage do work; the lock, AccessX and keyboard-monitor stages pass everything through. Pointer focus follows the surface under the pointer. A click focuses and raises through policy, and Alt+drag moves and Alt+middle-drag resizes, as in Marco.
- **Bindings:** a hard-coded set of Marco's defaults, a stand-in until M8 (17):
  - Ctrl+Alt+Left/Right to switch workspace;
  - Alt+F4 to close;
  - Alt+F10 to maximise;
  - Alt+Tab without the popup.
- **Headless snapshots:** configure sequences for map, maximise, fullscreen and resize; focus changes on click; stacking after raise; and workspace switches unmapping and remapping.

## Acceptance criteria

- [x] Every behaviour above has a fixture test, written before the handler it covers.
- [x] Nested (`cargo run -p caminito`, then clients on its socket): gtk3-demo, gtk4-demo, a Qt app and foot map, take focus by click, stack, move, resize, maximise and switch workspaces.
- [x] Popups and menus in gtk3-demo open in the right place.

## Notes

- gtk3-demo's entry completion list doesn't flip up at the bottom of an output. GTK3 maps it as a subsurface of the entry's window, not an `xdg_popup`, so only the client places it, and GTK3 doesn't know where its window is on the output.
- Reopening a GTK3 submenu with radio items briefly shows the old selection. Each reopen is a new surface whose first buffer GTK3 draws stale, then corrects about 10 ms later.
