# Idle, blanking and the lock after it

Epic: caminito-v0.1
Type: implementation
Status: open
Milestone: M7
Blocked by: 126
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Caminito owns idle ([06](06-v01-protocol-and-integration-set.md)'s *Idle and presence* row, [09](09-gsettings-mapping.md)'s *Idle, blanking and locking*): the `idle` module, with the timers as something a test can drive with its own clock.

- **The threshold:** `org.mate.session idle-delay` (minutes). After it, the outputs are blanked if `org.mate.screensaver idle-activation-enabled` is on.
- **The lock after it:** with `lock-enabled`, a lock is requested `lock-delay` minutes after blanking starts.
- **Activity** on any input device unblanks and restarts the timer. It never unlocks.
- **ext-idle-notify,** with idle clients notified whatever `idle-activation-enabled` says.
- **Inhibition:** the idle-inhibit protocol, counted while its surface is visible, and mate-session's `IsInhibited` (`dbus::session_manager`). Neither applies while locked, when no client surface is visible ([12](12-screen-locking.md)).
- **The debug dump** has the idle state and the inhibitors.

## Acceptance criteria

- [ ] Unit or headless tests with a driven clock, written before the code:
  - the outputs blank at `idle-delay`, and not with `idle-activation-enabled` off;
  - a key or a pointer motion unblanks and restarts the timer;
  - the lock comes `lock-delay` after blanking, and not with `lock-enabled` off;
  - an ext-idle-notify client gets `idled` and `resumed`;
  - an idle-inhibit surface that's visible holds idle off, and one on another workspace or minimised doesn't;
  - mate-session's `IsInhibited`, from a stand-in on the bus, holds it off;
  - while locked, the inhibitors hold nothing off.
- [ ] A change of each key applies without a restart.
- [ ] The checklist has an item, tagged M7, and it was run in a Session with `idle-delay` at one minute: the screen blanks, a key brings it back, a playing video holds it off, and the session locks after the delay. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **mate-session's own idle presence stays inaccurate:** it's X SYNC over XWayland (06). Whether mate-session then calls anything on idle is found here and written down.
- **Powering the outputs off** is [129](129-dpms.md)'s.
