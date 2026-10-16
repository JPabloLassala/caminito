# 36. The Maximise animation is taken from Plasma

Date: 2026-10-16
Status: Accepted

## Context

Marco has no animation when a window is maximised, unmaximised, tiled or untiled: the window is at its old place in one frame and at its new one, at whatever size its client has drawn by then, in the next. Between the request and the client's commit at the new size, policy already has the window at its new place with its old size (`Managed::frame` lags behind until the commit), so the old content shows at the new place for a frame or more. ADR 0033 set the rules every animation follows, and ADR 0034 took the Opening and Closing animations from Plasma. The developer wants a fourth: the window moving and growing into its new place.

## Decision

- **Caminito has the Maximise animation, taken from Plasma's Maximize effect** (`src/plugins/maximize/package/contents/code/main.js` in KWin): 250 ms, the frame going from its old rectangle to its new one on a cubic ease-out, with a picture of the window as it was fading out over the live window, also on a cubic ease-out. MATE has no animation speed, so the length is fixed.
- **It shows maximising and unmaximising both ways, and tiling, untiling and a change of tile.** Plasma's effect shows maximise states only; Caminito's tile is a state of its own (ADR 0024) that moves a window just as far, so it animates too. Maximising one way and fullscreen don't animate.
- **It starts at the request, not at the client's commit.** The box moves at once, and the live window is scaled into it at whatever size the client has drawn, so a slow client is stretched for a while but nothing waits for it. The animation ends at 250 ms whether or not the client has drawn at the new size.
- **A drag that unmaximises or untiles a window doesn't animate it**: the window follows the pointer at once. Letting go of a drag that tiles or maximises it does.
- **It follows ADR 0033's rules.** Policy changes at once and only the drawing takes time. It runs when `compositing-manager` is on, `reduced-resources` off and `org.mate.interface enable-animations` on, and its own key in `org.mate.Caminito`, `maximize-animation`, is on by default.
- **An animation that is interrupted hands over from what is on screen**, as ADR 0034's do: from the box it was drawn in, keeping the first picture's fade. Plasma cancels the size change on a resize part-way and starts again from the window's geometry.
- **Inside the Overview, and during its transition, it doesn't run.** During a Workspace slide it runs, and the window moves with its workspace as well.

The spec has the behaviour in full: the project document *Spec: Maximise animation (spec)* in Linear.

## Consequences

- Caminito differs from Marco in what maximising and tiling a window look like. A user who wants Marco's turns `maximize-animation` off.
- The Scene draws a picture of a window over that same live window, and takes that picture while the window is still mapped, before its Frame is redrawn in the new state.
- A client that's slow to draw at its new size is stretched until the animation ends, and drawn as it is now after that.
- Rejected:
  - waiting for the client's commit to start, as KWin's Wayland windows can (the window would sit at its new place with its old content first, the very jump this hides);
  - holding the animation's end until the commit (a client that never draws at the asked size, or draws a size of its own, would hold it forever, so it would need a timeout too);
  - animating maximising one way and fullscreen (not asked for: fullscreen is a mode change more than a move, and one-way maximising is rarely used);
  - animating a drag's unmaximise (the window would lag behind the pointer);
  - Plasma's restart from the window's geometry when it's resized part-way (it jumps);
  - one key for the Opening, Closing and Maximise animations (each has its own, as each of ADR 0033's effects does).
