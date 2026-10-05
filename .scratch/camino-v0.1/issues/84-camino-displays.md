# camino-displays

Type: implementation
Status: open
Milestone: M6 (proposed)
Blocked by: 81, 82, 83
Map: [Camino v0.1](../map.md)

## What to build

camino-displays, the display settings program that replaces MATE's Display capplet in a Session ([ADR 0014](../../../docs/adr/0014-camino-keeps-the-layouts-and-camino-displays-edits-them.md)): a Rust program in this repo, in GTK4 without libadwaita. It stands in for the patched mate-control-center that 17 put in M6 for the capplet's 5% steps.

The prototype that settled the design is `.scratch/camino-displays-prototype/index.html` on the branch `prototype/camino-displays` (8a4981d), with its verdicts in the commit message. It is a source, not code to keep. Its page is the design.

- **The page,** as Plasma's: the arrangement of the displays on top, a row with one entry per display, the selected display's settings below, and Identify, Reset and Apply at the bottom. One page, no tabs. The arrangement stays when there is one display. The user reads "display" here, never "output".
- **Arranging:** a dragged display docks to the outline of the others ([82](82-layout-healing.md)), and the others close up as soon as it is picked up. It snaps to the matching edge of a display it lies against, within 12 pixels of the canvas, whatever the zoom. The primary display has a star.
- **Per display:**
  - Enabled. The only display that is on can't be turned off.
  - Primary ([81](81-primary-output.md)).
  - Resolution: one row per size with its aspect ratio, the native one marked.
  - Refresh rate: the rates of the chosen resolution. A new resolution takes the rate nearest the current one, the higher of two equally near.
  - Scale: 100% to 300% in 25% steps, and what the display will look like in logical pixels. A current scale off that grid is shown as "135% (automatic)", and can't be chosen again once the slider moves.
- **A change of size** (resolution or scale) keeps the displays to the right and below on their side, then heals (82).
- **Apply:** nothing changes until Apply. The configuration goes over wlr-output-management, then "Keep this configuration?" counts 15 seconds down and reverts at the end. Keep asks Camino to save ([83](83-saved-layouts.md)). A change of the primary display alone is applied and saved at once.
- **Identify** shows each display's name on the monitor itself. How, a layer surface of camino-displays or a request to Camino, is to decide here.
- **A hot-plug while the window is open** shows the new set, and drops what wasn't applied.
- **Packaging:** `camino-git` ships it with a desktop entry that takes the Display capplet's place in the control center under Camino. How the stock entry is hidden there is to decide here.
- **Not in it:** rotation, mirroring and adaptive sync, which Camino doesn't do (58).

## Acceptance criteria

- [ ] The page matches the prototype's, with MATE's GTK theme, at 100% and at a fractional scale.
- [ ] Nested, with two headless or winit outputs: a scale and a place changed and applied are what `wlr-randr` then lists; Revert, and the countdown running out, put the old ones back; Keep leaves a saved layout.
- [ ] The arrangement can't be left with a gap, an overlap or a display cut off, by dragging or by any setting.
- [ ] On the tty with two monitors: a mode and a refresh rate change, a display turned off and on, and the primary display changed, which moves the panel.
- [ ] In the session, the control center's "Displays" opens camino-displays, and `docs/testing/checklist.md` has its items.

## Notes

- **GTK4 and MATE's theme:** GTK4 follows MATE's GTK theme less closely than GTK3 would (ADR 0014). What it looks like with the themes MATE ships is to look at first, before the page is built out.
- **The toolkit's Rust bindings** are a new dependency for the workspace, and `deny.toml` has to pass.
