# Default output scale

Type: implementation
Status: open
Milestone: M4
Blocked by:
Map: [Camino v0.1](../map.md)

## What to build

The scale Camino gives an output it has no other word on ([13](13-hidpi-and-accessibility-scope.md), [ADR 0008](../../../docs/adr/0008-fractional-default-scale-integer-x11-scale.md)), as a pure function in the `output` module, with `window-scaling-factor` read by the settings thread.

- **KWin's `chooseScale`, ported as it is (13):**
  - **Inputs:** the EDID's physical size in mm, the preferred mode's size in pixels, whether the output is internal, and whether a lid switch exists.
  - **Target DPI:** 125 for an internal output with a lid switch, 150 for an internal output without one, 30.5 for an external output more than 500 mm tall (a TV), and 96 otherwise.
  - **Raw scale:** DPI ÷ target on each axis. It's clamped to at least 1, and to at most mode size ÷ 800 and 3, so at least 800 logical pixels remain. The smaller axis wins.
  - **Rounding:** to 5% steps (6/120). Below 1.20 it becomes 1, and within 0.06 of an integer it snaps to that integer.
  - **Bad sizes:** an EDID size under 3 mm on either axis gives 1.
- **`org.mate.interface window-scaling-factor` (09, 13):** 0 uses the rule. 1 or 2 forces that integer. The settings thread reads it, with the vendored schema in the test schemas.
- **Pure:** no Smithay output or libinput in the function. [51](51-output-lifecycle.md) calls it, and [56](56-libinput-on-the-tty.md) supplies the lid switch.

## Acceptance criteria

- [ ] Unit tests, written before the code, on a table of KWin's EDID cases (`autotests` of `chooseScale`), each rounding rule's edges, a bad size and a TV.
- [ ] The table includes the developer's monitors: each one's EDID size, preferred mode and whether it's internal, read from `/sys/class/drm/*/edid` (`edid-decode`), with the scale the rule picks for it.
- [ ] Settings translation tests for `window-scaling-factor` 0, 1, 2 and an out-of-range value.

## Notes

- The developer's monitors' EDIDs are read on their machine; the ticket records them in its notes.
