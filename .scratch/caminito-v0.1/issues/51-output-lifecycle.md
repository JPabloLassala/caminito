# Output lifecycle

Type: implementation
Status: resolved
Milestone: M4
Blocked by: 50
Map: [Caminito v0.1](../map.md)

## What to build

Outputs that come and go, with a position and a scale Caminito picks until a client sets them ([13](13-hidpi-and-accessibility-scope.md), [06](06-v01-protocol-and-integration-set.md)). Built and tested on the headless backend, for [55](55-the-tty-backend.md)'s connectors and [58](58-wlr-output-management.md)'s changes to use.

- **Added:** in its preferred mode, to the right of the outputs there are, at [50](50-default-output-scale.md)'s default scale, or `window-scaling-factor`'s integer when it's set.
- **Removed:** policy moves its windows off it (`caminito-policy` already removes outputs), the X11 scale is worked out again (41), and redraws are queued.
- **Default or set:** Caminito keeps, per output, whether its scale and position are its own default or were set over output management. A change of `window-scaling-factor` re-applies only to outputs still at Caminito's default scale (13).
- **Re-laying out:** when an output is added, removed or disabled, the outputs still at Caminito's default position are laid out left to right again. Outputs a client positioned keep their positions (decided 2026-09-30). msd reapplies saved layouts in M5.
- **Nothing persisted:** a re-plugged output starts at the default again (06).
- **Fixture:** helpers to plug and unplug a headless output with a given EDID size and mode, and to mark an output's scale or position as set, as output management will.

## Acceptance criteria

- [x] Headless tests, written before the code: adding two outputs lays them out left to right at their default scales; removing the left one moves its windows and lays the other at the origin; a client-positioned output keeps its position across an unplug; a re-plugged output is back at its default.
- [x] Headless tests: changing `window-scaling-factor` re-scales default outputs only, and the X11 scale follows.
- [x] The policy proptests include plugging and unplugging outputs, and the invariants hold.

## Notes

- **One path for every change.** `Caminito::add_output`, `remove_output`, `set_output_scale`, `set_output_position`, `output_mode_changed` and a `window-scaling-factor` change all end in `lay_out_outputs`, which places the outputs at the default position and hands policy the whole set at once through the new `Policy::set_outputs`. `add_output` and `remove_output` on policy now go through it too.
- **Windows move with their output.** When an output moves, `set_outputs` moves each window on it (by largest overlap before the change) by the same amount, and its floating geometry with it, so a window keeps its place on its output. Without this, the right output moving to the origin would have left its windows off every output, and policy would have pushed them to its edge. Windows on a removed output still go to the first output, as before.
- **Where default outputs go with positioned ones (decided here):** the outputs at the default position are laid left to right, in the order they were added, starting at the right edge of the outputs a client positioned (or 0 with none), all at y = 0. So they never overlap a positioned output. It can move a default output when a positioned one is added or grows; msd sets every position when it applies a layout (M5), so the mix is rare. ([82](82-layout-healing.md) replaced this: a new output goes right of the rightmost, and the layout is healed after every change.)
- **Records:** `Caminito::output_records` keeps each output's `Monitor`, whether its scale and position were `ChosenBy::Caminito` or `ChosenBy::OutputManagement`, and its global. A removed output's global is disabled at once and destroyed 10 s later, so a client binding it meanwhile doesn't bind a dead global, and windows get `leave` for it. The record goes with the output, so a re-plugged monitor starts at its defaults.
- **Disabling** is `remove_output`, as [58](58-wlr-output-management.md)'s "disabling an output removes it as unplugging does" has it; re-enabling is `add_output`, at the defaults again.
- **Mode changes:** `output_mode_changed` reads the `Monitor` again for the new mode, and an output at Caminito's default scale takes the default for it. The `Monitor`'s mode is the one shown, which `add_output` sets to the preferred one first.
- **Internal outputs** are those whose connector name starts `eDP-`, `LVDS-` or `DSI-`, as KWin decides it. There's no lid switch until [56](56-libinput-on-the-tty.md), so `Monitor::lid_switch` is false for now.
- **Backends:** `virtual_output` now takes an EDID size and only sets the preferred mode; Caminito applies the mode and the scale. The winit output has no EDID size, so its default is 1, or `window-scaling-factor`'s integer. `CAMINITO_WINIT_SCALE`, when set, is applied as output management would set it.
- **Fixture:** `plug(mm, mode)`, `change_mode` and `unplug`, the latter with the backend's `Headless::unplug`; `set_output_scale` and `set_output_position` mark the scale or position set. `add_output(size, scale)` plugs a monitor with no EDID size and sets its scale, so existing tests keep their scales whatever `window-scaling-factor` says.
- **Tests:** `tests/output_lifecycle.rs`, with the developer's AOC (1.0) and LG 4K (1.70): two outputs left to right; unplugging the left one moves a window to the other at the origin (both kinds) and sends `leave` then `enter`; a positioned output keeps its place across an unplug; a re-plugged output is back at its defaults; default outputs go after positioned ones; `window-scaling-factor` 1 then 0 re-scales only the default output and moves the one after it; `window-scaling-factor` 2 takes the X11 scale to 2, and with Xwayland running `Gdk/WindowScalingFactor` follows; a mode change on a default-scale output gives it the default for the new mode (`Fixture::change_mode`). Policy: windows move with a moved output, a maximised window restores onto it, and `set_outputs` for an unplug and re-layout. The proptests gain `Plug` and `Unplug` ops that lay the outputs out left to right as Caminito does, and the invariant that windows are told only of outputs policy has.
- **A flake seen once:** in one of nine workspace runs, `xwayland_crashing_again_and_again_as_it_starts_makes_caminito_give_up_and_run_without_x` failed in `crash_xwayland_starting`. That's the restart timing ticket 49 is about, not outputs. It passed in every other run, and in every run of the `caminito` suite alone.
