# Capture shows the lock, never the session

Epic: caminito-v0.1
Type: implementation
Status: open
Milestone: M7
Blocked by: 125
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

While locked, every image capture source renders what the lock shows ([12](12-screen-locking.md), [17](17-milestone-slicing.md)'s *Capture while locked*). [103](103-image-capture.md) built the sources.

- **An output's source** has the lock surfaces, or the solid background before the locker's first frame.
- **A toplevel's source** has the background alone: its window isn't shown to anyone while locked. Its capture session isn't stopped, so it goes on after the unlock.
- **wlr-screencopy,** if Caminito has it by then, follows the same rule.

## Acceptance criteria

- [ ] Headless tests, written before the code: with a window of a known colour on screen and the session locked, an output's capture and that window's capture have none of its pixels, before and after the test locker's first frame; after the unlock both have them again.
- [ ] The checklist's lock item of [125](125-lock-state-and-ext-session-lock.md) gains a step: `grim` or `mate-screenshot` run with a delay from before the lock saves the lock screen.

## Notes

- **A capture client that is the locker's own** gets the same frames as any other.
