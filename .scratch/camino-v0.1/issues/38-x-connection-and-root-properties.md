# Camino's X connection and root properties

Type: implementation
Status: open
Milestone: M3
Blocked by: 34
Map: [Camino v0.1](../map.md)

## What to build

`xwayland::xconn` ([15](15-crate-and-module-architecture.md)): Camino's own X connection, and the root properties Smithay doesn't publish ([11](11-xwayland-integration.md)). Per-window properties are [39](39-x-window-properties-and-requests.md)'s.

- **The connection (11, 15):** an ordinary x11rb client that connects to `:n` after `Ready` and `start_wm`, as a calloop fd source on the main thread. It selects `SubstructureNotify | PropertyChange` on the root and XKB `BellNotify`. It never selects `SubstructureRedirect`, never grabs the server, and never writes a property Smithay maintains (11's ownership rule, as a comment and a debug assertion on every write).
- **Written once per `start_wm`:** `_NET_SUPPORTED` replaced by a superset listing everything Camino handles, and the check window's `_NET_WM_NAME` set to "Camino".
- **Desktops (11):** `_NET_NUMBER_OF_DESKTOPS`, `_NET_DESKTOP_NAMES` and `_NET_CURRENT_DESKTOP`, written at start and on every change, from policy's workspace count, names and active workspace. The root requests for the current desktop and the count run the same code as Camino's bindings; a count request writes `num-workspaces`, as the pager does (06).
- **Work area (11):** `_NET_WORKAREA` (one rectangle per desktop) and `_GTK_WORKAREAS_D<n>` (per desktop, one per monitor), built as Marco does (`screen.c:2354-2435`), rewritten when outputs, the workspace count or policy's work area change. In M3 policy's work area is the outputs; struts and exclusive zones join it in M5 (17).
- **Bell:** `BellNotify` is received and logged at debug level. M6 joins it to the single bell event (06).
- **Start-up order (11):** `Ready` → `start_wm` → connect → `_NET_SUPPORTED` and the name → root properties → only then export `DISPLAY`. The XSETTINGS and scale steps slot in when [40](40-xsettings.md) and [41](41-x11-scale.md) land.
- **Coordinates:** in X client space. Until [41](41-x11-scale.md), the client scale is 1.

## Acceptance criteria

- [ ] X11-tier tests, written before the code: `_NET_SUPPORTED` is Camino's superset and the WM name is "Camino"; the desktop atoms follow `num-workspaces`, `workspace-names` and a workspace switch; a `_NET_CURRENT_DESKTOP` client message switches workspaces; a `_NET_NUMBER_OF_DESKTOPS` message changes `num-workspaces`; `_NET_WORKAREA` and `_GTK_WORKAREAS_D0` are right for two headless outputs.
- [ ] A test that `DISPLAY` isn't set in Camino's environment until the root properties exist.
- [ ] A test that `xconn` never writes any of `_NET_CLIENT_LIST`, `_NET_CLIENT_LIST_STACKING`, `_NET_ACTIVE_WINDOW`, `_NET_WM_STATE` or `WM_STATE`.
- [ ] Nested, `xprop -root` shows the desktop atoms and work area, and `wmctrl -d` lists the workspaces.
