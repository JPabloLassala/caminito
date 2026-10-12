# 33. The Workspace slide and the Overview are taken from Plasma

Date: 2026-10-12
Status: Accepted

## Context

Marco is Caminito's reference, and it has two effects only: the minimise animation (172) and the visual bell (169). v0.1's map lists "showy effects (wobbly windows and the like)" as out of scope, and ADR 0016 says "No effect is taken from KDE" for that reason.

The developer wants two of Plasma's:

- its Slide effect, which shows a change of workspace as the old workspace's windows leaving and the new one's arriving;
- its Overview, which shows a workspace's windows shrunk and set side by side with a bar of the workspaces, and opens and closes with each window moving between its real place and its place there.

Neither is in Marco, so neither has a row in the parity table or a key in `org.mate.Marco.*`.

## Decision

- **Caminito has the Workspace slide and the Overview, taken from Plasma.** This amends ADR 0016's "No effect is taken from KDE", which stays true of the Window switcher.
- **They are outside v0.1,** each in an epic of its own: `workspace-slide` first, then `overview`. v0.1's out-of-scope line and its parity exit stay as they are.
- **Policy changes at once, and only the drawing takes time,** as with the minimise animation. Focus, keys, clicks and what clients are told follow policy from the first frame of a slide.
- **The Overview is an Interaction, with its state in policy** (ADRs 0010, 0016, 0026): the windows listed, the selection and the drag. The compositor crate has the layout, the drawing and the transition. Caminito draws it in the scene.
- **Every animation follows the three keys the minimise animation follows:** `compositing-manager` on, `reduced-resources` off and `org.mate.interface enable-animations` on. Each effect also has a key in `org.mate.Caminito`, on by default: `workspace-slide` and `overview-animation`. The second turns off the Overview's transition, not the Overview.
- **The Overview has no search field.** MATE has no KRunner, and Caminito links no toolkit to draw a text field (ADR 0003).
- **Only a System shortcut opens the Overview** in its first version.

The two specs have the behaviour in full: `.scratch/workspace-slide/spec.md` and `.scratch/overview/spec.md`.

## Consequences

- Caminito differs from Marco in what a workspace switch looks like. A user who wants Marco's turns `workspace-slide` off.
- The scene has to draw windows of a workspace that isn't active, at an offset and clipped to their output, and to draw a window at another size and place than its own while it is still live. The Window switcher's thumbnails are the nearest thing to it.
- During a slide a window is drawn where a click doesn't reach it: input goes to the new workspace's windows at their final places.
- The Workspace popup can't show while the Overview is open, since a seat has one Interaction. The Overview's bar marks the active workspace instead.
- Orca doesn't read the Overview, as it doesn't read the Window menu (ADR 0026).
- Both effects are checked under Per-output workspaces and under Spanning workspaces (ADR 0032).
- Rejected:
  - joining v0.1 (its exit is parity with Marco, which has neither effect);
  - one epic for both (the slide is a change to an existing action, and the Overview is a new Interaction that builds on what the slide adds to the scene);
  - a client on a Layer surface for the Overview (ADR 0016's reason: it would need a private protocol for the windows' content, the keys and the drag);
  - sliding the wallpaper, the panel and caja's desktop (each is the same thing on both workspaces);
  - a hot corner and a touchpad gesture now (each is a ticket of its own, and a gesture means a slide that follows the fingers, a larger design);
  - keys for the length and the curve (one more setting for a thing few would change).
