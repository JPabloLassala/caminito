# A monitor turned off and on again gets its saved layout back

Type: implementation
Status: open
Milestone: M6 (proposed)
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

When a monitor is turned off over output management and turned on again, it comes up at Camino's defaults, though a layout is saved for the monitors connected. The developer wants the saved layout back, as when the monitor is unplugged and plugged in again. Seen on 2026-10-06, in the developer's Session on `camino-git r210.ad13fa1-1`, while checking [87](87-hot-plug-blanks-the-other-monitor.md): with the layout saved for the LG on HDMI-A-1 and the AOC on DP-1 ([83](83-saved-layouts.md)), `wlr-randr --output HDMI-A-1 --off` and then `--on` lit the LG at 60 Hz, its preferred mode, where the layout has 59.94 Hz, and the developer saw the layout gone.

- **Why it is so today:** it's what [58](58-wlr-output-management.md) and 83 decided. `Camino::enable_output` shows an enabled monitor as a new output, at the defaults (51), and the saved layout is restored only when a monitor is plugged in or unplugged (`monitors_changed`). A disabled monitor is still one of the connected set, so the set didn't change.
- **To decide:**
  - What comes back: the whole saved layout, or only the enabled monitor's mode, place and scale, the others staying as they are. The primary output is the layout's too (81).
  - What a client's own request does. `wlr-randr --on` names no mode, place or scale, but camino-displays ([84](84-camino-displays.md)) sends all three when it enables a monitor, and those should stand.
  - What counts as the layout for the set when the saved one has the monitor disabled.
  - Whether a configuration applied since, and not saved, is lost when the layout comes back.

## Acceptance criteria

- [ ] Headless tests, written before the code: with a layout saved for two monitors, one disabled over output management and enabled again with no mode, place or scale asked comes up as the layout has it; what a client asks for when it enables a monitor stands.
- [ ] On the developer's machine, `wlr-randr --output HDMI-A-1 --off` and `--on` leave the two monitors as the saved layout has them, the primary output with them.
- [ ] 58's and 83's decisions that this changes are noted in this ticket's resolution.
