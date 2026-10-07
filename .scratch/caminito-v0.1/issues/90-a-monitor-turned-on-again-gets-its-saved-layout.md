# A monitor turned off and on again gets its saved layout back

Type: implementation
Status: resolved
Milestone: M6
Blocked by: none
Map: [Caminito v0.1](../map.md)

## What to build

When a monitor is turned off over output management and turned on again, it comes up at Caminito's defaults, though a layout is saved for the monitors connected. The developer wants the saved layout back, as when the monitor is unplugged and plugged in again. Seen on 2026-10-06, in the developer's Session on `caminito-git r210.ad13fa1-1`, while checking [87](87-hot-plug-blanks-the-other-monitor.md): with the layout saved for the LG on HDMI-A-1 and the AOC on DP-1 ([83](83-saved-layouts.md)), `wlr-randr --output HDMI-A-1 --off` and then `--on` lit the LG at 60 Hz, its preferred mode, where the layout has 59.94 Hz, and the developer saw the layout gone.

- **Why it is so today:** it's what [58](58-wlr-output-management.md) and 83 decided. `Caminito::enable_output` shows an enabled monitor as a new output, at the defaults (51), and the saved layout is restored only when a monitor is plugged in or unplugged (`monitors_changed`). A disabled monitor is still one of the connected set, so the set didn't change.
- **To decide:**
  - What comes back: the whole saved layout, or only the enabled monitor's mode, place and scale, the others staying as they are. The primary output is the layout's too (81).
  - What a client's own request does. `wlr-randr --on` names no mode, place or scale, but caminito-displays ([84](84-caminito-displays.md)) sends all three when it enables a monitor, and those should stand.
  - What counts as the layout for the set when the saved one has the monitor disabled.
  - Whether a configuration applied since, and not saved, is lost when the layout comes back.

## Acceptance criteria

- [x] Headless tests, written before the code: with a layout saved for two monitors, one disabled over output management and enabled again with no mode, place or scale asked comes up as the layout has it; what a client asks for when it enables a monitor stands.
- [x] On the developer's machine, `wlr-randr --output HDMI-A-1 --off` and `--on` leave the two monitors as the saved layout has them, the primary output with them.
- [x] 58's and 83's decisions that this changes are noted in this ticket's resolution.

## Resolution

Implemented on 2026-10-06, and checked on the developer's machine the same day (below).

A configuration that enables a disabled monitor and asks no mode, place or scale of it brings back the layout saved for the connected monitors, if that layout has the monitor enabled (`Caminito::plan_monitors_return` in `caminito/src/output/saved.rs`, applied by `State::apply_with_saved_layout`).

The decisions:

- **What comes back: the whole layout**, of the monitor and of the outputs that are enabled, the primary output with them. Disabling the monitor moved the others to close the gap (82), so the monitor's own place alone wouldn't give the layout back.
- **What a client asks stands.**
  - Which monitors are enabled is the configuration's: the layout lights none that the configuration leaves disabled, and disables none.
  - A monitor enabled with a mode, a place or a scale asked gets all of what the configuration says, and nothing from the layout. caminito-displays (84) sends all three, so it never brings the layout back.
  - Of an output that's enabled already, what the configuration asks and the output doesn't already have stands, and the layout says the rest. What the output already has is no ask, since a client may send back what every head has, as the test client does. So a place, mode or scale asked that equals the current one, in the same configuration as a bare enable, gives way to the layout's.
- **A layout that has the monitor disabled says nothing of it enabled**: it comes up at the defaults, as before.
- **A configuration applied since, and not saved, is lost**, as when a monitor is plugged in.
- **A saved mode the backend can't show** leaves the rest of the layout restored, with the monitor in its preferred mode, as on hot-plug (83).

What this changes of earlier decisions:

- **58** and **51**: an output enabled over output management is no longer always shown as a new output at the defaults. `Caminito::enable_output` still does that, and the layout is applied over it in the same configuration, so the monitor is lit once, in its saved mode.
- **83** and **ADR 0014**: the saved layout is restored at startup, on hot-plug, and now on such an enable. A configuration from `wlr-randr` lasts for the Session or until then. The ADR is amended.

Not covered by a test: three monitors, with the layout coming back for the ones enabled while another stays disabled. The places are the saved ones, healed (82).

The 13 tests that take Caminito's names on the session bus fail while a Caminito Session runs on it. The suite was run under `dbus-run-session`.

### On the developer's machine (2026-10-06)

On `caminito-git r220.daef32d-1`, in a Session started at 17:03, `.scratch/caminito-v0.1/issues/90-off-on.sh` turned each monitor off for 3 s and on again with a bare `wlr-randr --on`. The developer says the layout was fully restored, and `wlr-randr` and `PrimaryOutput` after it agree with the layout saved then: the LG at 0,0 at a scale of 1.75, the AOC at 2194,0 at a scale of 1, and HDMI-A-1 the primary output.

- **A weaker check than the ticket's:** the layout had been saved again at 17:00, with both monitors in their preferred modes (59.997 Hz and 60 Hz), so the run doesn't show a saved mode that isn't the preferred one coming back. The headless tests do. The place, the scale and the primary output were the layout's and not the defaults: with `window-scaling-factor` at 2, the defaults give a monitor enabled again a scale of 2, as the same script showed on r210 an hour before.
- **mate-panel died at each monitor's return**, twice within 5 s, and mate-session didn't start it a third time. It's the crash 87 saw, on r210 too, and not this change's: [91](91-mate-panel-dies-when-a-monitor-is-lit.md).
