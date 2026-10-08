# The screensaver shim drives the lock, idle and inhibitors

Epic: caminito-v0.1
Type: implementation
Status: open
Milestone: M7
Blocked by: 126, 128
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The two names [62](62-screensaver-names.md) reserved do what [12](12-screen-locking.md)'s table says, so the panel's *Lock Screen*, mate-power-manager and X11 apps that inhibit over D-Bus work.

- **`org.mate.ScreenSaver`:** `Lock` locks and returns at once; `Unlock` is refused with `AccessDenied`; `SetActive(true)` blanks and starts the `lock-delay` timer, and `SetActive(false)` unblanks and never unlocks; `GetActive` and `GetActiveTime`; `SimulateUserActivity` resets the idle timer and unblanks; `Inhibit`, `UnInhibit` and `GetInhibitors`; `Throttle`, `UnThrottle`, `Cycle` and `ShowMessage` accepted with no effect; `ActiveChanged` on every blank or lock change. No `Quit`.
- **`org.freedesktop.ScreenSaver`,** on both of its paths: `Inhibit` and `UnInhibit`, with the same inhibitor set.
- **The inhibitors** are per sender and dropped when it leaves the bus. They join [128](128-idle-and-blanking.md)'s.

## Acceptance criteria

- [ ] Headless tests with a D-Bus client, written before the code, one for each row of 12's table that has an effect: `Lock` locks; `Unlock` is refused and the session stays locked; `SetActive` both ways; `GetActive` and `GetActiveTime` while blanked and while locked; `SimulateUserActivity` unblanks and doesn't unlock; `ActiveChanged` is emitted once per change.
- [ ] An inhibitor taken over either name keeps the session from going idle, shows in `GetInhibitors`, and is gone when its sender leaves the bus.
- [ ] The checklist has an item, tagged M7, and it was run in a Session: the panel's *Lock Screen* locks, and `mate-screensaver-command --lock` and `--query` work. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **The cookies** are already kept by 62's shell, which reads none of them.
