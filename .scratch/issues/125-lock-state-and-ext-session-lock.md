# The lock state and ext-session-lock

Epic: caminito-v0.1
Type: implementation
Status: open
Milestone: M7
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Caminito locks the session itself, and a locker only draws on it ([12](12-screen-locking.md)'s *Locking, fail closed*, ADR 0007, [17](17-milestone-slicing.md)). With a locker started by hand, such as `swaylock`, the session locks and unlocks.

- **The state:** the `lock` module, with the state machine as a pure type (unlocked, pending, locked, locker-lost) and the code that applies it ([15](15-crate-and-module-architecture.md)). A lock request enters the locked state at once: every output shows a solid dark background, and no client has the keyboard or the pointer.
- **ext-session-lock:** the global, with the other privileged globals of [68](68-data-control-and-privileged-globals.md), so a sandboxed client isn't offered it. A client's `lock` is a lock request like any other, and its lock surfaces go on their outputs.
- **Only the locker unlocks:** `unlock_and_destroy` from the client that holds the lock. A client that goes away without it leaves the session locked.
- **Taking a lock over:** once locked with the lock's client gone, a `lock` is refused unless it's from a locker Caminito spawned, matched by pid. Until [126](126-the-locker-command.md) spawns one, no client is.
- **Over everything:** the lock is the scene's top layer, above `overlay` layer surfaces and override-redirect windows. Xwayland keyboard grabs are refused and released while locked ([11](11-xwayland-integration.md)).
- **Keys while locked:** only VT switching and keyboard layout switching. Every other binding and every accelerator grabbed through `GrabAccelerators` is suppressed.
- **The debug dump** has the lock state ([14](14-development-and-testing-loop.md)).
- **The test client** gains ext-session-lock, as 14 lists.

## Acceptance criteria

- [ ] Unit tests of the state type, written before the code: every transition of 12, the refused take-over among them.
- [ ] Headless tests with a test locker, written before the code:
  - a `lock` gets `locked`, and from then no other client has keyboard focus or pointer events;
  - `unlock_and_destroy` gives the focus back to the window that had it;
  - the locker killed leaves the dump locked, and a second client's `lock` is refused;
  - a binding and a grabbed accelerator do nothing while locked, and a layout switch does;
  - an Xwayland keyboard grab asked for while locked is refused, and one held is released.
- [ ] A sandboxed client isn't offered the global.
- [ ] The checklist has an item, tagged M7, and it was run nested: `swaylock` locks, an `overlay` layer surface and a menu left open don't show over it, and the password unlocks. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **The background drawn before the locker's first frame** is what makes the lock fail closed: nothing of the session shows between the request and the locker's frame.
- **Capture while locked** is [133](133-capture-while-locked.md)'s, AccessX's exception [134](134-accessx.md)'s and the Keyboard monitor's [135](135-keyboard-monitor-for-orca.md)'s.
