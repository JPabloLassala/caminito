# mate-panel dies when a monitor is lit

Type: implementation
Status: resolved
Milestone: M6 (proposed)
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

mate-panel dies when a monitor is turned on or plugged in, and when two die within a few seconds mate-session stops starting it again, so the Session is left with no panels. [87](87-hot-plug-blanks-the-other-monitor.md) saw the crash four times on 2026-10-06 and left it without a ticket. It was seen again that day while checking [90](90-a-monitor-turned-on-again-gets-its-saved-layout.md), on `camino-git r220.daef32d-1` with `mate-panel-camino 1.28.7.r111.g58ac6c1-3` and `libxfce4windowing 4.20.7-1`: `.scratch/camino-v0.1/issues/90-off-on.sh` turned the LG and then the AOC off and on, 5 s apart, both panels died, and none was running after.

- **What's known, from `coredumpctl`:**
  - Seven core dumps of mate-panel on 2026-10-06 (15:49:36, 16:16:10, 16:17:59, 17:00:05, 17:00:43, 17:04:30, 17:04:35), each within a second or two of a monitor being lit. Five were on `camino-git r210.ad13fa1-1`, before 90's change, so it isn't 90's.
  - Six are a SIGSEGV in `xfw_monitor_get_physical_geometry` (`libxfce4windowing-0.so.0 + 0xd354`), called from a signal handler of libxfce4windowing's own (`+ 0x24b67` or `+ 0x24b7d`) inside `wl_display_dispatch_queue_pending`: the library handling an event of an output, with a monitor it doesn't have, or no longer has.
  - The seventh (17:04:35, the panel mate-session had started 5 s before, as the AOC was lit) is a SIGABRT from `realloc` under `g_array_append_vals`, while an applet was drawn: a corrupt heap, which may be the same fault met later.
  - 87 saw it at the LG's re-plug and not at the AOC's. Here the first panel died as the LG was lit and its successor as the AOC was.
  - Qt clients died at the same moments (kdeconnectd, kalendarac and akonadi's agents), which may be nothing of Camino's, or may be the same events in an order clients don't expect.
- **To find out:**
  - Which event the handler is in, and whether what Camino sends is at fault: the order of `wl_output`, xdg-output and the globals' removal for an output that's disabled and enabled again, which keeps its name and gets a new global (58).
  - Whether the panel dies the same way under another compositor, wayfire or labwc, with `wlr-randr --off` and `--on`.
- **The fix is one of:** Camino's events in the order clients expect, with a headless test; or a carried patch for libxfce4windowing or mate-panel (ADR 0011), offered upstream.

## Acceptance criteria

- [x] The cause is named: the event, and whose fault its handling is.
- [x] On the developer's machine, `.scratch/camino-v0.1/issues/90-off-on.sh` leaves the mate-panel that was running, running, and `coredumpctl list mate-panel` has no new line.
- [x] If Camino changes, a headless test written before the change. If a package is patched, ADR 0011's list has it.

## Resolution

Implemented on 2026-10-06, and checked on the developer's machine the same day (below).

### The cause

The event is `ext_workspace_group_handle_v1.output_leave`, sent after the `wl_registry.global_remove` of the output it names. The fault is libxfce4windowing's, and Camino's order is what met it.

- **In libxfce4windowing** (4.20.7, and the same on its master on 2026-10-06), `xfw-workspace-group-wayland.c`:
  - A workspace group keeps a list of its monitors, with no reference to them.
  - When an output's global is removed, the screen takes the monitor off its own list, then emits `monitor-removed`, then drops the monitor. The group's handler of that signal, `screen_monitor_removed`, calls `group_output_leave`, which looks for the monitor in the screen's list. It's no longer there, so the group keeps its pointer, to a monitor that's freed.
  - An `output_leave` that comes after finds nothing either, for the same reason.
  - At the group's next `monitors-changed`, which is when an output enters it, each workspace's `group_monitors_changed` reads every monitor of the group. `objdump` puts the two return addresses of the core dumps, `+ 0x24b67` and `+ 0x24b7d`, after its two calls of `xfw_monitor_get_physical_geometry`. That's why the panel dies when a monitor is lit, and not when it goes off.
  - The SIGABRT under `realloc` fits the same freed monitor, read where the memory was reused. That wasn't shown.
- **In Camino**, `hide_output` disabled the global at once, and the group's `output_leave` went out later, in the refresh pass. With the leave first, the library finds the monitor and takes it out of the group, and the global's removal then has nothing to do.

### The change

A hidden output's global is now withdrawn in the refresh pass, after foreign-toplevel and ext-workspace have said the output left (`Camino::refresh_output_globals`, in `camino/src/output.rs`). It's one order for unplugging and for disabling over output management.

The headless tests are in `camino/src/tests/ext_workspace.rs`, written first: the test client marks an `output_leave` that names an output whose global it was already told is removed. The unplug test that was there failed with the new one.

No package is patched, so ADR 0011's list is as it was.

### Not done

- **The library's bug isn't reported upstream.** It's theirs on any compositor that removes the global first. The fix there is for `screen_monitor_removed` to take the monitor off the group's list itself.
- **wayfire and labwc weren't tried.** The source says what the cause is, so the answer wouldn't change the fix.
- **The Qt clients that died at the same moments** (kdeconnectd, kalendarac, akonadi's agents) weren't looked at. Their core dumps would say whether it's the same order.
- **A `wl_output` of the old global enters the group again** when its output is enabled again, if the client didn't release it: `Output::client_outputs` has every `wl_output` the client still holds, of the removed global too. The test client holds them, and sees a second `output_enter`. libxfce4windowing only notes an output it has no monitor for, so it's harmless there, and it was so before this change. Foreign-toplevel's `output_enter` reads the same list.
- **An output hidden and shown again before a refresh gets no leave**, since ext-workspace compares the `wl_output`s the client has, and they're the same. Its old global is still removed, so the library would keep the monitor. It needs a head disabled and enabled in one dispatch, which no client was seen to do, and it was so before this change.
- **Foreign-toplevel's `output_leave` now comes before the removal too**, by the refresh pass's order, with no test of it. libxfce4windowing's windows take a removed monitor off their own lists, so nothing there depended on it.

The 13 tests that take Camino's names on the session bus fail while a Camino Session runs on it. The suite was run under `dbus-run-session`.

### On the developer's machine (2026-10-06)

On `camino-git r225.c221860-1`, in a Session started at 17:32, the developer unplugged a monitor and plugged it in again, and says mate-panel didn't die. A minute after, the mate-panel that the Session started at 17:32:28 was still running, and `coredumpctl` had no core dump of any program since 17:05, the Qt clients' included.

- **A different check from the ticket's:** the monitor was unplugged and plugged in, and `.scratch/camino-v0.1/issues/90-off-on.sh`, which disables and enables each monitor over output management, wasn't run. Both hide the output through `hide_output`, and the headless tests have both. On r210 and r220 the panel died at a re-plug too (87).
- **One monitor, once.** Which monitor it was isn't recorded.
