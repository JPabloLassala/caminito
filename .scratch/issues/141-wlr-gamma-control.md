# wlr-gamma-control

Epic: night-light
Type: implementation
Status: open: the code is in, and the check with gammastep and wlsunset on the tty is the developer's
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

- [x] Headless tests with the test client: the global isn't there on headless or winit, and isn't offered to a sandboxed client.
- [x] Unit tests of the reading: a short ramp, a long ramp and an unreadable fd each send `failed`.
- [x] A test seam on the per-output state shows a second control on a held output failing, Night Light stepping back on one output of two, and fading back when the first client's control is destroyed and when the client is killed.
- [ ] On the tty, by the developer: `gammastep -O 3000` and `wlsunset` work; with Night Light on, only the output gammastep holds is gammastep's; quitting gammastep fades Night Light back in.

## Done (2026-10-10)

- **Tested first:** the reading as a pure function (`read_table`, unit tests of a short and a long ramp, a pipe and a write-only fd), Night Light's stepping back and in as `NightLightState` unit tests with `now` as an argument, then headless tests through the test client's new `gamma_control` module.
- **The test seam:** `Fixture::with_gamma(size)` gives the headless backend's monitors gamma tables, standing in for the tty's CRTCs, and offers the global; the headless backend keeps the client's table for the test to read, and the dump shows Night Light's side. Plain headless never offers the global, which one test checks. Winit doesn't either, by construction: only `Tty::new` calls `offer_gamma_control`. Winit isn't run in tests.
- **Not ported, written from the XML:** the ticket and the spec say to port niri's `src/protocols/gamma_control.rs`, but niri is GPL-3.0 and Caminito is GPL-2.0-or-later. Copying it would make the binary GPL-3.0, which goes against 03 and the spec's own rule about redshift's tables. So `protocols/gamma_control.rs` follows niri's design (at d0cd58e): the handler's three calls, one live control per output, and a failed control forgotten at once. It cites niri but copies no code. Relicensing to take niri's code is the developer's call.
- **Reading the table:** with `pread` from the file's start, as wlroots does, plus one byte more to catch a long file. A client needn't seek back after writing, and a pipe fails with `ESPIPE` rather than blocking the loop.
- **When Night Light steps back:** at the first `set_gamma`, not at `get_gamma_control`. Until then the control holds the output (a second one fails, and the dump says `gamma_client: true`), but Night Light's table stays. A control destroyed before setting a table changes nothing. Once a table was set, letting go (destroying the control, a failed `set_gamma`, or the client exiting) makes the table neutral again, and Night Light fades in from 6500 K over its 2 s. The spec says Night Light fades "from the table the output has", but a client's table has no temperature, so neutral is where it starts.
- **The tty:** `CrtcGamma` keeps the client's table beside Night Light's temperature and writes the client's while it's there. The refresh pass still hands every output's temperature on, so the forgetting that a resume, a modeset or a newly lit CRTC causes (137) writes the client's table again. While the session is paused, `set_gamma` keeps the table for the resume and returns success. A write that fails on an active, lit CRTC sends `failed`.
- **A removed output:** the refresh pass fails the control of any output that is no longer shown (unplugged or disabled by output management), and tells the backend to forget its table.
- **The dump:** each output has `gamma_client`, and `temperature` is `null` while a client's table is on it. The two debug snapshots gained the field.
- **06 amended:** its deferred list and its privileged-globals rule now name gamma control.
- **Not run:**
  - the check on the tty with gammastep and wlsunset, which is the developer's (and 143's);
  - `cargo deny`;
  - the code review's two sub-agents: this session had no tool to spawn them, so the review was done in one pass against CODING_STANDARDS.md and the spec.
- **The suite here:** run on Ubuntu, not on the developer's Arch machine. 1245 of caminito's tests pass. The 13 that fail are font-metric and xkb/Xwayland differences in `debug` (the frame's `top`, 31 against 29), `decoration`, `keymap`, `window_properties` and `x11_scale`, which this change doesn't touch. The debug snapshots' new `gamma_client` lines match. Clippy is clean on `caminito` and `caminito-test-client` once `manual_range_contains` is allowed: clippy 1.97 finds three of those in `caminito-theme`, which is untouched, as 137 found.
- **The shared target dir:** other worktrees build the same artifact names into it, so each test run first touched every source, then checked with `--list` that the binary held this branch's tests.
