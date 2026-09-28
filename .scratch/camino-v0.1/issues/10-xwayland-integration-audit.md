# XWayland integration audit

Type: research
Status: open
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

How much of Marco's X11 behaviour can Camino give XWayland windows, using Smithay master's X11 window manager (`xwayland::xwm`) plus Camino's own X connection (06)? Starting from Smithay master, niri's and cosmic-comp's XWayland code, and Marco at 41c50d5, cover the following. Output: a table of each X11 behaviour Camino needs, marked handled by Smithay, needs Camino's X connection, or not feasible, with source citations.

- **Window-manager coverage:** which ICCCM and EWMH requests and properties Smithay's window manager handles, and what's left: `_NET_WM_STATE` (above, below, sticky, demands-attention, skip-taskbar), `_NET_WM_DESKTOP`, `_NET_WM_STRUT(_PARTIAL)`, `_NET_WM_PID`, `_NET_WM_PING`, `_NET_WM_USER_TIME`, `_NET_WM_SYNC_REQUEST`, `WM_TAKE_FOCUS`, transient-for and window groups, `_MOTIF_WM_HINTS`, `_GTK_FRAME_EXTENTS`, and `_NET_WM_MOVERESIZE`.
- **Root properties Camino must publish:** `_NET_NUMBER_OF_DESKTOPS`, `_NET_CURRENT_DESKTOP`, `_NET_DESKTOP_NAMES`, `_NET_WORKAREA`, `_NET_FRAME_EXTENTS`, `_NET_ACTIVE_WINDOW` and `_NET_CLIENT_LIST`. Who reads them under Camino? mate-session and the mcc capplets run on XWayland; other X11 apps and toolkits may too.
- **One X connection or two:** can a second, Camino-owned X connection coexist with Smithay's window-manager connection, or should Camino's X work (the XKB bell, struts, the root properties) go through Smithay's connection? What happens to each after an Xwayland restart (08)?
- **Clipboard, primary selection and drag-and-drop** between X11 and Wayland clients in Smithay master: what works and which bugs are known.
- **Scaling:** how XWayland windows behave under a fractional output scale (`xwayland_shell`, and Xwayland's `-hidpi` / `-force-xrandr-emulation` handling in niri and cosmic).
- **Focus and stacking** of override-redirect windows (menus and tooltips), and how they interact with Camino's scene graph.
