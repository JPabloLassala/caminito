# Output lifecycle

Type: implementation
Status: open
Milestone: M4
Blocked by: 50
Map: [Camino v0.1](../map.md)

## What to build

Outputs that come and go, with a position and a scale Camino picks until a client sets them ([13](13-hidpi-and-accessibility-scope.md), [06](06-v01-protocol-and-integration-set.md)). Built and tested on the headless backend, for [55](55-the-tty-backend.md)'s connectors and [58](58-wlr-output-management.md)'s changes to use.

- **Added:** in its preferred mode, to the right of the outputs there are, at [50](50-default-output-scale.md)'s default scale, or `window-scaling-factor`'s integer when it's set.
- **Removed:** policy moves its windows off it (`camino-policy` already removes outputs), the X11 scale is worked out again (41), and redraws are queued.
- **Default or set:** Camino keeps, per output, whether its scale and position are its own default or were set over output management. A change of `window-scaling-factor` re-applies only to outputs still at Camino's default scale (13).
- **Re-laying out:** when an output is added, removed or disabled, the outputs still at Camino's default position are laid out left to right again. Outputs a client positioned keep their positions (decided 2026-09-30). msd reapplies saved layouts in M5.
- **Nothing persisted:** a re-plugged output starts at the default again (06).
- **Fixture:** helpers to plug and unplug a headless output with a given EDID size and mode, and to mark an output's scale or position as set, as output management will.

## Acceptance criteria

- [ ] Headless tests, written before the code: adding two outputs lays them out left to right at their default scales; removing the left one moves its windows and lays the other at the origin; a client-positioned output keeps its position across an unplug; a re-plugged output is back at its default.
- [ ] Headless tests: changing `window-scaling-factor` re-scales default outputs only, and the X11 scale follows.
- [ ] The policy proptests include plugging and unplugging outputs, and the invariants hold.
