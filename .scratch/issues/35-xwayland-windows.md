# XWayland windows

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M3
Blocked by: 34
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

XWayland windows managed like native ones ([11](11-xwayland-integration.md)): `xwayland::xwm` implements `XwmHandler` for `State`, and maps each X11 window into `Mapped` and policy. Override-redirect windows are [36](36-override-redirect-windows.md)'s.

- **`Mapped` for X11 (15):** `Mapped` already wraps `desktop::Window`, which covers `X11Surface`. The protocol differences sit behind a private trait, implemented on `ToplevelSurface` (`window/xdg.rs`) and `X11Surface` (`window/x11.rs`), which `Mapped` reaches through its one `match` on `underlying_surface()`. No Caminito window type is added (ADR 0010). The first commit moves the xdg reads behind the trait with no change in behaviour, and the X11 adapter follows.
  - **Facts:** title, app id, window type, size hints, requested position, pid (`_NET_WM_PID` for X11, never Xwayland's credentials), whether the window wants a frame, icon, the parent's `WindowKey`, urgency, input model, and taskbar and pager skipping. Both adapters answer every fact, xdg with defaults where it has none.
  - **Requests:** `set_geometry` plus `flush`, so each protocol sends once per loop in the refresh pass. Also activate, close through `WM_DELETE_WINDOW`, `keyboard_target`, and `configure_pending` for a pending window's pre-map configure. xdg's `configure_owed` moves into its adapter.
  - `Mapped` keeps the id, outputs, fractional scale, the states policy last asked for, and the frame. The frame code only asks the adapter whether the window wants a frame.
- **Lifecycle:** `window/lifecycle.rs` owns the pending windows (CONTEXT.md; `Unmapped` becomes `Pending`), privately. Its events are created, pending state request, committed content, withdrawn and destroyed. The xdg and X11 handlers only translate their protocol's events into them.
  - A window of either protocol maps into policy on its first commit with content. For X11, `map_window_request` calls `set_mapped(true)` and the window stays pending until its `wl_surface` is associated and commits.
  - The initial `_NET_WM_STATE` seeds an X11 pending window's states, and later requests update them as xdg's do. A pending window's `configure_request` is honoured in full.
  - An unmap takes the window out of policy, and a remap gets a new `WindowId`, as for xdg.
  - One private `WindowKey { Wayland(WlSurface), X11(X11Window) }` finds windows for commits, X callbacks, transient parents and destroy. X11 keys compare by `window_id()`, because a dead `X11Surface` never compares equal.
  - xdg commits and X11 `property_notify` both call one `Caminito::window_changed(id)`, which refreshes the frame facts and then tells policy.
- **Facts for `PolicyWindow`:**
  - the window type from `WmWindowType` mapped to Marco's types, with Marco's rules for frames, layers, taskbar and pager skipping, and which types take focus (`recalc_window_features`, `meta_window_update_layer`);
  - `WM_TRANSIENT_FOR` through `window_id()`, and `_NET_WM_STATE_MODAL` for the modal style set;
  - size hints from `WM_NORMAL_HINTS`, `_NET_WM_PID`, `WM_CLASS` as the app id;
  - `requested_position` from `USPosition` and `PPosition` at map.
- **Requests (11):** a `configure_request` is honoured for windows that aren't maximised, fullscreen or tiled, clamped to the work area as Marco does. Maximise, fullscreen, minimise, and mouse move and resize (`_NET_WM_MOVERESIZE` directions 0–8) go through policy. Above, below and sticky requests reach policy and are recorded; they take effect in M8 (17).
- **Frames (07):** XWayland windows get Marco frames by the same rules as native ones, with `_MOTIF_WM_HINTS` turning decorations off. The title comes from `_NET_WM_NAME` or `WM_NAME`, live. Frame interaction ([31](31-frame-interaction.md)) works unchanged.
- **Stacking (11):** Caminito's scene is the truth. After every restack that involves an XWayland window, Caminito calls `update_stacking_order_upwards` with the XWayland windows in scene order.
- **Focus (11):** `focus-mode`, `raise-on-click` and `auto-raise` apply unchanged, through Smithay's `KeyboardTarget`, which handles the ICCCM input models and `WM_TAKE_FOCUS`. `SeatHandler::KeyboardFocus` becomes `enum { Surface(WlSurface), X11(X11Surface) }`, and a mapped window gives its target through the adapter. `PointerFocus` stays `WlSurface` until [37](37-clipboard-and-dnd.md). A new XWayland window takes focus as a native one does now; 11's X11 rules for *smart* and *strict* come with focus-stealing prevention in M8. `XUrgencyHint` and demands-attention mark the window urgent in `Mapped`, for M5's foreign-toplevel and M8's Alt+Tab.
- **Debug dump ([32](32-debug-interface.md)):** windows gain `xwayland` and, for X11 windows, the X window id and the Marco type.

## Acceptance criteria

- [x] Unit tests of the `WmWindowType`-to-Marco-type mapping and each type's features, written before the code.
- [x] X11-tier tests, written before the code: a normal window maps framed, takes focus on click and moves; a dialog with `WM_TRANSIENT_FOR` stacks above its parent on the parent's workspace; a `USPosition` window is placed where it asked; a `configure_request` for a maximised window is refused; a window with Motif decorations off gets no frame; `_NET_CLIENT_LIST_STACKING` follows Caminito's order after a raise.
- [x] X11-tier lifecycle tests, written before the code: a window that asks to be maximised before it maps maps maximised; an unmap and remap gives a new policy id; a window destroyed while pending leaves no trace; focusing a window moves X input focus to it.
- [x] The fixture looks up a test's window by its policy id whichever protocol it uses, and frame interaction tests run against an XWayland window too.
- [x] The proptest invariants still hold with `TestWindow`s of every type (policy is shared, so no X11-specific operations are needed).
- [x] Nested, xterm, xeyes and gtk3-demo with `GDK_BACKEND=x11` map with Menta frames, take focus and move.

## Notes

- Policy has no tiled state until M8's edge and keyboard tiling (17), so a `configure_request` is refused while the window is maximised either way, fullscreen or grabbed. The tiled case joins `request_geometry` with tiling.
- `_NET_WM_ICON` is [39](39-x-window-properties-and-requests.md)'s. Until then an X window's icon comes from `WM_CLASS`, as a native window's comes from its app id.
- The nested check ran winit on Xvfb, with XTest standing in for the hand. xterm and xeyes weren't installed, so glxgears stood in for a plain Xlib client and pluma for a GTK3 SSD app. glxgears (placed at its `USPosition` 0, 0) and pluma mapped with Menta frames, took focus on a click, moved by exactly the drag of their titlebars, and took typing. `_NET_ACTIVE_WINDOW` and `_NET_CLIENT_LIST_STACKING` followed. gtk3-demo's main window has a header bar, so it stays CSD (07); it took focus, and its header bar moved it through `_NET_WM_MOVERESIZE`. Its Spinner demo mapped as a modal dialog with Menta's modal frame, centred on its parent.
- The check found a gap: Caminito hit-tests an X window by its whole surface, but GTK's CSD windows on X11 set an input shape that leaves out their shadow (gtk3-demo's is inset 35 px into a 45 px `_GTK_FRAME_EXTENTS` shadow). Xwayland doesn't pass the X input shape on to the `wl_surface`, and Smithay doesn't read it, so a click on a window beneath the shadow goes to the CSD window. Reading the shape takes Caminito's own X connection, so it's [39](39-x-window-properties-and-requests.md)'s.
