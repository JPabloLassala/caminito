# 34. The Opening and Closing animations are taken from Plasma

Date: 2026-10-14
Status: Accepted

## Context

Marco has no animation when a window opens or closes: a window is there in one frame and gone in the next. ADR 0033 took the Workspace slide and the Overview from Plasma and set the rules every animation follows. The developer wants a third: a window that zooms and fades as it opens and closes.

## Decision

- **Caminito has the Opening animation and the Closing animation, taken from Plasma's Scale effect** (`src/plugins/scale/package/contents/code/main.js` in KWin): 200 ms, from 0.8 of the window's size and no opacity to its own size and full opacity about its centre, ease-out (cubic) to open and ease-in (cubic) to close. MATE has no animation speed, so the length is fixed.
- **They follow ADR 0033's rules.** Policy changes at once and only the drawing takes time. They run when `compositing-manager` is on, `reduced-resources` off and `org.mate.interface enable-animations` on, and their own key in `org.mate.Caminito`, `open-close-animation`, is on by default. One key covers both.
- **The windows are Plasma's:** a window with a Frame, or of type Normal, Dialog or ModalDialog. Override-redirect windows, popups and Layer surfaces never animate.
- **A closing window is drawn from a picture kept of it**, its last frame with its Frame and shadow, at its place in the stack, after policy has let it go. When there is no picture to keep, it closes at once.
- **An animation that is interrupted hands over from what is on screen**, not from its own start as Plasma does, so nothing jumps.
- **Inside the Overview, and during its transition, neither runs**, as Plasma skips the Scale effect under a full-screen effect. During a Workspace slide both run, and the window moves with its workspace as well.

The spec has the behaviour in full: `.scratch/open-close-animation/spec.md`.

## Consequences

- Caminito differs from Marco in what a window opening or closing looks like. A user who wants Marco's turns `open-close-animation` off.
- The Scene draws something that isn't a window in policy: the closing window's picture. It is clicked through, and no query of policy sees it.
- Rejected:
  - GNOME Shell's animation, and numbers of Caminito's own (the slide and the Overview are Plasma's, and Plasma's is the one the developer asked for);
  - 250 ms to match the slide and the minimise animation (200 ms is part of the Scale effect's feel);
  - a key for opening and another for closing, and keys for the length and the curve (a setting few would change, as ADR 0033 says);
  - Plasma's restart of an interrupted animation from its own start (a window closing half faded in would jump to full opacity);
  - an empty Frame faded out when the picture is gone;
  - animating the minimise and the unminimise this way (the minimise animation is Marco's and stays).
