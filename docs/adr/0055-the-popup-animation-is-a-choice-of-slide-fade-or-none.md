# 55. The Popup animation is a choice of slide, fade or none

Date: 2026-10-31
Status: Accepted

## Context

ADR 0037 gave every popup the Popup slide and rejected Plasma's Fading popups, because the developer chose the slide. The developer now wants the fade as well, for the user to choose between them. None of Caminito's animations has any UI yet: their keys in `org.mate.Caminito` can only be set from outside, though `caminito-behaviour` has always planned a tab for them.

## Decision

- **Popups have one Popup animation: Slide, Fade or None.** This amends ADR 0037, whose slide stays as it is, and stays the default, so nobody's desktop changes on upgrade. A popup never slides and fades at once.
- **The key `popup-animation`, of those three values, replaces the boolean `popup-slide`.** There is no migration: anyone who set `popup-slide` off gets the slide again until they choose None.
- **The Popup fade is taken from Plasma's Fading popups, but for its closing:** 150 ms in and 150 ms out, at the popup's own place and size, with no scale. Plasma closes over 600 ms on an OutQuart curve, which would keep a closing menu's picture on screen far longer than it took to open.
- **It follows ADR 0037's rules, for the same popups.** Only the drawing differs: opacity in place of movement, with no edge and no cut-off. It has the grab, the keyboard and clicks at the popup's real place from its first frame, doesn't run inside the Overview, during its transition or under the lock, moves with its window during a Workspace slide, leaves with a window that closes, and turns round from where it is when interrupted. The Window switcher and the Workspace popup don't fade.
- **An Intellihide panel slides under Slide and Fade, and goes at once under None.** This amends ADR 0047, which tied it to the Popup slide's setting.
- **`caminito-behaviour` has an Animations tab** with the Popup animation's choice and switches for the Workspace slide, the Overview's transition, the Opening and Closing animations and the Maximise animation. Esmeril isn't on it, as it isn't an animation. When `compositing-manager`, `reduced-resources` or `enable-animations` turns the animations off, the tab's controls are greyed out, keep their values and say why, and they follow those keys live.

## Consequences

- A user who wants Marco's popups chooses None, not `popup-slide` off.
- Caminito's fade differs from Plasma's in how long a closing popup takes.
- Rejected:
  - replacing the slide with the fade (ADR 0037's slide is still wanted, and is still the default);
  - a scale as well as the fade, the "pop" in pop-in (the developer wanted only the fade, and it isn't Plasma's);
  - keeping `popup-slide` beside a new key (two settings that can contradict each other);
  - fading an Intellihide panel (it has to leave its edge either way);
  - Plasma's 600 ms fade out.
