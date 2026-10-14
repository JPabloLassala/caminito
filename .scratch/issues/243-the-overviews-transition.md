# The Overview's transition

Epic: overview
Type: implementation
Status: resolved
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

- [x] Headless tests that advance the clock, as the Workspace slide's do: a window's drawn rectangle is between its real place and the layout's part-way through the opening, and at the layout's after 250 ms; the same back for the closing.
- [x] A minimised window, the dimming, the panel and caja's desktop are part-way through their fades at the transition's middle.
- [x] The shortcut pressed part-way through the opening returns each window from where it is, with no jump.
- [x] A chosen window is at the top of the stack from the closing's first frame.
- [x] A click part-way through the opening reaches the window at the layout's final place.
- [x] With `overview-animation` off, and with each of the three other keys against it, the Overview opens and closes in one frame.
- [x] The tests of 241 and 242 pass unchanged.

## Done (2026-10-14)

- **Tested first:** `overview/transition.rs` is pure and has unit tests of the curve, the reversal and the rectangle between two places. The headless tests are in `tests/overview_transition.rs`, with Caminito's clock stopped, read from the new `overview_transition` member of the debug dump and from pixels. One of them runs under Per-output workspaces. Two outputs are 246's.
- **Policy is unchanged, and the compositor keeps its `Overview` for the closing's frames.** Policy closes at once, as in 241, so keys and clicks already acted on the final state; `refresh_overview` starts a closing from where the drawing is, and a reopen part-way starts an opening from there. The dump's `overview` member stays `null` from the closing's first frame, so 241's and 242's tests read it unchanged.
- **The scene draws each listed window between its real place and its slot**, in policy's stacking order, so a chosen window is at the top from the closing's first frame. A minimised window stays at its slot and fades with the part; a chosen one is unminimised by policy and moves with the others. The labels fade with the dimming. The spec doesn't name the labels; this was my choice.
- **The panel and caja's desktop fade.** `layer_surface_alpha` replaces the yes-or-no of 241, and `overview_hides` is true only at alpha 0. So the dump's `hidden_layer_surfaces` is empty during a transition.
- **`is_minimized` is in `SCENE_QUERIES`, `SceneView` and `SceneReads`**, since the scene reads it now. `overview` stays in them: the scene still checks policy's Overview, and 241's `opening_and_closing_change_what_the_scene_reads` passes unchanged.
- **The reversal takes a whole 250 ms from where the drawing is**, as a retargeted Workspace slide does. The speed is continuous in place, not in velocity.
- **Known limits, not fixed:** a window that maps during the closing isn't drawn until the closing ends (at most 250 ms), as the Overview's layer replaces the windows' layers until then. The Overview's layer is above the top layer, so a window that overlaps the panel at its real place goes from above the panel to below it at the closing's last frame. The windows have no shadow during the transition, and get it back at its end. Ticket 247 can judge all three in a Session.
- **Tests run:** all of `caminito-policy`, and `caminito`'s modules that match `overview`, `minimise`, `switcher`, `scene`, `panel`, `layer`, `workspace_slide`, `shadow`, `thumbnail`, `deadlines`, `lock`, `frame`, `capture`, `settings`, `decoration`, `fullscreen`, `bell` and `tile_preview`, with fmt and clippy. Not the full suite. `tests::locker::a_killed_locker_comes_back_…` failed one time in the large run and passed three times alone; this change doesn't touch it.
