# Xwayland's display held across a restart

Type: implementation
Status: open
Milestone: M4
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

Found by [43](43-packaging-and-closing-m3.md): the Packaging job's `check()` failed twice on `tests::xwayland_restart::a_restarted_xwayland_is_owed_the_wayland_clipboard` with `the condition didn't hold within 5 s` ([run 36735998882](https://github.com/JPabloLassala/camino/actions/runs/36735998882), both attempts). The cause is in Camino, not only in the tests: between an Xwayland exit and its restart, Camino holds nothing on the display, so another X server can take it.

When Xwayland exits, `stop_xwayland` drops Smithay's `XWayland` source, and with it the display's lock and listening sockets. The restart spawns again on the same number after the restart delay (42), and `XWayland::spawn` grabs the lock afresh. If another X server started in that gap and took the lowest free display, the grab fails with `Could not find a free socket for the XServer`. Each failure counts as another exit, so the delay doubles, and after `give_up_after` Camino runs without X while `DISPLAY` names someone else's server. In a session that's another nested Camino, an Xvfb or a second login taking the number. In the tests it's the other X11-tier tests, which start Xwaylands in parallel threads of the same process.

Keep the display across the restart:

- **Held from the first ready until Camino stops running X:** the lock file, and the listening sockets if Smithay allows it, so no other server can take the number while Xwayland is down. The restart hands them to the new Xwayland.
- **Smithay:** `X11Lock` and `prepare_x11_sockets` are crate-private, and `XWayland::spawn` always grabs its own. This needs an upstream API, such as a spawn that takes the lock and sockets from the last one, or an `XWayland` that returns them when it exits. Camino's Smithay is upstream, pinned by revision, with no fork.
- **Given up:** when Camino gives up on X, the display is released.

## Acceptance criteria

- [ ] An X11-tier test, written before the fix: while Xwayland is down for its restart delay, a lock taken on its display, as another server would, fails, and the restart comes back on the same display.
- [ ] Two fixtures, one restarting and one starting Xwayland in the gap, each end up on their own display, with no restart failing.
- [ ] Pinned to two cores (`taskset -c 0-1`), the `camino` lib suite passes in release ten runs in a row.
- [ ] CI and the Packaging job are green.

## Notes

- **Diagnosis (2026-09-30):**
  - **Reproduction:** pinned to two cores, the lib suite failed in release in about half the runs, always in a restart. On four cores and more it passed. The test alone passed 40 out of 40 runs.
  - **Which wait:** `dispatch_until` and the fixture helpers that wait through it now carry `#[track_caller]`, so a timeout reports the test's line. It was `f.wait_for_xwayland()` (`xwayland_restart.rs:152`), and once the give-up test's `crash_xwayland_starting` (`:124`).
  - **Timing:** Xwayland was ready within 100 ms of each spawn. The wait was before the spawn, at 0.5 s, 3.5 s or 7.5 s. That's the restart backoff after one, three or four exits, 0.5 + 1 + 2 + 4.
  - **The cause:** a print in `xwayland_exited` showed the extra exits were `can't start Xwayland: Could not find a free socket for the XServer`, from the respawn on the same display while a parallel test's Xwayland held it.
- **Not a fix:** a longer deadline in `dispatch_until`. It would hide the gap in tests and leave it in sessions.
- **Workaround in the tests (2026-09-30), so M3 could close:** a process-wide `RwLock` in the fixture, `X_DISPLAYS`. Every fixture that starts Xwayland holds it shared (`start_xwayland`, or `hold_x_displays` for the one test that starts it on `state.camino`), and a test that crashes Xwayland holds it exclusively from before the start (`restarts_xwayland`), which `crash_xwayland` asserts. So no test's Xwayland can take a restarting test's display. Pinned to two cores, the lib suite then passed in release ten runs in a row, at the same 17.5 s. The gap is still there in sessions. This ticket stays open for the fix, and the lock goes when it lands.
