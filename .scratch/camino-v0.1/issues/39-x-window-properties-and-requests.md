# X window properties and requests

Type: implementation
Status: open
Milestone: M3
Blocked by: 35, 38, 44
Map: [Camino v0.1](../map.md)

## What to build

The per-window half of Camino's X connection ([11](11-xwayland-integration.md)): the properties it writes and reads on client windows, and the client messages Smithay discards.

- **`_NET_WM_DESKTOP` (11):** honoured before map (`0xFFFFFFFF` means sticky, recorded until M8), written at map and on every move between workspaces, and its client message moves the window. Transients follow their parent.
- **`_NET_FRAME_EXTENTS` (11):** written at map and whenever the frame geometry changes (theme reload, maximise, shade, fullscreen): the Marco theme's borders for SSD windows, zero for CSD, fullscreen and undecorated ones. `_NET_REQUEST_FRAME_EXTENTS` is answered before map from the theme and the window type.
- **Property changes:** changes Smithay doesn't parse arrive as `XwmHandler::property_notify(Other(atom))`; Camino reads the new value over its own connection.
- **`_NET_WM_ICON` (07, 11):** read at map and on change, in 07's order after xdg-toplevel-icon and before the `.desktop` icon found through `WM_CLASS`, in the `icons` lookup. If Smithay #2160 has landed on the pinned rev, use it instead.
- **Struts:** `_NET_WM_STRUT` and `_NET_WM_STRUT_PARTIAL` read at map and on change, divided by the client scale, and kept per window. Policy's work area gains them with layer-shell exclusive zones in M5 (17).
- **Skip-taskbar and skip-pager** changes after map, kept in Camino's window state and never written back to `_NET_WM_STATE`.
- **Client messages Smithay drops (11):**
  - `_NET_CLOSE_WINDOW`, through `close()`;
  - `_NET_MOVERESIZE_WINDOW`, through the `configure_request` path;
  - `_NET_WM_MOVERESIZE` directions 9 and 10 (keyboard move and resize, 05) and 11 (cancel);
  - `_GTK_SHOW_WINDOW_MENU`, logged until M8's window menu.
- **Debug dump:** X11 windows gain their frame extents as published and their struts.

## Acceptance criteria

- [ ] X11-tier tests, written before the code: `_NET_REQUEST_FRAME_EXTENTS` is answered before map; `_NET_FRAME_EXTENTS` changes on maximise and on a theme switch, and is zero for a Motif-undecorated window; a window with `_NET_WM_DESKTOP` 2 before map maps on workspace 3; the `_NET_WM_DESKTOP` message moves a window; `_NET_CLOSE_WINDOW` closes; `_NET_MOVERESIZE_WINDOW` moves; a keyboard-move request starts a move and direction 11 cancels it; `_NET_WM_ICON` becomes the frame icon; struts are read and appear in the dump.
- [ ] Nested, `xprop` on an xterm shows `_NET_FRAME_EXTENTS` matching the Menta frame, and `wmctrl -r xterm -t 1` moves it.
