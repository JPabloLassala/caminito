# The desktop icons don't come back with the primary output

Type: implementation
Status: resolved
Milestone: M6
Blocked by: none
Map: [Caminito v0.1](../map.md)

## What to build

When the primary output is unplugged and plugged back, the panels come back to it and caja's desktop doesn't. The checklist item *The primary output moves the panel, the desktop icons and RandR's primary* expects both to come back with the monitor ([81](81-primary-output.md)). [60](60-tty-checks-left-over.md) saw it on 2026-10-06, in the developer's Session on `caminito-git r195.9e4baac-1` with `caja-caminito 1.28.0-1` and `mate-panel-caminito 1.28.7.r111.g58ac6c1-3`. The primary output was HDMI-A-1, the LG, chosen in the saved layout ([83](83-saved-layouts.md)).

- **Unplugging went as expected:** 5 s after HDMI-A-1 was unplugged, DP-1 was alone at 0,0 with both panels and the `desktop` layer surface on it.
- **Plugging back didn't:** 5 s after, `primary_output` was HDMI-A-1 again, at 0,0, with both panels. The `desktop` layer surface was still on DP-1, at 2194,0, and 2194x1234, which is HDMI-A-1's logical size and not DP-1's 1920x1080. It was the same 10 minutes later.
- **Not known:** whether caja heard of the primary output again. The restore of a saved layout makes the primary output a choice once more (83), after the output is added; whether `PrimaryOutput`'s change is signalled then, and whether caja's carried patch follows a name that is the same as before the unplug, are the first things to look at. The size says caja did resize the surface for HDMI-A-1 and didn't move it.

## Acceptance criteria

- [x] The cause is written down in this ticket's notes, with a headless test written before the fix where the cause is Caminito's. (The cause is in `caja-caminito`'s patch, so there's no headless test to keep.)
- [x] After the primary output is unplugged and plugged back, the `desktop` layer surface is on it, at its size, as the panels are. (Seen with the real caja on the headless Caminito, and on the two monitors on 2026-10-06, with no panel running to compare: see the resolution.)

## Notes

- **The cause is `caja-caminito`'s patch (81), not Caminito.** The patch followed the monitors by GDK's `monitor-added`. GDK 3.24 on Wayland emits that as soon as it binds the `wl_output`, before any of the output's events, so the monitor has no connector yet and can't be matched against `PrimaryOutput` (`_gdk_wayland_screen_add_output` in `gdk/wayland/gdkscreen-wayland.c`, read at 3.24.52, the version installed). The connector is set later, in `apply_monitor_change`, which emits the screen's `monitors-changed` and `size-changed` and nothing on the display.
- **So at the re-plug** `monitor-added` came while the new monitor had no name, and so did Caminito's `PropertiesChanged`, which races the output's events on another socket and is sent as soon as the output is added. The patch left the surface where it was. Then `size-changed` ran caja's own size handler, which the patch had made ask for the primary output's monitor: by then it had its name, so the desktop took HDMI-A-1's size, on DP-1. That handler sets the size and never the output.
- **The two things the ticket left to look at:** Caminito does signal `PrimaryOutput` when the choice returns, restored from a saved layout or not: the stand-in's name was the last one signalled, so the name differs (`unplugging_the_primary_output_puts_the_stand_in_in_and_plugging_it_back_returns_the_choice` in `caminito/src/tests/primary_output.rs` listens for it). And the patch follows a name that's the same as before the unplug, since it compares nothing with what it had before.
- **Why 81's own check passed,** and the unplug here: a choice made with both monitors on, and a stand-in going in, name a monitor GDK already has.
- **The panel** isn't in this: mate-panel's patch came back to HDMI-A-1 in the same run.
- **Reproduced before the fix (2026-10-06)** with a throwaway test, not kept: the headless Caminito with the AOC and the LG plugged in, the LG chosen, `org.mate.Caminito` owned on the test's bus, and the installed `/usr/bin/caja --force-desktop` (`caja-caminito 1.28.0-1`) on Caminito's socket. Unplugging the LG put `desktop` on `headless-1` at 1920x1080. Plugging it back left it on `headless-1`, at 0,0 and 2259x1271, the LG's logical size, with nothing on `headless-2`: what 60 saw.

## Resolution

Resolved on 2026-10-06, in `caja-caminito`, whose `pkgrel` is 2.

- **The fix:** the patch follows the screen's `monitors-changed` in place of the display's `monitor-added` and `monitor-removed`. It comes each time GDK applies what a monitor's events said, its connector among it, and when one is removed. It also comes when a monitor's mode or scale changes, so the desktop's size follows those through the same function.
- **Seen:** the same throwaway test with the caja built from the patched tree had `desktop` on `headless-2` at 1920,0 and 2259x1271 after the re-plug, and nothing on `headless-1`, in two runs. `caja-caminito 1.28.0-2` was then built with `makepkg` from this patch. It wasn't installed, and wasn't run in a Session.
- **Seen on the developer's machine (2026-10-06),** in the Session on `caminito-git r220.daef32d-1` with `caja-caminito 1.28.0-2`, the caja started after the upgrade. The primary output was HDMI-A-1, the LG, at 0,0 and 2194x1234, with `desktop` on it. Its cable was unplugged for 11 s:
  - 5 s after the unplug, `primary_output` was DP-1, alone at 0,0, with `desktop` on it at 1920x1080.
  - 5 s after the re-plug, `primary_output` was HDMI-A-1 again, at 0,0, with `desktop` on it at 2194x1234 and none on DP-1, at 2194,0. It was the same 30 s after, with the same caja running and no new core dump.
- **Not seen:** the panels beside it. mate-panel wasn't running, having died earlier in that Session ([91](91-mate-panel-dies-when-a-monitor-is-lit.md)), so step 4 of the checklist's *The primary output moves the panel, the desktop icons and RandR's primary* has its desktop half seen and its panel half waiting on 91.
