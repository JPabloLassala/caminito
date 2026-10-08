# A fixture that hands out window handles

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M3
Blocked by: 44
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

One way for a test to map a window of either kind and ask about it, before tickets 36–42 add their X11-tier tests. From the architecture review of 29 September, candidate 4. Test-only: no behaviour changes.

- **`ClientWindow` is the handle:** a `Copy` id, `Xdg(ClientId, WindowId)` or `X11(ClientId, u32)`. The fixture holds X clients by index as it does Wayland ones, so `x11_call` takes a `ClientId`, and `disconnect(id)` closes an X client. frames.rs's `Framed` goes.
- **`Fixture::connected(kind) -> (Fixture, ClientId)`** replaces `connected()` and `with_x11()`, and `f.connect(kind)` adds a client to a fixture, starting Xwayland first if it isn't running.
- **`f.map(client, WindowSpec) -> ClientWindow`:** one spec for both kinds. It defaults to 400 × 300 and asking for a frame; `WindowSpec::plain(size)` says nothing about decoration, as `map_window` did. `Decoration` is `Server`, `Client`, `NoPreference` (xdg only: a decoration object without a mode) or `Unspoken`. `parent` is the xdg parent or `WM_TRANSIENT_FOR`, and `app_id` the xdg app id or `WM_CLASS`'s class. The X-only fields (`position`, `types`, `states`, `input`) panic on an xdg toplevel. There's no `empty` or `override_redirect`, since `map` only returns managed windows; tests of pending windows make them by hand through `x11::XWindowSpec`, the X client's own spec, renamed from `WindowSpec`. `map_window`, `map_x11_window`, `map_ssd`, `map_with_mode` and `map_titled` go.
- **What `map` promises:** policy manages the window, its frame is drawn, and the client has seen everything Caminito sent it. For xdg that's the second roundtrip. For X11 it's `dispatch_until` managed and framed, then `sync`: an X roundtrip, since Xwayland answers Caminito before it answers the client, and one dispatch to read the answers.
- **Queries on the fixture:** `f.policy_id(w)`, `f.pid(w)`, `f.mapped(w)`, `f.geometry(w)`, `f.frame_extents(w)`, `f.visible_frame(w)`, `f.button_rect(w, function)`, `f.button_state(w, function)` and `f.control_under_pointer()`. No test reads a window's geometry or `Mapped` through `state.caminito` any more.
- **Verbs for either kind:** `f.sync(w)`, `f.close_requested(w)` and `f.draw_at_asked_size(w, size)`. `w.xdg() -> WindowId` and `w.x11() -> u32` are the way out to protocol-specific calls, and each panics on the other kind; `w.client()` gives the client.
- **Both kinds by default:** `on_both_kinds!` in `fixture.rs` expands one body into `<name>::xdg` and `<name>::x11` tests. All of frames.rs runs on both, as do grabs.rs's Alt+drag, Alt+middle-drag and lost-keyboard tests, debug.rs's dump and frame-debug tests, and the cursor test. What stays on one kind says why in its module doc or beside the test: xdg-decoration and KDE decoration, xdg move and resize requests, xdg_shell, popups, the input and bindings tests (they check what a Wayland client sees), settings (its windows only fill workspaces), and xwayland_windows.rs. The X11 dump snapshot writes X ids as `[X id]`.
- **Guessed dispatches go:** 24 bare `f.dispatch()` calls after a map or a roundtrip, and none had to come back. The pointer-focus lag the review found (`update_pointer_focus` before `refresh_frames`, candidate 5) didn't show: no test hovers a frame in the pass that first draws it. Five stay, each after a direct state change: `add_output` ×3, `policy.activate` and `toggle_frame_debug`.
- **The fixture ends Xwayland when it's dropped:** Smithay only tries displays `:0` to `:32`, fewer than the 45 fixtures that now start Xwayland (39 tests). `X11Wm::start_wm` leaves a source in the event loop that holds the loop's handle, so an X11 fixture's loop, its `Display` and Xwayland's Wayland connection outlive it, and so does Xwayland. `Drop` stops Xwayland through the test-only `Caminito::stop_xwayland_in_test`, which gives the display's lock back, then signals the Xwayland child it finds under `/proc`. `rustix`'s `process` feature is a dev-dependency for it.
- **CODING_STANDARDS.md, Tests:** map with `Fixture::map`, read window facts through the fixture, and run shared behaviour on both kinds through `on_both_kinds!` unless the test says why not.
- Left for later: [42](42-xwayland-restart.md) restarts Xwayland, and each `X11Wm` it replaces leaves its event source, and a loop handle, in the loop.

## Acceptance criteria

- [x] No test file outside `fixture.rs` reads `state.caminito.policy` for a window's geometry or `Mapped`, or defines its own map helper.
- [x] frames.rs's shared-behaviour tests, the geometry half of grabs.rs's Alt+drag and Alt+middle-drag tests, and the debug dump test run as `::xdg` and `::x11`.
- [x] Every bare `f.dispatch()` left in `caminito/src/tests/` follows a direct state change or carries a comment naming its cause.
- [x] The existing assertions all still hold. The commit body gives `cargo test -p caminito`'s wall time before and after.
