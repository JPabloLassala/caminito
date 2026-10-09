# The lock state and ext-session-lock

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, and the rest of the swaylock check (an overlay surface, a menu, the bindings on the tty) is 136's
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

- [x] Unit tests of the state type, written before the code: every transition of 12, the refused take-over among them.
- [x] Headless tests with a test locker, written before the code:
  - a `lock` gets `locked`, and from then no other client has keyboard focus or pointer events;
  - `unlock_and_destroy` gives the focus back to the window that had it;
  - the locker killed leaves the dump locked, and a second client's `lock` is refused;
  - a binding and a grabbed accelerator do nothing while locked, and a layout switch does;
  - an Xwayland keyboard grab asked for while locked is refused, and one held is released.
- [x] A sandboxed client isn't offered the global.
- [ ] The checklist has an item, tagged M7, and it was run nested: `swaylock` locks, an `overlay` layer surface and a menu left open don't show over it, and the password unlocks. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **The background drawn before the locker's first frame** is what makes the lock fail closed: nothing of the session shows between the request and the locker's frame.
- **Capture while locked** is [133](133-capture-while-locked.md)'s, AccessX's exception [134](134-accessx.md)'s and the Keyboard monitor's [135](135-keyboard-monitor-for-orca.md)'s.

## Done (2026-10-08)

- **The checklist item** is *swaylock locks the session, over everything, and its password unlocks*, tagged M7. It was run nested in part on 2026-10-08, with swaylock 1.8 on a build of `master`:
  - `swaylock` locked it and the dump's `lock` went from `unlocked` to `locked`; a second `swaylock` was refused and exited; with the first killed the dump had `locker-lost` and the next `swaylock` was refused too, with the log's line for each.
  - The developer saw swaylock with no window of the session over it, typed to its indicator, and unlocked with the password: the dump was `unlocked` again, and pluma was back with the text typed in it before the lock.
  - Not run, and left to [136](136-packaging-and-closing-m7.md): an `overlay` layer surface and a menu left open under the lock (no overlay client was installed), and the item's step on the tty, where the bindings are pressed while locked.
- **The bindings can't be pressed in a nested run:** the host's compositor has them, and its Alt+F4 closed the nested Caminito. The item's nested steps press none, and its last step has them on the tty.
- **Test first:** the state type's unit tests were red before its transitions were written, and so were the first headless test and the two the review asked for. The other headless tests were written after the code they cover, against the criterion. Each was then checked by taking its line of the code out and seeing the test fail. The one that survived is the refusal of an Xwayland keyboard grab under the lock: the grab is also ended at the next key and the next refresh pass, so the test passes on either.
- **The states:** `pending` is the session locked at Caminito's own request before the locker has taken the lock. Only a locker Caminito spawned takes a pending lock, as with a lost one, so nothing else can attach to it and unlock. [126](126-the-locker-command.md)'s request seam is `LockState::requested`'s first caller, and its spawn sets `Lock::spawned_locker`.
- **`locked` waits for a frame of the lock on every output**, as ext-session-lock has it. The session is locked at the request all the same: the keyboard and the pointer leave it in the handler, and no frame drawn from then shows it. An output that can't draw, as on another VT, holds `locked` back until it can. [130](130-logind-lock-unlock-and-sleep.md)'s sleep delay can wait on the same thing.
- **The keyboard path:** 15 has the lock as a gate before xkbcommon. It drops no key, since the locker is typed to: the bindings stage runs VT switching alone and the grabs stage nothing, and xkbcommon switches layouts.
- **Grabs:** as the lock comes on, menus are dismissed and every grab of the keyboard and the pointer ends. Under it, popup grabs, moves and resizes and Xwayland keyboard grabs are refused. An input method's keyboard grab is Smithay's to grant, so it's ended at the next key instead; there's no test of that, as the test client has no input method. An input method that held a grab doesn't get it back at the unlock.
- **Nothing below the lock is drawn** while locked, rather than drawn and covered. Capture goes through the same layers, so it shows the lock already; [133](133-capture-while-locked.md) has the tests and the window captures.
