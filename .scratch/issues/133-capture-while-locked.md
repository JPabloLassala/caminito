# Capture shows the lock, never the session

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M7
Blocked by: 125
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

While locked, every image capture source renders what the lock shows ([12](12-screen-locking.md), [17](17-milestone-slicing.md)'s *Capture while locked*). [103](103-image-capture.md) built the sources.

- **An output's source** has the lock surfaces, or the solid background before the locker's first frame.
- **A toplevel's source** has the background alone: its window isn't shown to anyone while locked. Its capture session isn't stopped, so it goes on after the unlock.
- **wlr-screencopy,** if Caminito has it by then, follows the same rule.

## Acceptance criteria

- [x] Headless tests, written before the code: with a window of a known colour on screen and the session locked, an output's capture and that window's capture have none of its pixels, before and after the test locker's first frame; after the unlock both have them again.
- [x] The checklist's lock item of [125](125-lock-state-and-ext-session-lock.md) gains a step: `grim` or `mate-screenshot` run with a delay from before the lock saves the lock screen.

## Notes

- **A capture client that is the locker's own** gets the same frames as any other.

## Done (2026-10-09)

- **The test** is `a_capture_shows_the_lock_never_the_session`, on both kinds of window. It opens an output session and a window session, with `paint_cursors` and the pointer over the window, before the lock, and takes a frame of each: before the locker's first frame both are the lock background alone, after it the output's session has the locker's colour and a new window session the background (the old one has nothing new to show, so it waits), and after the unlock the first two sessions have the window again. It was red before the code, on the window's frame under the lock, which never came: nothing in its capture changed.
- **An output's source needed no code:** 125 had the capture go through the scene's layers, which show the lock alone while locked.
- **A toplevel's source** is a solid of the lock background, of the size of its frame rect, so its sessions keep their constraints and go on. It has no cursor while locked, whatever `paint_cursors` says: the cursor is in it only over a window that's showing.
- **wlr-screencopy** is still deferred (06), so there's nothing of it to follow the rule.
- **The checklist step** is in *swaylock locks the session, over everything, and its password unlocks*: `grim` 1.5 or later (older ones have only wlr-screencopy), or `mate-screenshot --delay=6`, started with swaylock. It wasn't run here: it's part of [136](136-packaging-and-closing-m7.md)'s closing run.
