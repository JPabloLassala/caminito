# X window properties and requests

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M3
Blocked by: 35, 38, 44, 45
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The per-window half of Caminito's X connection ([11](11-xwayland-integration.md)): the properties it writes and reads on client windows, and the client messages Smithay discards.

- **`_NET_WM_DESKTOP` (11):** honoured before map (`0xFFFFFFFF` means sticky, recorded until M8), written at map and on every move between workspaces, and its client message moves the window. Transients follow their parent.
- **`_NET_FRAME_EXTENTS` (11):** written at map and whenever the frame geometry changes (theme reload, maximise, shade, fullscreen): the Marco theme's borders for SSD windows, zero for CSD, fullscreen and undecorated ones. `_NET_REQUEST_FRAME_EXTENTS` is answered before map from the theme and the window type.
- **Property changes:** changes Smithay doesn't parse arrive as `XwmHandler::property_notify(Other(atom))`; Caminito reads the new value over its own connection.
- **`_NET_WM_ICON` (07, 11):** read at map and on change, in 07's order after xdg-toplevel-icon and before the `.desktop` icon found through `WM_CLASS`, in the `icons` lookup. If Smithay #2160 has landed on the pinned rev, use it instead.
- **Struts:** `_NET_WM_STRUT` and `_NET_WM_STRUT_PARTIAL` read at map and on change, divided by the client scale, and kept per window. Policy's work area gains them with layer-shell exclusive zones in M5 (17).
- **Input shape (35):** the SHAPE extension's input region, read at map and on `ShapeNotify`, bounds where an X window takes the pointer. GTK's CSD windows on X11 leave their shadow out of it, but Xwayland doesn't pass it on to the `wl_surface`, so without it a click beneath a CSD shadow goes to the shadow's window.
- **States X is told (35):** a minimised window is hidden through `set_hidden` (`WM_STATE` Iconic and `_NET_WM_STATE_HIDDEN`), and shown again when it's restored. The above, below and sticky states policy records go back into `_NET_WM_STATE`.
- **Skip-taskbar and skip-pager** changes after map, kept in Caminito's window state and never written back to `_NET_WM_STATE`.
- **Client messages Smithay drops (11):**
  - `_NET_CLOSE_WINDOW`, through `close()`;
  - `_NET_MOVERESIZE_WINDOW`, through the `configure_request` path;
  - `_NET_WM_MOVERESIZE` directions 9 and 10 (keyboard move and resize, 05) and 11 (cancel);
  - `_GTK_SHOW_WINDOW_MENU`, logged until M8's window menu.
- **Debug dump:** X11 windows gain their frame extents as published and their struts.

## Acceptance criteria

- [x] X11-tier tests, written before the code: `_NET_REQUEST_FRAME_EXTENTS` is answered before map; `_NET_FRAME_EXTENTS` changes on maximise and on a theme switch, and is zero for a Motif-undecorated window; a window with `_NET_WM_DESKTOP` 2 before map maps on workspace 3; the `_NET_WM_DESKTOP` message moves a window; `_NET_CLOSE_WINDOW` closes; `_NET_MOVERESIZE_WINDOW` moves; a keyboard-move request starts a move and direction 11 cancels it; `_NET_WM_ICON` becomes the frame icon; struts are read and appear in the dump; a click on a window beneath another's input-shaped shadow reaches the window beneath.
- [x] Nested, `xprop` on an xterm shows `_NET_FRAME_EXTENTS` matching the Menta frame, and `wmctrl -r xterm -t 1` moves it. Passed with stand-ins, pluma and a script, since neither xterm nor wmctrl is installed; [46](46-nested-x11-checks-left-over.md) runs them.

## Notes

