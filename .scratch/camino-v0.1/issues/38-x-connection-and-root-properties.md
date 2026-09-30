# Camino's X connection and root properties

Type: implementation
Status: resolved
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

- [x] X11-tier tests, written before the code: `_NET_SUPPORTED` is Camino's superset and the WM name is "Camino"; the desktop atoms follow `num-workspaces`, `workspace-names` and a workspace switch; a `_NET_CURRENT_DESKTOP` client message switches workspaces; a `_NET_NUMBER_OF_DESKTOPS` message changes `num-workspaces`; `_NET_WORKAREA` and `_GTK_WORKAREAS_D0` are right for two headless outputs.
- [x] A test that `DISPLAY` isn't set in Camino's environment until the root properties exist.
- [x] A test that `xconn` never writes any of `_NET_CLIENT_LIST`, `_NET_CLIENT_LIST_STACKING`, `_NET_ACTIVE_WINDOW`, `_NET_WM_STATE` or `WM_STATE`.
- [x] Nested, `xprop -root` shows the desktop atoms and work area, and `wmctrl -d` lists the workspaces. The `xprop` half passed; `wmctrl -d` moved to [46](46-nested-x11-checks-left-over.md), since wmctrl isn't installed.

## Notes

- **The connection** is made in `xwayland_ready`, right after `start_wm`. Smithay's root writes are done by then, since `start_wm` waits for replies after them, so `_NET_SUPPORTED` is read back and extended rather than rebuilt. The superset adds `_NET_SUPPORTING_WM_CHECK` and `_NET_WM_NAME`, which Smithay sets without listing, the three desktop atoms, `_NET_WORKAREA` and `_GTK_WORKAREAS`, which GTK3 needs listed before it reads `_GTK_WORKAREAS_D<n>`. [39](39-x-window-properties-and-requests.md) adds its atoms to the same list. If the connection fails, Camino logs it once and runs X without it.
- **Publishing** follows the refresh-pass rule: `xconn` keeps what it last wrote, and each refresh pass writes only what differs from policy. `xwayland_ready` runs the same publish once before it sets `DISPLAY`. When the workspace count drops, the `_GTK_WORKAREAS_D<n>` of the removed desktops are deleted.
- **`_NET_WORKAREA`** is Marco's screen-wide work area, the X screen from the origin to the far edges of every output, less the struts, of which there are none yet. `_GTK_WORKAREAS_D<n>` lists each output's work area, in policy's output order. GTK3 keeps the last rectangle that meets a monitor, and while each is a whole output none overlap, so the order doesn't matter yet; M5's struts may make it matter. Policy gained `work_areas()`, which M5's struts and exclusive zones change.
- **Root requests:** `_NET_CURRENT_DESKTOP` runs the new `Action::SwitchToWorkspace(n)`, which M8's `switch-to-workspace-N` bindings will share, and is ignored for a desktop that doesn't exist, as Marco does. `_NET_NUMBER_OF_DESKTOPS` writes `num-workspaces`, ignoring 0 as Marco does and clamping to the schema's 36, which GSettings would otherwise refuse.
- **Ownership:** every write and delete goes through one of three helpers that `debug_assert!` the property isn't Smithay's. The X11-tier test watches the root's property changes through every publishing path and finds only `xconn`'s. `xconn` writes no window's properties yet, so for `_NET_WM_STATE` and `WM_STATE` only the assertion stands guard; [39](39-x-window-properties-and-requests.md), which writes per-window properties, should watch its windows the same way.
- **The `DISPLAY` test** reads the desktop atoms, the work areas, `_NET_SUPPORTED` and the check window's name. It runs Camino's sources without the refresh pass until `DISPLAY` appears, then reads the root from a thread without dispatching Camino, so a publish that waited for the refresh pass fails it. That mutation was checked.
- **The nested check** ran winit on Xvfb. `xprop -root` showed the three desktop atoms, `_NET_WORKAREA`, `_GTK_WORKAREAS_D0` to `D3` and the superset, and the check window's `_NET_WM_NAME` was "Camino". wmctrl isn't installed, so a script listed the desktops from the same properties as `wmctrl -d` does; [46](46-nested-x11-checks-left-over.md) runs `wmctrl -d` itself. A GDK beep from an X11 client logged `an X client rang the bell at 50%`.
