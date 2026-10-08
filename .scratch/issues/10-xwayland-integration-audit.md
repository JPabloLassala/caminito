# XWayland integration audit

Epic: caminito-v0.1
Type: research
Status: resolved
Blocked by:
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Question

How much of Marco's X11 behaviour can Caminito give XWayland windows, using Smithay master's X11 window manager (`xwayland::xwm`) plus Caminito's own X connection (06)? Starting from Smithay master, niri's and cosmic-comp's XWayland code, and Marco at 41c50d5, cover the following. Output: a table of each X11 behaviour Caminito needs, marked handled by Smithay, needs Caminito's X connection, or not feasible, with source citations.

- **Window-manager coverage:** which ICCCM and EWMH requests and properties Smithay's window manager handles, and what's left: `_NET_WM_STATE` (above, below, sticky, demands-attention, skip-taskbar), `_NET_WM_DESKTOP`, `_NET_WM_STRUT(_PARTIAL)`, `_NET_WM_PID`, `_NET_WM_PING`, `_NET_WM_USER_TIME`, `_NET_WM_SYNC_REQUEST`, `WM_TAKE_FOCUS`, transient-for and window groups, `_MOTIF_WM_HINTS`, `_GTK_FRAME_EXTENTS`, and `_NET_WM_MOVERESIZE`.
- **Root properties Caminito must publish:** `_NET_NUMBER_OF_DESKTOPS`, `_NET_CURRENT_DESKTOP`, `_NET_DESKTOP_NAMES`, `_NET_WORKAREA`, `_NET_FRAME_EXTENTS`, `_NET_ACTIVE_WINDOW` and `_NET_CLIENT_LIST`. Who reads them under Caminito? mate-session and the mcc capplets run on XWayland; other X11 apps and toolkits may too.
- **One X connection or two:** can a second, Caminito-owned X connection coexist with Smithay's window-manager connection, or should Caminito's X work (the XKB bell, struts, the root properties) go through Smithay's connection? What happens to each after an Xwayland restart (08)?
- **Clipboard, primary selection and drag-and-drop** between X11 and Wayland clients in Smithay master: what works and which bugs are known.
- **Scaling:** how XWayland windows behave under a fractional output scale (`xwayland_shell`, and Xwayland's `-hidpi` / `-force-xrandr-emulation` handling in niri and cosmic).
- **Focus and stacking** of override-redirect windows (menus and tooltips), and how they interact with Caminito's scene graph.

## Answer

Full findings are on branch `research/xwayland-integration-audit` (commit 713f172), in `research/xwayland-integration-audit.md`. It has the Marco → Caminito behaviour table, with every row marked Smithay, Caminito X connection or Not feasible and cited at Smithay `928d4a9`, cosmic-comp `9f746ea`, niri `1f03391` and Marco `41c50d5`.

- **Smithay covers the per-window protocol:** mapping, titles and classes, size hints, `WM_HINTS`, `WM_TAKE_FOCUS`, transient-for, Motif hints, window types, `_NET_WM_PID`, `_NET_WM_PING`, `_NET_WM_SYNC_REQUEST`, `_GTK_FRAME_EXTENTS` and mouse `_NET_WM_MOVERESIZE`. It forwards the `_NET_WM_STATE` requests (maximise, fullscreen, modal, above, below, sticky, shaded, demands-attention) and maintains `_NET_CLIENT_LIST(_STACKING)`, `_NET_ACTIVE_WINDOW` and `_NET_SHOWING_DESKTOP`. All policy is Caminito's.
- **Smithay lacks:** `_NET_WM_DESKTOP` and the desktop count, current desktop and names; `_NET_WORKAREA` and `_GTK_WORKAREAS_D<n>`; `_NET_FRAME_EXTENTS` and its request; `_NET_CLOSE_WINDOW`, `_NET_MOVERESIZE_WINDOW`, `_NET_WM_USER_TIME`, struts, `_NET_WM_ICON`, `_NET_WM_ALLOWED_ACTIONS` and `_GTK_SHOW_WINDOW_MENU`. It also lacks keyboard move/resize and moveresize cancel, and it drops skip-taskbar and skip-pager changes after map. Unknown client messages are discarded. Upstream work-area issue #1181 has been open since 2023.
- **Recommended: two X connections.** `X11Wm` exposes no connection, root or atoms, and it discards the XKB bell and unknown client messages. A second, Caminito-owned client can write the missing properties and receive the root client messages. The rule is that Caminito never writes a property Smithay maintains (`_NET_CLIENT_LIST*`, `_NET_ACTIVE_WINDOW`, `_NET_SHOWING_DESKTOP`, `_NET_WM_STATE`, `WM_STATE`).
- **`_NET_SUPPORTED` must be replaced with a superset** after `start_wm` and before `DISPLAY` is exported, because GTK3 caches it. Caminito also renames the WM check window from "Smithay X WM" to "Caminito", because mcc's `wm-common` reads that name on X11.
- **XSETTINGS conflict:** Smithay claims `_XSETTINGS_S0` on every `start_wm`, so msd's xsettings plugin exits ("only one xsettings manager"). Recommended: Caminito serves XSETTINGS through `X11Wm::set_xsettings`, from msd's key table.
- **An Xwayland crash ends the MATE on Caminito session.** mate-session is an X11 GTK client, so GTK's X IO error handler kills it, and by 08 that ends the session. Same-display restart still matters for bare `caminito`: on `XwmHandler::disconnected`, drop the `XWayland` source, respawn on display n, purge X11 windows by hand (dropping `X11Wm` doesn't call `destroyed_window`), and rebuild both connections and all published state. Smithay has no event for Xwayland exiting after start-up (#1410).
- **Clipboard, primary and DnD** are bridged both ways, including INCR, with DnD since #1848. Caminito must implement `allow_selection_access`, which defaults to false; the recommendation is cosmic's gate on X11 focus. Gaps: the XDND Ask and Private actions, `CLIPBOARD_MANAGER` storing, and open DnD accept bug #1953.
- **Scaling is one global number for all X11 clients.** `-hidpi` is rootful-only, and `-force-xrandr-emulation` only adds fake modes. cosmic uses Smithay's per-client scale plus the XSETTINGS DPI and scale. niri's xwayland-satellite uses the smallest output scale plus viewporter. Work area and struts must use the same scale.
- **Override-redirect windows (menus, tooltips):** recommended is cosmic's approach. They get their own scene layer above toplevels and the top layer, but below overlay and the lock, ordered by Smithay's `above` sibling and never focused on click. `zwp_xwayland_keyboard_grab_v1` is honoured, but never over the lock (#1714).

### Handed on

- **[11](11-xwayland-integration.md):** confirm two connections and the property-ownership rule; the XSETTINGS owner; accepting that an Xwayland crash logs the user out; which Caminito-connection rows are in v0.1; the clipboard focus policy; the override-redirect layer; optional upstream Smithay PRs; passing `XCURSOR_*` through `envs`, because `XWayland::spawn` clears the environment.
- **[13](13-hidpi-and-accessibility-scope.md):** the X11 scale policy, its interaction with MATE's `window-scaling-factor` and the XSETTINGS scale settings, and mixed-DPI limits. (Resolved: the largest integer output scale, with msd's DPI formulas.)
