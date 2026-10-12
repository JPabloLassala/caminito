# A workspace switch slides

Epic: workspace-slide
Type: implementation
Status: ready-for-agent
Blocked by: 230
Spec: [Workspace slide](../workspace-slide/spec.md)

## What to build

The first slice, and the one a user can try: the **Workspace slide** (CONTEXT.md) on one output, for every direction, under "Direction and distance", "Which switches slide", "Timing", "Policy and input" and "Settings" in [the spec](../workspace-slide/spec.md). The windows that stay still are ticket 234, a second switch during a slide is 235, and two outputs are 236.

- **On a change of the output's active workspace**, whatever its cause, the old workspace's windows leave and the new one's arrive, with their Frames, shadows and popups. Layer surfaces and the wallpaper stay still.
- **The line is the Workspace grid's**: one output's width for each column between the two cells and one output's height for each row. A jump is one straight line, and no workspace between is drawn. A wrap under `wrap-style` slides one cell in the direction of the key. There is no gap.
- **250 ms, ease-out**, from Caminito's clock, drawn on every frame of the output, with a deadline as the minimise animation has (`caminito/src/deadlines.rs`). Choose the curve and record it in the spec's "Not decided".
- **Policy switches at once.** Focus, input, ext-workspace and X11 are as they are with no slide, and a click part-way reaches the new workspace's window at its final place.
- **The Workspace popup** is drawn over the sliding windows.
- **`workspace-slide`** in `org.mate.Caminito`, a boolean, default `true`, in both copies of the schema. The slide runs when it is on and the minimise animation's three keys allow (`settings/translate.rs`). A change applies to the next switch.
- A second switch during a slide ends the running one at once and starts its own, until 235.
- The debug dump shows the slide: its output, its two workspaces, its offset, and each sliding window's drawn rectangle.

## Acceptance criteria

- [ ] Headless tests that advance the clock, as `caminito/src/tests/minimise_animation.rs` does: a switch right, left, up and down in a grid with rows, read part-way and at the end; a jump across two columns and a row; a wrap.
- [ ] Part-way through, the old workspace's windows and the new one's are both drawn, the panel hasn't moved, and after 250 ms only the new one's are drawn, at their own places.
- [ ] A switch from the pager's ext-workspace request and one from activating a window that isn't showing both slide.
- [ ] A key press part-way reaches the new workspace's focused window, and a click reaches the window at its final place.
- [ ] With `workspace-slide` off, and with each of the three other keys against it, a switch is a cut.
- [ ] The parity tests pass unchanged, with the key off where they read a frame after a switch.
