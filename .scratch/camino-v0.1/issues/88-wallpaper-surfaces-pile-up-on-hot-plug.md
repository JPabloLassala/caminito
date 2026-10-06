# Each hot-plug leaves more wallpaper layer surfaces

Type: implementation
Status: resolved
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

- [x] The cause is written down in this ticket's notes, with a headless test written before the fix where the cause is Camino's.
- [x] After a monitor is unplugged and plugged back, each output has one `mate-background` layer surface in `DumpState`.

## Notes

- **The cause is msd's.** It makes the surfaces and never destroys them; Camino keeps what a client keeps. So there's no headless test in Camino's suite, and the fix is a patch to msd in the Patched package.
- **How they pile up,** from a `WAYLAND_DEBUG=1` msd through a hot-plug:
  - `rebuild_monitor_windows` destroys every window and makes one for each monitor. Each `gtk_widget_show` makes gtk-layer-shell do a Wayland roundtrip, to have the surface's first configure.
  - A monitor that's plugged in says what it is in more than one batch: `wl_output`'s geometry, mode and scale with a `done`, then xdg-output's place, size and name with a second `done`, since GDK binds xdg-output only once it has the `wl_output`. GDK emits `monitors-changed` at each.
  - The second `done` comes in during the roundtrip of the first window's `show`. `on_monitors_changed` runs inside the rebuild, finds two monitors and a list with no window yet (the first isn't added until `show` returns), and rebuilds: nothing to destroy, two windows made. The rebuild it interrupted then adds its first window and makes its second. Four windows, two on each output.
  - From then on the list isn't one window for each monitor, and the handler tells a hot-plug from a resize by comparing the two counts. So what a later change does depends on the counts: with four windows and one monitor it rebuilds, and can double again; with two windows, both on one output, and two monitors, it only redraws, and the output that came back has no wallpaper at all.
- **Camino does nothing unusual here.** On hardware the larger counts are inferred, not read from a log: The second `done` is what xdg-output asks for from version 3, and any later change to an output (a mode, a scale, a place) inside a rebuild's roundtrip would do the same. On hardware there are more of them at a re-plug, with the saved layout restored, which is how 60 saw 3 and 4 where the headless run has 2 and 2.
- **Reproduced headless, with the real msd:** [88-hot-plug-check/](88-hot-plug-check/) has a test that isn't in the suite, since it needs msd installed. It runs `mate-settings-daemon` with only its background plugin, on its own bus and dconf database, against Camino on the headless backend with two outputs, unplugs and re-plugs each, and prints each output's `layer_surfaces`. Its head says how to run it.

  | When | headless-1 | headless-2 | headless-1, patched | headless-2, patched |
  | --- | --- | --- | --- | --- |
  | Start | 1 | 1 | 1 | 1 |
  | headless-2 unplugged | 1 | gone | 1 | gone |
  | headless-2 plugged back | 2 | 2 | 1 | 1 |
  | headless-1 unplugged | gone | 2 | gone | 1 |
  | headless-1 plugged back | 0 | 2 | 1 | 1 |

  The first two columns are `mate-settings-daemon-camino 1.29.1-2`, as installed. The last two are the same tag with `0006`, built into a scratch prefix.
- **The fix, `0006-background-Do-not-rebuild-the-windows-from-inside-a-.patch`,** at `pkgrel` 3:
  - While `rebuild_monitor_windows` runs, `on_monitors_changed` returns. Once the rebuild has made its windows it checks that there's one for each monitor, in the display's order, and makes them again if a monitor came or went meanwhile. Every caller redraws after a rebuild, so a change of size alone needs nothing more.
  - The handler makes the same check where it compared the counts, so a monitor replaced by another between two dispatches is a rebuild too.
  - **It has no gate, and isn't a Carried patch:** it's a fix to upstream's own bug, right on any compositor, and a gate would only keep the bug for the other sessions. [ADR 0011](../../../docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-camino.md) is qualified to allow such an upstream fix, as it allows a build patch. **That qualification is this ticket's, and the developer hasn't confirmed it:** the other way is the same patch behind `MATE_WM_NAME=Camino`. In the Wayfire session the plugin makes no window while caja shows the desktop icons, which is its default, so nothing changes there.
  - It's in code new in 1.29.1. It isn't sent upstream yet.
  - **The check compares each window's monitor by its address,** which the window's record keeps without a reference, as upstream's code did. A monitor that goes while another is made at the same address would pass for it. Not seen, and not guarded against.
- **Whether the count grows on a change that isn't a hot-plug:** not from this cause. With the same monitors a `monitors-changed` only redraws, and a rebuild is entered only when a monitor comes or goes, or `show-desktop-icons` changes, which Camino's `0002` takes out. `wlr-randr --off` and `--on` remove and add the output's global, so they are a hot-plug to msd.
- **Not looked into, and with no ticket yet:** when Camino closes the surface of an output that's gone, gtk-layer-shell closes the window from an idle, and the window's `destroy` handler frees its record but leaves it in the list. Here the rebuild always came first, in the same dispatch as the global's removal, and emptied the list. If the idle ever came first, the handler would read a freed record, as it would have before `0006`.

## Resolution

Resolved on 2026-10-06. The cause is msd's: a `monitors-changed` that arrives during the roundtrip of a window's `show` re-entered the rebuild of the background's windows, and both rebuilds' windows stayed. `mate-settings-daemon-camino` `pkgrel` 3 carries the fix as `0006`, with no gate. With it, the headless run with the real msd has one `mate-background` surface on each output after each unplug and re-plug.

- **Seen in the Session** on 2026-10-06, in the developer's Session on `camino-git r220.daef32d-1` with `mate-settings-daemon-camino 1.29.1-3`, in each output's `layer_surfaces` in `DumpState`:

  | When | HDMI-A-1 | DP-1 |
  | --- | --- | --- |
  | Before, both connected | 1 | 1 |
  | DP-1 unplugged | 1 | gone |
  | DP-1 plugged back and HDMI-A-1 unplugged | gone | 1 |
  | HDMI-A-1 plugged back | 1 | 1 |

  DP-1's re-plug wasn't read on its own, with both connected: the next read was after HDMI-A-1's unplug. The saved layout and the primary output were back at the end, and caja's `desktop` followed the primary output to DP-1 and back ([89](89-desktop-icons-stay-when-the-primary-comes-back.md)).
- **No panel in any of the reads,** the first one included: no output had a layer surface of mate-panel's. That's [91](91-mate-panel-dies-when-a-monitor-is-lit.md)'s, not this ticket's.
