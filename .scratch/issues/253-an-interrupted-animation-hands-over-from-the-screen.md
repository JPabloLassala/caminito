# An interrupted animation hands over from what is on screen

Epic: open-close-animation
Type: implementation
Status: ready-for-agent
Blocked by: 252
Spec: [Opening and Closing animations](../open-close-animation/spec.md)

## What to build

"Interruptions" in [the spec](../open-close-animation/spec.md): each starts from what is on screen at the time, so nothing jumps. Plasma restarts from the animation's own start instead; ADR 0034 says why Caminito doesn't.

- **A window closes while it opens:** its picture shrinks and fades from the size and opacity it had reached, and takes the part of the 200 ms that's left on the closing curve from there.
- **A window is shown again while its picture fades:** the picture is dropped and the window opens from the picture's size and opacity.
- **A window is minimised while it opens:** the minimise animation takes it from its real place, as now, and the opening ends.
- **A window is maximised, resized or moved while it opens:** it grows towards its new place, read each frame.

## Acceptance criteria

- [ ] Headless tests that advance the clock, one for each of the four, read just before and just after the interruption: the drawn rectangle and opacity don't jump.
- [ ] Each ends at the place and opacity it should: gone, at the window's place, in the window list's box, at the new place.
