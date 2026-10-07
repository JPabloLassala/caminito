# Xwayland's display held across a restart

Type: implementation
Status: resolved
Milestone: before v0.1 (gates no milestone)
Blocked by: none
Map: [Caminito v0.1](../map.md)

## What to build

Found by [43](43-packaging-and-closing-m3.md): the Packaging job's `check()` failed twice on `tests::xwayland_restart::a_restarted_xwayland_is_owed_the_wayland_clipboard` with `the condition didn't hold within 5 s` ([run 36735998882](https://github.com/JPabloLassala/caminito/actions/runs/36735998882), both attempts). The cause is in Caminito, not only in the tests: between an Xwayland exit and its restart, Caminito holds nothing on the display, so another X server can take it.

When Xwayland exits, `stop_xwayland` drops Smithay's `XWayland` source, and with it the display's lock and listening sockets. The restart spawns again on the same number after the restart delay (42), and `XWayland::spawn` grabs the lock afresh. If another X server started in that gap and took the lowest free display, the grab fails with `Could not find a free socket for the XServer`. Each failure counts as another exit, so the delay doubles, and after `give_up_after` Caminito runs without X while `DISPLAY` names someone else's server. In a session that's another nested Caminito, an Xvfb or a second login taking the number. In the tests it's the other X11-tier tests, which start Xwaylands in parallel threads of the same process.

Keep the display across the restart:

- **Held from the first ready until Caminito stops running X:** the lock file, and the listening sockets if Smithay allows it, so no other server can take the number while Xwayland is down. The restart hands them to the new Xwayland.
- **Smithay:** `X11Lock` and `prepare_x11_sockets` are crate-private, and `XWayland::spawn` always grabs its own. This needs an upstream API, such as a spawn that takes the lock and sockets from the last one, or an `XWayland` that returns them when it exits. Caminito's Smithay is upstream, pinned by revision, with no fork.
- **Given up:** when Caminito gives up on X, the display is released.

## Acceptance criteria

- [x] An X11-tier test, written before the fix: while Xwayland is down for its restart delay, a lock taken on its display, as another server would, fails, and the restart comes back on the same display.
- [x] Two fixtures, one restarting and one starting Xwayland in the gap, each end up on their own display, with no restart failing.
- [x] Pinned to two cores (`taskset -c 0-1`), the `caminito` lib suite passes in release ten runs in a row.
- [x] CI and the Packaging job are green.

## Notes

- **Diagnosis (2026-09-30):**
  - **Reproduction:** pinned to two cores, the lib suite failed in release in about half the runs, always in a restart. On four cores and more it passed. The test alone passed 40 out of 40 runs.
  - **Which wait:** `dispatch_until` and the fixture helpers that wait through it now carry `#[track_caller]`, so a timeout reports the test's line. It was `f.wait_for_xwayland()` (`xwayland_restart.rs:152`), and once the give-up test's `crash_xwayland_starting` (`:124`).
  - **Timing:** Xwayland was ready within 100 ms of each spawn. The wait was before the spawn, at 0.5 s, 3.5 s or 7.5 s. That's the restart backoff after one, three or four exits, 0.5 + 1 + 2 + 4.
  - **The cause:** a print in `xwayland_exited` showed the extra exits were `can't start Xwayland: Could not find a free socket for the XServer`, from the respawn on the same display while a parallel test's Xwayland held it.
- **Not a fix:** a longer deadline in `dispatch_until`. It would hide the gap in tests and leave it in sessions.
- **Workaround in the tests (2026-09-30), so M3 could close:** a process-wide `RwLock` in the fixture, `X_DISPLAYS`. Every fixture that starts Xwayland holds it shared (`start_xwayland`, or `hold_x_displays` for the one test that starts it on `state.caminito`), and a test that crashes Xwayland holds it exclusively from before the start (`restarts_xwayland`), which `crash_xwayland` asserts. So no test's Xwayland can take a restarting test's display. Pinned to two cores, the lib suite then passed in release ten runs in a row, at the same 17.5 s. The gap is still there in sessions. This ticket stays open for the fix, and the lock goes when it lands. It gates no milestone, only v0.1 (decided 2026-09-30).
- **Decided (2026-09-30): Caminito holds the lock itself, with no Smithay change.** Once the restart is scheduled, `Restart` takes `/tmp/.X<n>-lock` as X servers do, with Caminito's pid in X's format, and every X server leaves a lock naming a live process alone. It's taken in an idle at the end of the dispatch that saw the exit, since an exit reported from the `XWayland` source's own callback (`XWaylandEvent::Error`) leaves the source, and Smithay's `X11Lock`, alive until the callback returns. `start_xwayland` lets it go just before `XWayland::spawn` grabs it again. Another server could still take the display in two short gaps: from the exit to the end of that dispatch, and between the release and the spawn. It can't in the restart's delay. The listening sockets aren't held, since the spawn removes and binds its own. After giving up, or if the restart can't be scheduled, nothing is held: the lock is let go before every spawn and taken only once a respawn is scheduled. If the lock can't be taken (another server got it, or a stale lock is in the way, which Caminito doesn't reclaim but the spawn does), Caminito logs one `warn!` and the restart goes on as before. Closing the gaps, and handing the sockets over, still needs the upstream API above. If it lands on the pinned master, Caminito uses it.
- **Tests** (`tests/xwayland_restart.rs`): `xwayland_s_display_stays_caminito_s_while_xwayland_is_down_for_a_restart` takes the lock as another server would while Xwayland is down, and failed without the fix (`no other X server can take :5`). `an_xwayland_started_while_another_restarts_takes_a_display_of_its_own` gives up after one failed restart; without the fix, run alone, both fixtures got `:2`. In the full suite it can only catch the gap when the restarting display is the lowest free one. The fixture's `X_DISPLAYS` lock is gone, and so are `restarts_xwayland` and `hold_x_displays`. Pinned to two cores, the lib suite passed in release ten runs in a row, at 15 s.
- **Resolved (2026-10-04):** CI ([run 37177692100](https://github.com/JPabloLassala/caminito/actions/runs/37177692100)) and the Packaging job ([run 37177692118](https://github.com/JPabloLassala/caminito/actions/runs/37177692118)) are green on 3380aec, which has the fix (9f31716). The two short gaps stay: at the pinned revision (928d4a9), Smithay's `X11Lock` and `prepare_x11_sockets` are still crate-private.
