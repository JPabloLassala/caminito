# The minimise animation

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's minimise animation, towards the window list's button ([05](05-v01-marco-feature-cut.md)). [65](65-foreign-toplevel.md) stores the button's rectangle that mate-panel sets for each window, and nothing reads it yet.

- **The animation:** a minimised window shrinks into its button's rectangle, as Marco's does, drawn in the Scene from the window's last content.
- **Off** with `compositing-manager` off, with `reduced-resources` on, or with `org.mate.interface enable-animations` off ([09](09-gsettings-mapping.md)).
- **Both kinds of window,** and whichever way they're minimised (a binding, the Frame's button, the window list, a client's request).

## Acceptance criteria

- [ ] Headless tests through the fixture, written before the code, with the driven clock: a minimised window drawn between its place and the rectangle part-way through, and gone at the end; no animation with each of the three keys; no rectangle; the window restored during the animation.
- [ ] `reduced-resources` and `enable-animations` leave `UNSUPPORTED` if 147 put them there; their rows, and `compositing-manager`'s, in `docs/parity.md` say what they turn off, with the tests above.
- [ ] The checklist has an item, tagged M8: minimising a native and an X11 window from the Frame and the window list, then with `reduced-resources` on. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the animation's shape (a scaled window, or Marco's outline), its duration and easing.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **No rectangle** (no window list, or a window it doesn't show): proposed, no animation.
- **Whether Policy sees the window as minimised during it:** proposed, yes, at once, with only the drawing delayed, so that the focus moves on at once.
