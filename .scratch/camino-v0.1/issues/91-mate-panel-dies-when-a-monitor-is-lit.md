# mate-panel dies when a monitor is lit

Type: implementation
Status: open
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

- [ ] The cause is named: the event, and whose fault its handling is.
- [ ] On the developer's machine, `.scratch/camino-v0.1/issues/90-off-on.sh` leaves the mate-panel that was running, running, and `coredumpctl list mate-panel` has no new line.
- [ ] If Camino changes, a headless test written before the change. If a package is patched, ADR 0011's list has it.
