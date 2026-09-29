# XWayland windows

Type: implementation
Status: open
Milestone: M3
Blocked by: 34
Map: [Camino v0.1](../map.md)

## What to build

XWayland windows managed like native ones ([11](11-xwayland-integration.md)): `xwayland::xwm` implements `XwmHandler` for `State`, and maps each X11 window into `Mapped` and policy. Override-redirect windows are [36](36-override-redirect-windows.md)'s.

- **`Mapped` for X11 (15):** `Mapped` already wraps `desktop::Window`, which covers `X11Surface`. The differences are `match`es on `underlying_surface()` inside `Mapped`: `X11Surface::configure`, `close` through `WM_DELETE_WINDOW`, and the facts below.
- **Facts for `PolicyWindow`:**
  - the window type from `WmWindowType` mapped to Marco's types, with Marco's rules for frames, layers, taskbar and pager skipping, and which types take focus (`recalc_window_features`, `meta_window_update_layer`);
  - `WM_TRANSIENT_FOR` through `window_id()`, and `_NET_WM_STATE_MODAL` for the modal style set;
  - size hints from `WM_NORMAL_HINTS`, `_NET_WM_PID`, `WM_CLASS` as the app id;
  - `requested_position` from `USPosition` and `PPosition` at map.
- **Requests (11):** a `configure_request` is honoured for windows that aren't maximised, fullscreen or tiled, clamped to the work area as Marco does. Maximise, fullscreen, minimise, and mouse move and resize (`_NET_WM_MOVERESIZE` directions 0–8) go through policy. Above, below and sticky requests reach policy and are recorded; they take effect in M8 (17).
- **Frames (07):** XWayland windows get Marco frames by the same rules as native ones, with `_MOTIF_WM_HINTS` turning decorations off. The title comes from `_NET_WM_NAME` or `WM_NAME`, live. Frame interaction ([31](31-frame-interaction.md)) works unchanged.
- **Stacking (11):** Camino's scene is the truth. After every restack that involves an XWayland window, Camino calls `update_stacking_order_upwards` with the XWayland windows in scene order.
- **Focus (11):** `focus-mode`, `raise-on-click` and `auto-raise` apply unchanged, through Smithay's `KeyboardTarget`, which handles the ICCCM input models and `WM_TAKE_FOCUS`. A new XWayland window takes focus as a native one does now; 11's X11 rules for *smart* and *strict* come with focus-stealing prevention in M8. `XUrgencyHint` and demands-attention mark the window urgent in `Mapped`, for M5's foreign-toplevel and M8's Alt+Tab.
- **Debug dump ([32](32-debug-interface.md)):** windows gain `xwayland` and, for X11 windows, the X window id and the Marco type.

## Acceptance criteria

- [ ] Unit tests of the `WmWindowType`-to-Marco-type mapping and each type's features, written before the code.
- [ ] X11-tier tests, written before the code: a normal window maps framed, takes focus on click and moves; a dialog with `WM_TRANSIENT_FOR` stacks above its parent on the parent's workspace; a `USPosition` window is placed where it asked; a `configure_request` for a maximised window is refused; a window with Motif decorations off gets no frame; `_NET_CLIENT_LIST_STACKING` follows Camino's order after a raise.
- [ ] The proptest invariants still hold with `TestWindow`s of every type (policy is shared, so no X11-specific operations are needed).
- [ ] Nested, xterm, xeyes and gtk3-demo with `GDK_BACKEND=x11` map with Menta frames, take focus and move.
