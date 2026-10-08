# Default output scale

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M4
Blocked by:
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The scale Caminito gives an output it has no other word on ([13](13-hidpi-and-accessibility-scope.md), [ADR 0008](../../docs/adr/0008-fractional-default-scale-integer-x11-scale.md)), as a pure function in the `output` module, with `window-scaling-factor` read by the settings thread.

- **KWin's `chooseScale`, ported as it is (13):**
  - **Inputs:** the EDID's physical size in mm, the preferred mode's size in pixels, whether the output is internal, and whether a lid switch exists.
  - **Target DPI:** 125 for an internal output with a lid switch, 150 for an internal output without one, 30.5 for an external output more than 500 mm tall (a TV), and 96 otherwise.
  - **Raw scale:** DPI ÷ target on each axis. It's clamped to at least 1, and to at most mode size ÷ 800 and 3, so at least 800 logical pixels remain. The smaller axis wins.
  - **Rounding:** to 5% steps (6/120). Below 1.20 it becomes 1, and within 0.06 of an integer it snaps to that integer.
  - **Bad sizes:** an EDID size under 3 mm on either axis gives 1.
- **`org.mate.interface window-scaling-factor` (09, 13):** 0 uses the rule. 1 or 2 forces that integer. The settings thread reads it, with the vendored schema in the test schemas.
- **Pure:** no Smithay output or libinput in the function. [51](51-output-lifecycle.md) calls it, and [56](56-libinput-on-the-tty.md) supplies the lid switch.

## Acceptance criteria

- [x] Unit tests, written before the code, on a table of KWin's EDID cases (`autotests` of `chooseScale`), each rounding rule's edges, a bad size and a TV.
- [x] The table includes the developer's monitors: each one's EDID size, preferred mode and whether it's internal, read from `/sys/class/drm/*/edid` (`edid-decode`), with the scale the rule picks for it.
- [x] Settings translation tests for `window-scaling-factor` 0, 1, 2 and an out-of-range value.

## Notes

- The developer's monitors' EDIDs are read on their machine; the ticket records them in its notes.
- **The function** is `output::default_scale(&Monitor, WindowScalingFactor)`. `Monitor` holds the EDID size in mm (Smithay's `Size<i32, Raw>`, as `PhysicalProperties` has it), the size of the mode to be shown, whether the output is internal, and whether a lid switch exists. `Forced(n)` gives `n` whatever the EDID says; `Auto` runs `kwin_scale`, ported from KWin's `src/outputconfigurationstore.cpp:859-931` at commit 79ea0aa5. It's unused outside tests until [51](51-output-lifecycle.md) calls it, which its `expect(dead_code)` will flag.
- **One correction to 13's summary:** KWin keeps 800 logical pixels for laptops and external outputs, but 360 for an internal output without a lid switch (a phone). The port keeps KWin's 360, since 13 chose to port the rule as it is, and KWin's Pixel 3a and OnePlus 6 rows need it. For Caminito this only matters on an internal panel without a lid.
- **The mode:** KWin chooses the scale for the mode it picked, which isn't always the preferred one (its 30 Hz 4K row gets 2560×1440). The function takes whichever mode size the caller shows; 51 passes the preferred mode.
- **The developer's monitors** (a desktop: no internal output and no lid switch), read with `edid-decode /sys/class/drm/card1-*/edid` on 2026-09-30:

  | Connector | Monitor | EDID size | Preferred mode | Internal | Scale |
  |---|---|---|---|---|---|
  | DP-1 | AOC G2490W1G4 | 527 × 296 mm (DTD; base block 53 × 30 cm) | 1920×1080 | no | 1.0 (93 DPI) |
  | HDMI-A-1 | LG HDR 4K | 600 × 340 mm (DTD; base block 60 × 34 cm) | 3840×2160 | no | 1.70 (161 DPI) |

  The AOC gets 1.0 at either size.
- **`window-scaling-factor`** is `Interface::window_scaling_factor`, a `WindowScalingFactor` of `Auto` for 0 and `Forced(n)` for 1 or 2. A value outside the schema's 0–2 is clamped and logged as out of range. The vendored `org.mate.interface` schema already had the key, and a fixture test reads it through the settings thread.
- **Tests:** `output/tests.rs` has all 20 of KWin's `testGenerateConfigs` rows (each at the mode KWin picks), the developer's monitors, the 3 mm bound, the TV's 500 mm edge, each target DPI, the 800 and 360 pixel limits, the 3 cap, the smaller axis, and `round_scale`'s steps, its 1.20 floor and its 0.06 snaps. `settings/translate/tests.rs` covers 0, 1, 2, 3 and −1.
