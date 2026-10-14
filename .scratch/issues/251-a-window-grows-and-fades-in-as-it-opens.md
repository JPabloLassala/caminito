# A window grows and fades in as it opens

Epic: open-close-animation
Type: implementation
Status: ready-for-agent
Blocked by: None
Spec: [Opening and Closing animations](../open-close-animation/spec.md)

## What to build

The first slice, and the one a user can try: the **Opening animation** (CONTEXT.md), under "What opens and what closes", "Which windows", "Timing and shape", "Policy and input" and "Settings" in [the spec](../open-close-animation/spec.md). Closing is 252, interruptions 253, and the other effects 254.

- **A window that is shown** for the first time, or again after its client hid it, grows from 0.8 of its size to its own about its centre and fades in from no opacity, over 200 ms on a cubic ease-out, with its Frame, shadow and popups. Both Wayland (`xdg_shell_commit`) and X11 (`xwayland_commit`) reach this through `window_committed` → `map_pending` (`window/lifecycle.rs`).
- **Which windows:** one with a Frame, or of type Normal, Dialog or ModalDialog. Not override-redirect windows, popups, Layer surfaces, or the other types without a Frame. Not a window on a workspace no output shows.
- **Drawn on every frame** of its output from Caminito's clock, with a deadline (`caminito/src/deadlines.rs`), through `scaled_window_elements` (`scene.rs`) as the Overview draws a window at a place, size and opacity.
- **Policy maps it at once.** Focus, input and what the client is told are as with no animation, and a click part-way reaches the window at its real place.
- **`open-close-animation`** in `org.mate.Caminito`, a boolean, default `true`, in both copies of the schema. The animation runs when it is on and the minimise animation's three keys allow (`settings/translate.rs`). A change applies to the next window.
- The fixture keeps the key off unless a test sets it, as it does `workspace-slide`.
- The debug dump lists each opening window with the rectangle and opacity it is drawn at.

## Acceptance criteria

- [ ] Headless tests that advance the clock: a Wayland and an X11 window read part-way and at the end, smaller and fainter part-way about the same centre, and at 200 ms at their own place with full opacity.
- [ ] A Utility window with a Frame animates; a Menu, a Splashscreen without a Frame, an override-redirect window and a Layer surface don't.
- [ ] An X11 window unmapped and mapped again opens again.
- [ ] A key press part-way reaches the new window, and a click reaches it at its real place outside the drawn rectangle.
- [ ] With `open-close-animation` off, and with each of the three other keys against it, a window is there in one frame.
- [ ] The parity tests pass unchanged.
