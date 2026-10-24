# 47. Caminito hides and slides an Intellihide panel, which only asks for it

Date: 2026-10-23
Status: Proposed

## Context

The Dock applet sits in a panel of its own, and that panel should get out of the way when a window overlaps it and come back when none does. This is Intellihide. mate-panel's own autohide is different: it hides whenever the pointer leaves, its Wayland version is unreleased 1.29 work, and the panel can't know when a window overlaps it, because foreign-toplevel carries no geometry.

## Decision

- **A carried mate-panel patch adds _Intellihide_ to a panel's Properties, beside Autohide.** With it on, the panel reserves no space, and it tells Caminito, through a small private protocol on its layer surface, that it wants intellihide.
- **Caminito does the rest.** It knows every window's geometry, so it decides when a window on the panel's output's active workspace overlaps the panel, and when the pointer has rested at that edge long enough to reveal it. It slides the layer surface out and back itself, with the popup slide (ADR 0037).
- **The panel is never told it is hidden.** It draws as usual, and its input region goes with the surface.

## Consequences

- Hiding is compositor policy, so it can be tested in `caminito-policy` like the work area, without a panel.
- Without Caminito, the patch's option does nothing and the panel behaves as an ordinary one. It is gated like every carried patch (ADR 0011).
- Rejected: a panel that hides itself. It would need every window's geometry from a new protocol, and it would race the compositor on every move and resize.
