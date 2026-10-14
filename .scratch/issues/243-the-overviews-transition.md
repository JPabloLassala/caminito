# The Overview's transition

Epic: overview
Type: implementation
Status: ready-for-agent
Blocked by: 241
Spec: [Overview](../overview/spec.md)

## What to build

The Overview opens and closes with each window moving between its real place and its place in the layout, under "The transition" and "Settings" in [the spec](../overview/spec.md). It builds on ticket 241's Overview, which opens and closes in one frame.

- **250 ms with an ease-out curve**, the Workspace slide's, from Caminito's clock, drawn on every frame of the output, with a deadline as the slide has.
- **Opening:** each window moves and shrinks from its real place to its place in the layout. A minimised window fades in at its place. The dimming fades in, and the panel and caja's desktop fade out. The bar fades in with them when 244 is in.
- **Closing:** every window moves back to its real place at once. A chosen window is raised as they go. A minimised window that wasn't chosen fades out.
- **The shortcut pressed again part-way** reverses the transition from where it is.
- **Policy has the Overview open or closed from the first frame**, and only the drawing takes time: a click or a key part-way through the opening acts on the layout's final places, and one part-way through the closing reaches the windows at their real places.
- **`overview-animation`** in `org.mate.Caminito`, a boolean, default `true`, in both copies of the schema. The transition runs when it is on and the minimise animation's three keys allow: `compositing-manager` on, `reduced-resources` off and `org.mate.interface enable-animations` on. Without it the Overview opens and closes in one frame. The key turns off the transition, not the Overview.
- The session lock ends a transition at once.
- The fixture keeps `overview-animation` off unless a test sets it, as it keeps `workspace-slide` off.
- The debug dump shows the transition: its direction, its part and each window's drawn rectangle.

## Acceptance criteria

- [ ] Headless tests that advance the clock, as the Workspace slide's do: a window's drawn rectangle is between its real place and the layout's part-way through the opening, and at the layout's after 250 ms; the same back for the closing.
- [ ] A minimised window, the dimming, the panel and caja's desktop are part-way through their fades at the transition's middle.
- [ ] The shortcut pressed part-way through the opening returns each window from where it is, with no jump.
- [ ] A chosen window is at the top of the stack from the closing's first frame.
- [ ] A click part-way through the opening reaches the window at the layout's final place.
- [ ] With `overview-animation` off, and with each of the three other keys against it, the Overview opens and closes in one frame.
- [ ] The tests of 241 and 242 pass unchanged.
