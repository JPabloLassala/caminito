# wlr-output-management

Type: implementation
Status: resolved
Milestone: M4
Blocked by: 51
Map: [Caminito v0.1](../map.md)

## What to build

`zwlr_output_management_unstable_v1` version 4, the first protocol Caminito writes itself ([06](06-v01-protocol-and-integration-set.md), [15](15-crate-and-module-architecture.md)), in `protocols::output_management`. Its real user, msd's wlrandr plugin and the Display capplet, arrives in M5; in M4 it's tested with `wlr-randr`.

- **Heads and modes:** each output with its modes, current mode, position, scale, transform and enabled state, sent on bind and after every change, with `done` and a serial.
- **Configurations:** a client's configuration is tested or applied whole. Applying changes the outputs through [51](51-output-lifecycle.md), marking the scale and position as set, not default. A stale serial is cancelled. A configuration the backend can't apply fails, and nothing changes.
- **Scales:** fractional, snapped to 1/120 (13), with preferred scales re-sent ([52](52-preferred-scale-across-outputs.md)).
- **Written fresh:** niri's and cosmic-comp's implementations are GPL-3, so they're design references only (03).
- **Offered to every client for now.** Hiding it from `wp_security_context_v1` sandboxes, as 06 says, lands in M5 with the other privileged globals (decided 2026-09-30).
- **Test client:** a wlr-output-management client in `caminito-test-client`.

## Acceptance criteria

- [x] Headless tests, written before the code: the heads and modes a client sees for two outputs; applying a new scale, position and mode changes the outputs and marks them set; a test-only configuration changes nothing; a stale serial is cancelled; disabling an output removes it as unplugging does.
- [x] Nested, `wlr-randr` lists the winit output, and `wlr-randr --output winit --scale 1.5` rescales it.
- [ ] On the tty as `caminitotest`, `wlr-randr` changes a monitor's mode, scale and position, and turns one off and on.

## Notes

- **Two parts.** `protocols::output_management` owns the protocol: each client's head and mode objects, the serial, and the protocol errors. It knows nothing of outputs. `OutputManagementState::refresh` takes the heads there are, as plain data, in the refresh pass, and sends only what changed, then `done` with a new serial. `OutputManagementHandler` answers a configuration. `output.rs` and `handlers/output_management.rs` are Caminito's side: the heads from the outputs, and a configuration checked whole, then applied whole.
- **A configuration is applied in this order:** the steps that can fail first (a monitor lit again, a mode switched), then the monitors turned off, then the places and scales, with one `lay_out_outputs` at the end. If a step fails, the steps before it are taken back, and the client gets `failed`.
- **Set, not default:** only a property the configuration names is marked `ChosenBy::OutputManagement`. `wlr-randr` sends the mode, place, scale and transform of every enabled head, so after it runs, every enabled output is set. So `wlr-randr --output A --off` leaves the other outputs where they were; they don't close the gap. ([82](82-layout-healing.md) changed this: a place has no such mark any more, and Caminito heals the layout whoever placed its outputs. The scale keeps its mark.)
- **Disabled outputs** are kept in `Caminito::disabled_outputs`, and are shown as heads that are not enabled. `disable_output` is `remove_output` plus that list. An output enabled again keeps its `Output` and its policy id, gets a new `wl_output` global and a new record, and so starts at the defaults (51) unless the configuration names a place and a scale. A disabled monitor that is unplugged is only forgotten.
- **Modes:** on the tty an output now lists every mode of its connector, and `DrmOutput::use_mode` switches. A custom mode is taken only if it's one of the output's modes (with refresh 0, the first of that size); Caminito makes no modelines. Nested, the window's size is the only mode, and the output can't be turned off, so such a configuration fails.
- **Not here: rotation and adaptive sync.** Heads report their transform, and adaptive sync as off. A configuration that asks for another transform, or for adaptive sync, fails. No ticket asks for rotation yet. Nested, `wlr-randr` shows `flipped-180`, the transform the winit output has for GLES.
- **A head left out of a configuration stays as it is.** The protocol makes that an error, but wlroots doesn't, and clients are written against wlroots.
- **Stale:** a configuration made at an older serial, or one that names a head that's gone, gets `cancelled`. The serial moves on as soon as a configuration is applied, so a second one made before it, even in the same dispatch, is cancelled. `done` with the new serial follows in the refresh pass, also when the configuration changed nothing.
- **Scales** outside 0.5 to 8 make the configuration fail.
- **`test` is weaker than `apply` on the tty.** It checks the modes, the transform, the scale and that Caminito has the DRM master, but asks the GPU nothing. So a configuration can pass `test` and fail `apply`. If taking back a failed configuration's mode switch fails too, Caminito logs it and the mode stays.
- **A monitor turned off that sends a hot-plug change** is lit again as a new output, since a changed connector is removed and added (55). Not seen yet; it needs a real monitor.
- **The winit window's resize** changes the head's mode list. The head stays: the new mode object is sent, then the old one is finished.
- **Scales** from a client are snapped to 1/120. A scale sent back as a `wl_fixed` (1.7 comes as 435/256) snaps to what it was.
- **Tests:** `tests/output_management.rs`, with a wlr-output-management client in `caminito-test-client::output_management` that sends every head, as `wlr-randr` does. The headless backend plugs monitors with several modes, and `Headless::refuse_mode` makes one fail, for the test that a failed configuration takes its earlier steps back. `a_client_lists_the_globals` gains `zwlr_output_manager_v1 v4`.
- **Nested, checked in the virtual machine** with `wlr-randr` 0.5.0: it lists `winit` with its one mode, `flipped-180` and scale 1; `--output winit --scale 1.5` succeeds and the list then shows 1.5; `--off` and `--transform 90` get "failed to apply configuration"; `--dryrun --pos 10,10` succeeds and changes nothing.
- **The tty criterion isn't checked, and is unticked.** Another ticket's checks had the virtual machine's tty while this was written, so none of the tty code (mode switch, off and on) has run. It moves to [60](60-tty-checks-left-over.md), on the developer's machine with its two monitors (resolved 2026-10-04 on that basis, as 55 was).
- **An earlier attempt** is on `origin/ticket-58-output-management` (0cd91ec, WIP, CI failed). This implementation doesn't build on it.
