# Shadows

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's compositor shadows around the windows Caminito decorates, on [07](07-decoration-strategy.md)'s model, which M2 left to M8 since `compositing-manager` governs the effects together ([17](17-milestone-slicing.md)).

- **Which windows:** SSD windows only. A CSD window draws its own.
- **The shape:** a Gaussian shadow around the Frame's visible rectangle, following its corners, outside the invisible resize border, with Marco's parameters for each window type and for the focused window.
- **Cached** per size and parameters, so that a frame of the same size draws no new shadow.
- **Off** with `compositing-manager` off.
- **Left out of input,** so a click on a shadow goes to what's under it, and **of captures:** a toplevel capture is the Frame and the window, and [103](103-image-capture.md)'s test that pins it to the Frame still passes.

## Acceptance criteria

- [ ] Unit tests of the shadow's geometry, written before the code: its rectangle for a Frame of each type, focused and not, at scales 1, 1.5 and 2.
- [ ] Headless tests through the fixture, on xdg and XWayland SSD windows: the shadow's pixels outside the Frame, none for a CSD window, none with `compositing-manager` off; a click on the shadow reaching the window under it; 103's capture test unchanged.
- [ ] `compositing-manager`'s row in `docs/parity.md` says it turns shadows off, and the keyless section's shadows say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: a native SSD window and an X11 one, focused and not, at a fractional scale, and with `compositing-manager` off. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: `compositor.c`'s per-type and focus parameters (radius, opacity, offsets), and which window types have none.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **Caching and damage at fractional scales:** proposed, the shadow is drawn at the output's scale and cached per scale, and damaged with its Frame.
- **Shaded windows:** proposed, the shadow follows the shaded Frame, as Marco's does.