- **Where the facts live:** what Caminito's X connection reads of a window (the desktop it asked for before map, `_NET_WM_ICON`, the struts, the input region, and skips asked after map) is kept in the `X11Surface`'s user data as `XFacts`, which the X11 adapter reads. `Mapped` and policy stay protocol-free.
- **Before map:** `map_window_request` reads `_NET_WM_DESKTOP`, the icon, the struts and the input region, and selects `ShapeNotify`, before the window is pending. Policy gained the `PolicyWindow::requested_workspace` fact, read at map as `requested_position` is: a window maps on the desktop it asked for if that exists, and isn't focused if that isn't the active one. A transient goes with its parent whatever it asked. `0xFFFFFFFF` records sticky.
- **Publishing** follows the refresh-pass rule, as the root properties do: `xconn` keeps what it last wrote on each managed window, `_NET_WM_DESKTOP` (sticky as `0xFFFFFFFF`) and `_NET_FRAME_EXTENTS` (policy's `frame_extents`, so zero for CSD and fullscreen windows), and writes only what differs. A window that's no longer managed loses its `_NET_WM_DESKTOP`, as EWMH asks of a withdrawn one.
- **`_NET_REQUEST_FRAME_EXTENTS`** for a window that isn't managed is answered at once, from its type, `WM_TRANSIENT_FOR`, `_MOTIF_WM_HINTS` and the fullscreen and maximised states in its `_NET_WM_STATE`, read over Caminito's own connection, since the message can arrive before Smithay has read them. The answer is the focused frame it would map with. A managed window's are policy's, which the refresh pass writes again.
- **States X is told:** the refresh pass compares policy's minimised, above, below and sticky with Smithay's `_NET_WM_STATE` and calls `set_hidden`, `set_above`, `set_below` and `set_sticky` only where they differ.
- **Property changes:** `property_notify(Other(atom))` now reads the icon or the struts again if the atom is theirs, and is otherwise ignored, so Caminito's and Smithay's own writes no longer count as a window change.
- **Keyboard move and resize** (directions 9 and 10) start policy's move, or a resize from the bottom right corner, from where the pointer is, until the next click. There's no keyboard grab yet: the arrow keys, Return and Escape come with M8's `begin-move` and `begin-resize`. Direction 11 ends the window's move or resize where it is, as Marco's `meta_display_end_grab_op` does.
- **`_NET_MOVERESIZE_WINDOW`** shares `configure_request`'s path, with its gravity in place of `WM_NORMAL_HINTS`' when it names one. `_NET_CLOSE_WINDOW` goes through `Policy::close`, so docks and the desktop aren't closed, as with Alt+F4.
- **The input region** is read with SHAPE's `GetRectangles`. X reports one rectangle covering an unshaped window, which is taken as no shape, so the region follows the window's size. The hit test passes over an XWayland window where the point is outside it. Managed windows need it because their `wl_surface` is Smithay's frame window's, which has no shape. Xwayland gives an override-redirect window's input shape to its own surface only from 24.1 (xserver MR 1510), so since 2026-10-10 an override-redirect window's is read too, for Ubuntu 24.04's 23.2 and older. A test checks the latter.
- **Reading what Caminito's X connection wrote:** Xwayland orders requests per connection only, so a test client could read a property before Xwayland had handled Caminito's write of it. That made 38's `_NET_NUMBER_OF_DESKTOPS` test fail about once in twenty runs under load. The fixture's `x11_call` now syncs Caminito's X connection first.
- **`_NET_SUPPORTED`** gained the desktop, frame-extent, close, moveresize-window, icon and strut atoms. `_GTK_SHOW_WINDOW_MENU` is handled but not listed: GTK sends it only if it's listed, and shows its own menu otherwise, which is better until M8's window menu.
- **Messages x11rb queues** while Caminito waits for a reply leave the socket quiet, so the refresh pass reads them too.
- **Ownership:** `xconn` writes only `_NET_WM_DESKTOP` and `_NET_FRAME_EXTENTS` on windows, through the helpers that assert it isn't Smithay's; `_NET_WM_STATE` and `WM_STATE` go through Smithay's setters. A test watching a window's property changes can't tell whose they are, so the assertion is the guard, as 38 left it.
- **The nested check** ran winit on Xvfb. pluma's `_NET_FRAME_EXTENTS` was `1, 1, 31, 1`, Menta's normal frame, and a script sending what `wmctrl -r <window> -t 1` sends moved it to the second workspace, and back to the first. `_NET_SUPPORTED` listed the new atoms, and nothing was logged at `WARN`.
