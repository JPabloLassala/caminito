# Screensaver names

Type: implementation
Status: resolved
Milestone: M5
Blocked by:
Map: [Caminito v0.1](../map.md)

## What to build

`dbus::screensaver` ([12](12-screen-locking.md), [15](15-crate-and-module-architecture.md)) owning `org.mate.ScreenSaver` and `org.freedesktop.ScreenSaver` before mate-session starts, so mate-screensaver's daemon finds the name taken, exits, and never draws an X11 lock over a session it can't lock. This is the Screensaver shim's shell; M7 gives it the lock, idle and inhibit state ([17](17-milestone-slicing.md)).

- **Names:** both are requested with `Names::WellKnown`, without replacing an existing owner and without allowing replacement. Under `Names::UniqueOnly` (tests) the interfaces are served at the unique name.
- **A name that's taken** is logged as an error with its owner, and Caminito carries on: a nested run on a bus that already has a screensaver must still start.
- **Ready:** the `dbus` module reports when its names are owned (or refused), which [61](61-session-start-up-and-exit.md) waits for before spawning mate-session.
- **The interfaces,** as mate-screensaver and the freedesktop spec define them, so that callers in the session get answers, not errors that break them:
  - `Lock`: logged and refused with a D-Bus error. The session can't lock until M7.
  - `GetActive` answers false, `GetActiveTime` 0, and `SetActive` is logged and does nothing.
  - `Inhibit` and `UnInhibit` (both names), and `Throttle` and `UnThrottle`: cookies are handed out and taken back, kept per caller, and dropped when the caller leaves the bus. Nothing reads them until M7's idle.
  - `SimulateUserActivity` does nothing. `Cycle` and `ShowMessage` do nothing.
- **Thin methods (15):** each sends a request into the loop and awaits the reply, as `dbus::debug` does.

## Acceptance criteria

- [x] Headless tests with the D-Bus test client, written before the code: `Lock` returns an error and is logged; `GetActive` is false; `Inhibit` returns distinct cookies and `UnInhibit` takes one back; a caller that drops off the bus loses its cookies.
- [x] A test that owns `org.mate.ScreenSaver` first, then starts Caminito with `Names::WellKnown`: Caminito logs the conflict and reports ready.
- [ ] Nested on a private bus, `mate-screensaver` started by hand exits at once, and `mate-screensaver-command --lock` reports the refusal.

## Notes

- **The interface's method list** is read from mate-screensaver's `gs-listener-dbus.c` at the version Arch ships (1.28.0, 16) when the ticket starts; the list above is from 12 and the freedesktop spec, and may be short.
- **Why refuse `Lock` rather than accept it:** a caller that believes the session locked would be wrong in the unsafe direction (12's fail-closed rule, read backwards).
- **The method list, read from `gs-listener-dbus.c:1373-1417` at 1.28.0,** is the list above plus `Unlock` and `GetInhibitors`, and the `ActiveChanged` signal. `Unlock` is refused with `AccessDenied` (12). `GetInhibitors` gives mate-screensaver's descriptions, `Application="…"; Since="…"; Reason="…";`. The signal is declared and never sent. `Quit` isn't provided (12).
- **`org.freedesktop.ScreenSaver` isn't mate-screensaver's.** 12 says the name is owned "as mate-screensaver does", but 1.28.0 owns `org.mate.ScreenSaver` only. Caminito owns both anyway, for the applications that inhibit idle there. It's served at `/org/freedesktop/ScreenSaver` and `/ScreenSaver`, with `Inhibit` and `UnInhibit` only, and shares the one set of cookies.
- **Names:** `RequestName` with `DO_NOT_QUEUE` only, after the interfaces are served. A name that's taken is logged with `error!`, with its owner's unique name. That's the ticket's word, where CODING_STANDARDS.md keeps `error!` for what stops Caminito.
- **Ready is `Caminito::dbus_ready`.** `start_dbus` asks for the names while it connects, on the main thread, so it's true as soon as `start_dbus` returns `Ok`. If `start_dbus` fails (no bus, or `org.mate.Caminito.Debug` taken, which still fails the connection), it's never true, and there are no names to wait for: [61](61-session-start-up-and-exit.md) must not wait then.
- **Errors:** `Lock` gets `org.freedesktop.DBus.Error.NotSupported`, and is logged with the caller's unique name.
- **Cookies** are one counter for inhibitors and throttles, never 0. A cookie is taken back only by the caller that got it, on the method of its kind; anything else is ignored without an error, as mate-screensaver ignores an unknown cookie. mate-screensaver takes back any caller's (a FIXME at `gs-listener-dbus.c:1030`).
- **A caller that leaves** is seen through `NameOwnerChanged`, watched by a task on zbus's thread. That task isn't in order with the method calls, so after each `Inhibit` or `Throttle` the method checks that the caller still has its name, and drops its cookies if not. This costs one call to the bus for each cookie. No test covers that order; the headless test covers the plain case.
- **`Cycle`, `SimulateUserActivity` and `ShowMessage`** are answered on zbus's thread and send nothing into the loop, since there's nothing for the loop to do. The rest are thin methods. `dbus::debug` and `dbus::screensaver` share `dbus::ask`.
- **Throttles** can't be seen from outside: no method lists them. The tests only show that their cookies are distinct and aren't inhibitors.
- **Tests:** `tests/screensaver.rs`, with proxies for both interfaces in `caminito-test-client::dbus`. `Logs::capture` in the fixture reads what the event loop logs. The tests share one bus, so `Fixture::reserve_names` lets one fixture at a time have the well-known names, and waits for the bus to free the last one's. `caminito_owns_its_debug_name` uses it too.
- **Nested, checked in the virtual machine** on a private bus with mate-screensaver 1.28.0-3 (installed in the guest for this): `mate-screensaver --no-daemon` logs "screensaver already running in this session" and exits with status 1 at once. `mate-screensaver-command --query` says "The screensaver is inactive". `--inhibit` shows in `GetInhibitors`, and is gone when the command is killed.
- **The third criterion is half met, and is unticked.** `mate-screensaver-command --lock` can't report the refusal: it sends `Lock` and expects no reply (`mate-screensaver-command.c:430`), so it prints nothing and exits with 0, as it does for `--unlock`, `--cycle` and `--poke`. The refusal is in Caminito's log ("can't lock the session for :1.11: the lock comes with M7"), and `gdbus call … org.mate.ScreenSaver.Lock` gets the `NotSupported` error. Resolved on that basis on 2026-10-04.
