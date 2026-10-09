# logind: Lock, Unlock, the locked hint and the sleep delay

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, and the checklist's two items, in a Session, are 136's
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

- [x] Headless tests with a stand-in logind on a private bus, written before the code:
  - its `Lock` signal locks, and `Unlock` unlocks and stops the locker;
  - `Unlock` in the give-up state unlocks;
  - `SetLockedHint` is called with each change;
  - on `PrepareForSleep(true)` with a lock pending the inhibitor is released after the lock surfaces are rendered, and after 2 s with a locker that never draws; it's taken again on `PrepareForSleep(false)`.
- [x] Nested, with no logind session of its own, Caminito logs once and locks as before.
- [ ] The checklist has *The suspend race with a slow locker* and *`loginctl unlock-session` from a console*, tagged M7 (17), and they were run in a Session: the lock screen is what shows on resume, never the desktop, and `loginctl lock-session` and `unlock-session` work. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **Any process of the user can unlock** through logind without authentication, as on X11 with mate-screensaver. 12 accepts it for a single-user session.

## Done (2026-10-09)

- **`dbus::logind`** connects to the system bus on the tty only. A nested run logs one `info` line and doesn't connect: with the host's `XDG_SESSION_ID` inherited it would find the host's session, follow its locks and set its locked hint. On the tty, a bus or a session that can't be found logs one `warn` line, and the session locks as before. The headless test of that is a stand-in that knows no session: one line names logind, and a lock request still spawns the locker.
- **The session** is `XDG_SESSION_ID`'s (`GetSession`), as the give-up message's ID is, or else `GetSessionByPID` of Caminito's process.
- **A task on the connection's executor** finds the session, follows `Lock`, `Unlock` and `PrepareForSleep`, takes the inhibitor and sets the hints in order. It logs nothing; the event loop gets the signals, the inhibitor's fd and the failures over a calloop channel. Connecting is the one wait on the main thread, as for the session bus.
- **`Lock`** calls `State::request_lock`, so `disable-lock-screen` refuses it too. The seam lost its `expect(dead_code)`.
- **`Unlock`** is `Caminito::end_lock`: `LockState::ended` takes every state to `unlocked`, a locker that holds the lock hears `finished`, and `Locker::end` finishes the locker's helper and sends it SIGTERM (`Helper::terminate`), still watched, so it's reaped. From the give-up state it's what takes the locker out of it, as 126 asked.
- **Smithay keeps an ended lock as the one that holds it** until another is told `locked`, and its locker's `unlock_and_destroy` still reaches the handler. The handler now unlocks only a lock that's `locked` and confirmed, so that locker can't end a lock asked for since. Its `destroy` after `finished` is a protocol error in Smithay, which ends a locker that's going anyway.
- **The locked hint** is set from the refresh pass when `is_locked` changes, starting from logind's `false`: `pending`, `locked` and `locker-lost` are all `true`.
- **The sleep delay's "on screen"** is `Lock::is_shown_on`: a locker holds the lock and every output has drawn a frame with its lock surface's buffer on it. The background alone isn't enough, so a locker that never draws, or the give-up state, waits the 2 s, with a `warn` line. An unlocked session lets the delay go at once: locking on suspend is mate-power-manager's.
- **The inhibitor's fd** comes from zbus without close-on-exec, so Caminito sets it as it arrives. A locker spawned with it would otherwise hold the machine awake for as long as it runs.
- **The stand-in** keeps each inhibitor's read end, which hangs up once Caminito closes the write end, as logind's FIFO does. The 2 s test waits the real 2 s.
- **Test first:** the unit test of `ended` and the headless tests were written first and failed to build before the code.
- **The checklist items** are *The suspend race with a slow locker* and *`loginctl unlock-session` from a console*, tagged M7. They weren't run: they need a Session on the tty, and the ticket was done in a container with no display. They're left to [136](136-packaging-and-closing-m7.md).
