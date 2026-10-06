# Plugging or unplugging a monitor blanks the other one

Type: implementation
Status: open
Milestone: before v0.1
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

When a monitor is plugged in or unplugged on the tty, the monitor that stays goes black for a short time. The *Hot-plugging a monitor* checklist item expects that it never goes black or freezes. [60](60-tty-checks-left-over.md) saw it on 2026-10-06, in the developer's Session on `camino-git r195.9e4baac-1` (NVIDIA RTX 3060, driver 615.71.09, kernel 7.2.8): the LG on HDMI-A-1 at 3840x2160 and 59.94 Hz, the AOC on DP-1 at 1920x1080 and 144 Hz, with a layout saved for the two ([83](83-saved-layouts.md)).

- **It happened all four times:** unplugging and re-plugging the AOC blanked the LG, and unplugging and re-plugging the LG blanked the AOC. How long it lasted wasn't measured. Nothing froze, and no window was lost.
- **The log has no `can't light` line,** and no warning at an unplug. At each re-plug it has two rounds of `Adding connector` and `Setting new mode` for the monitor that came back, a second apart, and one `Failed to destroy old mode property blob: No such file or directory`.
- **Not known:** whether the blanking is Camino's or the driver's. The first things to look at:
  - `Gpu::connect` lights a re-plugged monitor in its preferred mode, and the saved layout's restore then switches it to the saved one (60 Hz, then 144 Hz for the AOC). That's two mode sets where one would do, and it's what the two rounds in the log look like.
  - Whether the commit that lights or drops one CRTC touches the other's, and whether a mode set on one head blanks all of them on NVIDIA whatever the compositor does. Another compositor on the same machine would tell the two apart.
  - An unplug sets no mode on the monitor that stays, yet it blanked too.

## Acceptance criteria

- [ ] The cause is written down in this ticket's notes: Camino's, the driver's, or both.
- [ ] What is Camino's is fixed: a re-plugged monitor with a saved layout is lit once, in the saved mode.
- [ ] On the developer's machine, the monitor that stays shows its picture without a break through an unplug and a re-plug of the other one, or what's left is in the README's *Known limitations* with its cause.
