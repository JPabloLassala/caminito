# Map: Opening and Closing animations

Status: closed on 2026-10-14. It reached its destination, [the spec](spec.md), in one grilling session with the developer.

## Destination

A spec, ready to cut into implementation tickets, for a window that zooms and fades as it opens and closes.

## Notes

- **Domain:** **Opening animation** and **Closing animation** are in [CONTEXT.md](../../CONTEXT.md). The choice of Plasma's Scale effect and its rules are [ADR 0034](../../docs/adr/0034-the-opening-and-closing-animations-are-taken-from-plasma.md), which follows [ADR 0033](../../docs/adr/0033-the-workspace-slide-and-the-overview-are-taken-from-plasma.md).
- **Not in a milestone.** It gates nothing of v0.1.
- **What the code has today:** the minimise animation draws the live window, which a minimised window still has. Nothing keeps a window's content once policy drops it (`window/lifecycle.rs`: `unmap_window`), so the Closing animation's picture is new. `scaled_window_elements` (`scene.rs`) draws a window at a place, size and opacity, and the Overview uses it.

## Decisions so far

From the grilling session of 2026-10-14:

- **Plasma's Scale effect**, with its numbers: 200 ms, 0.8 to 1 and opacity 0 to 1 about the centre, ease-out (cubic) to open and ease-in (cubic) to close.
- **Policy at once**, input at the real place, and the closing window's picture clicked through.
- **One key, `open-close-animation`,** on by default, behind the minimise animation's three.
- **Opens:** shown for the first time or again after its client hid it. **Closes:** destroyed or hidden by its client while on screen. Minimise, unminimise and workspace moves are neither.
- **Windows:** Plasma's rule, a Frame or type Normal, Dialog or ModalDialog. Fullscreen windows animate.
- **The picture:** the last frame with Frame and shadow, at its place in the stack. No picture, no animation.
- **Interruptions** hand over from what is on screen.
- **Slide:** both at once. **Overview:** neither. **Lock and an output going:** end them, as for the slide.
- **Tests:** the debug dump and the clock, no golden image.
- **The epic is `open-close-animation`**, tickets 251 to 254.
