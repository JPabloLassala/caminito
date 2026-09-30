# wlr-output-management

Type: implementation
Status: open
Milestone: M4
Blocked by: 51
Map: [Camino v0.1](../map.md)

## What to build

`zwlr_output_management_unstable_v1` version 4, the first protocol Camino writes itself ([06](06-v01-protocol-and-integration-set.md), [15](15-crate-and-module-architecture.md)), in `protocols::output_management`. Its real user, msd's wlrandr plugin and the Display capplet, arrives in M5; in M4 it's tested with `wlr-randr`.

- **Heads and modes:** each output with its modes, current mode, position, scale, transform and enabled state, sent on bind and after every change, with `done` and a serial.
- **Configurations:** a client's configuration is tested or applied whole. Applying changes the outputs through [51](51-output-lifecycle.md), marking the scale and position as set, not default. A stale serial is cancelled. A configuration the backend can't apply fails, and nothing changes.
- **Scales:** fractional, snapped to 1/120 (13), with preferred scales re-sent ([52](52-preferred-scale-across-outputs.md)).
- **Written fresh:** niri's and cosmic-comp's implementations are GPL-3, so they're design references only (03).
- **Offered to every client for now.** Hiding it from `wp_security_context_v1` sandboxes, as 06 says, lands in M5 with the other privileged globals (decided 2026-09-30).
- **Test client:** a wlr-output-management client in `camino-test-client`.

## Acceptance criteria

- [ ] Headless tests, written before the code: the heads and modes a client sees for two outputs; applying a new scale, position and mode changes the outputs and marks them set; a test-only configuration changes nothing; a stale serial is cancelled; disabling an output removes it as unplugging does.
- [ ] Nested, `wlr-randr` lists the winit output, and `wlr-randr --output winit --scale 1.5` rescales it.
- [ ] On the tty as `caminotest`, `wlr-randr` changes a monitor's mode, scale and position, and turns one off and on.
