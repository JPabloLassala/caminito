# logind: Lock, Unlock, the locked hint and the sleep delay

Epic: caminito-v0.1
Type: implementation
Status: open
Milestone: M7
Blocked by: 126
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`dbus::logind`, on the system bus ([12](12-screen-locking.md)'s *Robustness*).

- **`Lock` and `Unlock`** of Caminito's own session. `Lock` is a lock request. `Unlock` ends the lock and stops the locker, from the give-up state too: it's the text-console recovery.
- **`SetLockedHint`** on every lock and unlock.
- **The sleep delay:** Caminito holds a `sleep` delay inhibitor. On `PrepareForSleep(true)` it releases it once a pending lock has its lock surfaces rendered on every output, or after 2 s, and takes it again on resume.
- **Lock on suspend** stays mate-power-manager's key: it calls `Lock` on the shim ([127](127-screensaver-shim.md)).

## Acceptance criteria

- [ ] Headless tests with a stand-in logind on a private bus, written before the code:
  - its `Lock` signal locks, and `Unlock` unlocks and stops the locker;
  - `Unlock` in the give-up state unlocks;
  - `SetLockedHint` is called with each change;
  - on `PrepareForSleep(true)` with a lock pending the inhibitor is released after the lock surfaces are rendered, and after 2 s with a locker that never draws; it's taken again on `PrepareForSleep(false)`.
- [ ] Nested, with no logind session of its own, Caminito logs once and locks as before.
- [ ] The checklist has *The suspend race with a slow locker* and *`loginctl unlock-session` from a console*, tagged M7 (17), and they were run in a Session: the lock screen is what shows on resume, never the desktop, and `loginctl lock-session` and `unlock-session` work. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **Any process of the user can unlock** through logind without authentication, as on X11 with mate-screensaver. 12 accepts it for a single-user session.
