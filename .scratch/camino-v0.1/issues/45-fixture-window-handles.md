# A fixture that hands out window handles

Type: implementation
Status: open
Milestone: M3
Blocked by: 44
Map: [Camino v0.1](../map.md)

## What to build

One way for a test to map a window of either kind and ask about it, before tickets 36–42 add their X11-tier tests. From the architecture review of 29 September, candidate 4. Test-only: no behaviour changes.

- **`ClientWindow` is the handle:** a `Copy` id for a window of either kind. The fixture holds X11 clients by index as it does Wayland ones, so `ClientId` knows its kind and the X11 variant needs no `Arc<X11Client>`. frames.rs's `Framed` goes.
- **`Fixture::connected(kind) -> (Fixture, ClientId)`** replaces `connected()` and `with_x11()`.
- **`f.map(client, WindowSpec) -> ClientWindow`:** one spec for both tiers, moved from `camino-test-client::x11` into the fixture. It defaults to 400 × 300, decorated and titled. A field that means nothing on one kind, such as `app_id` or `override_redirect`, says so in its doc and is asserted against there. `empty: true` panics, because `map` only returns managed windows; pending-window tests keep the low-level client calls. `map_window`, `map_x11_window`, `map_ssd`, `map_with_mode` and `map_titled` go.
- **What `map` promises:** policy manages the window, its frame is current, and the client has seen everything Camino sent it. For xdg that's the second roundtrip. For X11 it's `dispatch_until` managed and framed, then an X roundtrip after Xwayland's echoes. Tests of either kind need no settling afterwards.
- **Queries on the fixture:** `f.geometry(w)`, `f.frame_extents(w)`, `f.button_rect(w, function)`, `f.control_under_pointer()` and `f.mapped(w)`, and none of them are read through `state.camino` in a test. The per-file `policy_id`, `mapped` and `geometry` helpers go, as does frames.rs's read of `frame_pointer.hover`.
- **Verbs for either kind:** `f.sync(w)`, `f.close_requested(w)` and `f.draw_at_asked_size(w, size)`, the ones `Framed` needed. `w.xdg() -> (ClientId, WindowId)` and `w.x11() -> u32` are the way out to protocol-specific calls, and each panics on the other kind.
- **Two tiers by default:** a `macro_rules!` in `fixture.rs` expands one body into `<name>::xdg` and `<name>::x11` tests. Tests of behaviour both tiers share run on both: frames.rs's, the geometry half of grabs.rs's Alt+drag and Alt+middle-drag tests, and the debug dump. A test that stays on one tier says why: xdg-decoration, xdg move and resize requests, xdg_shell, popups, Wayland input snapshots, and xwayland_windows.rs.
- **Guessed dispatches go:** about 22 bare `f.dispatch()` calls after a map or a roundtrip that already settled. If a test fails without one, it goes back with a comment naming the real cause and the ticket that fixes it, such as `update_pointer_focus` running before `refresh_frames` (review candidate 5). The six after direct state changes (`add_output`, `policy.activate`, frame debug toggles) stay.
- **CODING_STANDARDS.md, Tests:** map windows with `Fixture::map` and read window facts through the fixture's queries, not `state.camino`; tests of behaviour both tiers share run on both through the two-tier macro unless the test says why not.

## Acceptance criteria

- [ ] No test file outside `fixture.rs` reads `state.camino.policy` for a window's geometry or `Mapped`, or defines its own map helper.
- [ ] frames.rs's shared-behaviour tests, the geometry half of grabs.rs's Alt+drag and Alt+middle-drag tests, and the debug dump test run as `::xdg` and `::x11`.
- [ ] Every bare `f.dispatch()` left in `camino/src/tests/` follows a direct state change or carries a comment naming its cause.
- [ ] The existing assertions all still hold. The commit body gives `cargo test -p camino`'s wall time before and after.
