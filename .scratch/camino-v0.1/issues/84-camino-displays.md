# camino-displays

Type: implementation
Status: resolved
Milestone: M6
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
- [x] The arrangement can't be left with a gap, an overlap or a display cut off, by dragging or by any setting. (A property test of the page, `whatever_the_window_does_the_arrangement_stays_contiguous`.)
- [ ] On the tty with two monitors: a mode and a refresh rate change, a display turned off and on, and the primary display changed, which moves the panel.
- [ ] In the session, the control center's "Displays" opens camino-displays, and `docs/testing/checklist.md` has its items.

## Notes

- **GTK4 and MATE's theme:** GTK4 follows MATE's GTK theme less closely than GTK3 would (ADR 0014). What it looks like with the themes MATE ships is to look at first, before the page is built out.
- **The toolkit's Rust bindings** are a new dependency for the workspace, and `deny.toml` has to pass.

## Resolution

The program is the crate `camino-displays`, the workspace's sixth. Its library is the page's state (`page.rs`) and its client of output management (`outputs.rs`), with no toolkit in either, so that Camino's headless tests drive them against Camino: `camino/src/tests/camino_displays.rs`. The binary is the GTK4 window over them (`window.rs`, `arrangement.rs`, `compositor.rs`, `identify.rs`). The page's own tests, a property test among them, are `camino-displays/src/page/tests.rs`.

Decided here, on 2026-10-05:

- **Identify is layer surfaces of camino-displays**, not a request to Camino. The connection it has for output management, beside GTK's, shows one on the overlay layer in the middle of each output, with the display's name drawn by cairo and pango into a shared-memory buffer, for two seconds. They take no input. Camino stays out of it, and no library for GTK's own layer surfaces is needed: gtk4-layer-shell isn't a dependency.
- **The stock entry is shadowed, not hidden.** `camino-git` installs camino-displays's entry as `/usr/share/camino/xdg/applications/mate-display-properties.desktop`, the capplet's own id, and Camino puts `/usr/share/camino/xdg` first in its session command's `XDG_DATA_DIRS` (`data_dirs` in `camino/src/session.rs`). So the control center and the menus show one *Displays*, which is camino-displays, and X11 MATE and the Wayfire session still find the capplet's. The variable is the command's alone: the bus's activation environment and systemd aren't given it, since systemd's user manager outlives the Session. So a program the bus activates still sees the capplet's entry. No carried patch and no seventh patched package. `mate-control-center` is not a dependency of `camino-git`.
- **GTK4 and MATE's themes:** mate-themes ship no `gtk-4.0` directory, so with any of them the page is in GTK4's own theme, light, whatever MATE's is: with BlackMATE too. A theme that has one, such as Breeze, is followed. The font is MATE's. This is ADR 0014's "less closely", and nothing was done about it.
- **The primary display is written only when the user chose one in the window.** Then it is chosen once Camino shows the configuration, and the one Camino had is chosen again on Revert. With none chosen, Camino's rule stands (81): a display dragged to the left of the others becomes the primary output if no choice was ever made, and the star moves once the configuration is applied. A display that stands in for a primary one that was turned off isn't written either, so the user's choice returns with its monitor.
- **A scale is sent only where it changes,** so one that was Camino's default stays Camino's to choose (58). Reverting from a set scale to one that was Camino's default sends it, and it counts as set from then on, for the Session.
- **A display that is off** says no mode and no scale over output management. The page has it at its native mode and 100%, or at what it showed earlier while the window was open, and turning it on sends that scale.
- **A change from Camino drops what wasn't applied:** a hot-plug, a configuration from `wlr-randr`, another primary output. Hearing again of what Camino showed already drops nothing.
- **Closing the window during the countdown reverts.**
- **Without `org.mate.Camino`,** as under another compositor with output management, the page works with no primary display, and Keep saves nothing and says so.
- **`camino-displays --demo`** shows the page with the prototype's three monitors and changes nothing outside its window. It's how the page was looked at.
- **Dependencies:** gtk4 0.11 and its crates, all MIT. `wayland-client`, `wayland-protocols`, `wayland-protocols-wlr`, `rustix` and `pangocairo` are now declared once in the workspace.

How it was tested, beyond the tests:

- The window was driven with pointer events under Xvfb, in the demo: dragging, each setting, Apply, the countdown running out, Revert, Keep, a display off and on, the primary display alone.
- Camino was run nested in Xvfb, on a private bus and a scratch config, with camino-displays inside it on the one winit output: a scale applied is what `wlr-randr` then lists, Revert and the countdown running out put the old one back, Keep leaves it in `layouts.json`, and Identify shows the label. The page at 150% there was sharp.

Not tested, or not done:

- `cargo deny check`: cargo-deny isn't installed on the developer's machine. The new crates' licences are all in `deny.toml`'s list; the advisories weren't checked. CI runs it.
- Two outputs in a nested run: the winit backend has one, and the headless one is the tests'. The second criterion's two outputs are the headless tests', without the window: a scale and a place, a resolution, a display off and on, a hot-plug, reverting.
- Closing the window during the countdown, and a refresh rate, a display turned off and the primary display on real monitors.
- An enabled output in a custom mode, which is none of its listed ones, is shown at its native mode, and an Apply would set that.
- The first criterion with MATE's themes in a Session, the fourth and the fifth are the developer's, as the checklist's *The page with MATE's theme, at 100% and at a fractional scale*, *camino-displays applies, reverts and keeps a configuration* and *The control center's Displays is camino-displays, under Camino only*.
