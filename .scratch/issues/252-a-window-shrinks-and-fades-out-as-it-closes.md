# A window shrinks and fades out as it closes

Epic: open-close-animation
Type: implementation
Status: ready-for-agent
Blocked by: 251
Spec: [Opening and Closing animations](../open-close-animation/spec.md)

## What to build

The **Closing animation** (CONTEXT.md), under "What opens and what closes", "Timing and shape", "Policy and input" and "The closing window's picture" in [the spec](../open-close-animation/spec.md).

- **A window that closes** while on screen (destroyed or hidden by its client) leaves a picture of its last frame, with its Frame and shadow, that shrinks from its size to 0.8 of it about its centre and fades out over 200 ms on a cubic ease-in. The windows are 251's.
- **The picture is kept as the window goes.** Nothing keeps a window's content today once `unmap_window` (`window/lifecycle.rs`) has policy drop it; the xdg `toplevel_destroyed` and X11 `unmapped_window` and `destroyed_window` paths all end there. When there is no buffer left to keep, the window closes at once, logged at debug level at most.
- **It is drawn at the window's place in the stack**, on its workspace and output, so a window above it stays above it and what is beneath shows through.
- **Policy lets the window go at once.** Focus moves on as with no animation, and a click on the picture reaches what is beneath it. No query of policy sees the picture.
- The debug dump lists each closing window's picture with the rectangle and opacity it is drawn at, and its place in the stack.

## Acceptance criteria

- [ ] Headless tests that advance the clock: a Wayland window destroyed and an X11 window destroyed and unmapped, read part-way and at the end, and gone at 200 ms.
- [ ] Part-way, a window that was above the closing one is drawn above its picture, and a click on the picture reaches the window beneath.
- [ ] The focus has moved on in the first frame after the close.
- [ ] A window whose buffer is gone at the close closes at once.
- [ ] With `open-close-animation` off, and with each of the three other keys against it, a window is gone in one frame.
- [ ] The parity tests pass unchanged.
