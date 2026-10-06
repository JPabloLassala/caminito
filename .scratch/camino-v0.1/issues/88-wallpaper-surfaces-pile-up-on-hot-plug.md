# Each hot-plug leaves more wallpaper layer surfaces

Type: implementation
Status: open
Milestone: before v0.1
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

In the Session, every monitor plugged in or unplugged leaves more `mate-background` layer surfaces than there were, and they stay. [60](60-tty-checks-left-over.md) saw it on 2026-10-06, in the developer's Session on `camino-git r195.9e4baac-1` with `mate-settings-daemon-camino 1.29.1-2`, in each output's `layer_surfaces` in `DumpState`:

| When | HDMI-A-1 | DP-1 |
| --- | --- | --- |
| Before, both connected | 1 | 1 |
| 5 s after DP-1 was unplugged | 1 | gone |
| 5 s after DP-1 was plugged back | 3 | 4 |
| 5 s after HDMI-A-1 was unplugged | gone | 3 |
| 5 s after HDMI-A-1 was plugged back | 5 | 3 |
| 10 minutes later | 5 | 3 |

All of one output's have the same geometry, so only the top one is seen. Nothing looked wrong on screen.

- **The surfaces are msd's.** Its background plugin draws the wallpaper under Camino as a layer surface per monitor (`msd-background-manager-wayland.c`, [71](71-mate-settings-daemon-camino.md)'s carried patch).
- **Not known:** whether msd makes a surface for every monitor at each change without destroying the old ones, or destroys them and Camino keeps them. A `WAYLAND_DEBUG=1` msd through one hot-plug tells the two apart. Whether the count also grows on a change that isn't a hot-plug (a mode, a scale, `wlr-randr --off` and `--on`) wasn't looked at.

## Acceptance criteria

- [ ] The cause is written down in this ticket's notes, with a headless test written before the fix where the cause is Camino's.
- [ ] After a monitor is unplugged and plugged back, each output has one `mate-background` layer surface in `DumpState`.
