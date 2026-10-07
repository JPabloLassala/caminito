# The Frame as one per-window module

Type: implementation
Status: resolved
Milestone: M3
Blocked by: 35
Map: [Caminito v0.1](../map.md)

## What to build

One derivation of a window's frame, and one owner of the frame extents in use ([ADR 0012](../../../docs/adr/0012-policy-asks-frame-extents-for-its-own-state.md)), before [39](39-x-window-properties-and-requests.md) publishes `_NET_FRAME_EXTENTS`. From the architecture review of 29 September, candidate 1.

- **`FrameState` in `caminito-policy`:** maximised, focused and shaded. `PolicyWindow::frame_extents(state)` replaces `frame_extents(maximized)`, and `requested_position(extents)` takes the extents from policy. `Managed` stops caching extents and asks its window with its own state.
- **`Policy::frame_extents(id)`:** the extents policy lays the window out with, zero for CSD and fullscreen windows. The debug dump and `XwmHandler::configure_request` read it; `Mapped::visible_extents` goes.
- **Focus changes relayout:** when a window gains or loses the activation, policy lays it out again before applying.
- **`WindowFrame` owns the derivation:** the SSD decision, `frame_style` from the window's facts plus a `FrameState`, and the extents for every state, rebuilt when the facts or the theme change. It draws with the `FrameState` built from the states and activation policy last sent. `Mapped` keeps no `ssd`, `extents` or `maximized_extents` of its own.
- **Pending windows** get their extents from the same derivation, focused, and unmaximised or maximised. `pending_extents`' own `frame_style` call goes.
- **One entry point:** `window_changed`, `decoration_changed` and `frames_changed` read the facts again through `Caminito::read_window_facts` before telling policy. `map_pending` reads them with `Mapped::read_facts`, the same call, before the window is in policy.
- Left for later: hit-testing still reads the last drawn geometry (review candidate 5), and `caminito-theme`'s drawing sequence is unchanged (candidate 2).

## Acceptance criteria

- [x] Unit tests, written before the code: with eOS, a maximised utility frame's extents differ between focused and unfocused, and a CSD or dock window has none.
- [x] Policy tests, written before the code: with a `TestWindow` whose extents depend on focus, a maximised window's frame fills the work area whether it's focused or not, before and after focus moves; a shaded window's frame is its shaded titlebar.
- [x] The proptest invariants hold with `TestWindow`s whose extents depend on focus and shade.
- [x] Existing fixture, X11-tier and golden tests pass, with only their frame-extents lookups moved to `Policy::frame_extents`.
