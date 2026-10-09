# The screensaver shim drives the lock, idle and inhibitors

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, and the Session run of the checklist item is 136's
Milestone: M7
Blocked by: 126, 128
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The two names [62](62-screensaver-names.md) reserved do what [12](12-screen-locking.md)'s table says, so the panel's *Lock Screen*, mate-power-manager and X11 apps that inhibit over D-Bus work.

- **`org.mate.ScreenSaver`:** `Lock` locks and returns at once; `Unlock` is refused with `AccessDenied`; `SetActive(true)` blanks and starts the `lock-delay` timer, and `SetActive(false)` unblanks and never unlocks; `GetActive` and `GetActiveTime`; `SimulateUserActivity` resets the idle timer and unblanks; `Inhibit`, `UnInhibit` and `GetInhibitors`; `Throttle`, `UnThrottle`, `Cycle` and `ShowMessage` accepted with no effect; `ActiveChanged` on every blank or lock change. No `Quit`.
- **`org.freedesktop.ScreenSaver`,** on both of its paths: `Inhibit` and `UnInhibit`, with the same inhibitor set.
- **The inhibitors** are per sender and dropped when it leaves the bus. They join [128](128-idle-and-blanking.md)'s.

## Acceptance criteria

- [x] Headless tests with a D-Bus client, written before the code, one for each row of 12's table that has an effect: `Lock` locks; `Unlock` is refused and the session stays locked; `SetActive` both ways; `GetActive` and `GetActiveTime` while blanked and while locked; `SimulateUserActivity` unblanks and doesn't unlock; `ActiveChanged` is emitted once per change.
- [x] An inhibitor taken over either name keeps the session from going idle, shows in `GetInhibitors`, and is gone when its sender leaves the bus.
- [ ] The checklist has an item, tagged M7, and it was run in a Session: the panel's *Lock Screen* locks, and `mate-screensaver-command --lock` and `--query` work. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **The cookies** are already kept by 62's shell, which reads none of them.

## Done (2026-10-09)

- **The handler** runs with the whole of `State`, not `Caminito`, as `Lock`, `SetActive` and `SimulateUserActivity` reach `State::request_lock` and idle's timers. `Caminito::dbus_state_requests`, in `dbus`, makes its channel. `SimulateUserActivity` is a thin method now, where 62 answered it on zbus's thread.
- **`Lock`** is `State::request_lock` (126) and returns once the session is locked, before the locker comes. With `disable-lock-screen` it's refused with `AccessDenied`, as 62 refused it with `NotSupported` for the same reason (a caller told the session locked would be wrong the unsafe way), and nothing blanks.
- **`SetActive(true)`** is `IdleState::blank_now`, new here: the blank starts now and the lock delay counts from now. It's asked for, so `idle-activation-enabled` off and an inhibitor don't hold it off. A blank that's on is left as it is.
- **`SetActive(false)`** follows 12's table, which says the blank ends *if not locked*: while locked it does nothing, and only activity shows the lock. This ticket's line says only that it unblanks and never unlocks. Not locked, it's activity, which starts the count again, since a blank ended without it would be due again at once; ext-idle-notify's notifications hear `resumed` too, as with `SimulateUserActivity`.
- **`GetActive`** is the blank or the lock. `ActiveChanged` and `GetActiveTime`'s start are the refresh pass's (`Caminito::refresh_screensaver`), which signals once each time that changes, so a blank over the lock and the lock showing as it ends aren't signalled. The signals go through a channel to one task on zbus's thread, in order, as the Keyboard monitor's do. `GetActiveTime` counts from the refresh pass that saw the change, on Caminito's clock.
- **The inhibitors** join `State::refresh_idle`'s sum through `ScreensaverShim::is_inhibiting`; throttles don't. They go when their caller leaves the bus, as 62 had it, and hold nothing off while locked (12). The debug dump lists each under `idle.inhibitors` as `{ "source": "screensaver", "application": … }`.
- **Test first:** `IdleState::blank_now`'s unit tests and the headless tests in `tests/screensaver.rs` were written first: the unit tests failed to build and thirteen headless tests failed before the code. 62's `the_session_is_never_active` and `lock_is_refused_and_logged` are gone, as M7 changes both. Taking out the shim's inhibitors from idle's sum, and the lock's exception in `SetActive(false)`, each failed their tests.
- **From review:** the checklist's `--activate` step sets `lock-delay` to 1 first, as MATE's default of 0 locks with the blank. `GetActive` reads the blank and the lock as they are, and `GetActiveTime` counts from the refresh pass, so between a change and that pass, in the same dispatch, it can answer 0 for a session that's active. `blank_now` over a lock asks for the lock again at the delay, which `request_lock` takes as a lock already there.
- **The full suite** fails the same 236 tests here as on the base commit, in a container with Ubuntu's Xwayland, which the fixture can't start ("Xwayland starts"), and `caminito-theme`'s goldens fail with its fonts; the rest pass. Its clippy, 1.97, flags the three `manual_range_contains` in `caminito-theme` that 128 found; the rest of the workspace is clean.
- **Not tested:** that the inhibitor of a caller that leaves held idle off before it left, since its leaving can be handled before the refresh pass sees its inhibitor; the other inhibitor tests show it holds.
- **The checklist item** is *The panel's Lock Screen and `mate-screensaver-command`*, tagged M7. It wasn't run: the ticket was done in a container with no display, so it's left to [136](136-packaging-and-closing-m7.md).
