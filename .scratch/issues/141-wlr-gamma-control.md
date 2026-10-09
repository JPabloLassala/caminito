# wlr-gamma-control

Epic: night-light
Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: 137
Map: [Night Light](../night-light/map.md)

## What to build

`zwlr_gamma_control_manager_v1`, version 1, under "Caminito: wlr-gamma-control" in [the spec](../night-light/spec.md) and [ADR 0023](../../docs/adr/0023-caminito-owns-night-light-and-serves-wlr-gamma-control.md). Port niri's `src/protocols/gamma_control.rs`; Smithay re-exports `wayland-protocols-wlr`. It amends [06](06-v01-protocol-and-integration-set.md)'s line that left it out.

- **The global is privileged and on the tty backend only.** It isn't offered to a client in a `wp_security_context_v1` sandbox, by the same filter as the other privileged globals.
- **`get_gamma_control`:**
  - It sends `gamma_size` from the CRTC.
  - It sends `failed` at once when the output has no gamma, or another **gamma client** holds it.
- **`set_gamma(fd)`:** reads exactly `size × 3` u16s with nothing after, or sends `failed`. The table is set at once, with no fade.
- **Night Light steps back from that output alone** while a gamma client holds it. It fades back in when the control is destroyed or fails, or the client exits.
- **A removed output** sends `failed` to its control.
- **After a session resume or a modeset,** the gamma client's table is the one applied again.
- `DumpState` shows which outputs a gamma client holds.

## Acceptance criteria

- [ ] Headless tests with the test client: the global isn't there on headless or winit, and isn't offered to a sandboxed client.
- [ ] Unit tests of the reading: a short ramp, a long ramp and an unreadable fd each send `failed`.
- [ ] A test seam on the per-output state shows a second control on a held output failing, Night Light stepping back on one output of two, and fading back when the first client's control is destroyed and when the client is killed.
- [ ] On the tty, by the developer: `gammastep -O 3000` and `wlsunset` work; with Night Light on, only the output gammastep holds is gammastep's; quitting gammastep fades Night Light back in.
