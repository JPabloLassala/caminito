# Screensaver names

Type: implementation
Status: open
Milestone: M5
Blocked by:
Map: [Camino v0.1](../map.md)

## What to build

`dbus::screensaver` ([12](12-screen-locking.md), [15](15-crate-and-module-architecture.md)) owning `org.mate.ScreenSaver` and `org.freedesktop.ScreenSaver` before mate-session starts, so mate-screensaver's daemon finds the name taken, exits, and never draws an X11 lock over a session it can't lock. This is the Screensaver shim's shell; M7 gives it the lock, idle and inhibit state ([17](17-milestone-slicing.md)).

- **Names:** both are requested with `Names::WellKnown`, without replacing an existing owner and without allowing replacement. Under `Names::UniqueOnly` (tests) the interfaces are served at the unique name.
- **A name that's taken** is logged as an error with its owner, and Camino carries on: a nested run on a bus that already has a screensaver must still start.
- **Ready:** the `dbus` module reports when its names are owned (or refused), which [61](61-session-start-up-and-exit.md) waits for before spawning mate-session.
- **The interfaces,** as mate-screensaver and the freedesktop spec define them, so that callers in the session get answers, not errors that break them:
  - `Lock`: logged and refused with a D-Bus error. The session can't lock until M7.
  - `GetActive` answers false, `GetActiveTime` 0, and `SetActive` is logged and does nothing.
  - `Inhibit` and `UnInhibit` (both names), and `Throttle` and `UnThrottle`: cookies are handed out and taken back, kept per caller, and dropped when the caller leaves the bus. Nothing reads them until M7's idle.
  - `SimulateUserActivity` does nothing. `Cycle` and `ShowMessage` do nothing.
- **Thin methods (15):** each sends a request into the loop and awaits the reply, as `dbus::debug` does.

## Acceptance criteria

- [ ] Headless tests with the D-Bus test client, written before the code: `Lock` returns an error and is logged; `GetActive` is false; `Inhibit` returns distinct cookies and `UnInhibit` takes one back; a caller that drops off the bus loses its cookies.
- [ ] A test that owns `org.mate.ScreenSaver` first, then starts Camino with `Names::WellKnown`: Camino logs the conflict and reports ready.
- [ ] Nested on a private bus, `mate-screensaver` started by hand exits at once, and `mate-screensaver-command --lock` reports the refusal.

## Notes

- **The interface's method list** is read from mate-screensaver's `gs-listener-dbus.c` at the version Arch ships (1.28.0, 16) when the ticket starts; the list above is from 12 and the freedesktop spec, and may be short.
- **Why refuse `Lock` rather than accept it:** a caller that believes the session locked would be wrong in the unsafe direction (12's fail-closed rule, read backwards).
