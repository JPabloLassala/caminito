# Saved layouts

Type: implementation
Status: open
Milestone: M6 (proposed)
Blocked by: 81, 82
Map: [Camino v0.1](../map.md)

## What to build

Camino keeps the Saved layouts itself and restores them, in place of msd's wlrandr plugin and `monitors-wayland.xml` ([ADR 0014](../../../docs/adr/0014-camino-keeps-the-layouts-and-camino-displays-edits-them.md), which supersedes that part of [71](71-mate-settings-daemon-camino.md)).

- **What is saved:** per set of connected monitors, each output's enabled state, mode, place and output scale, and the primary output ([81](81-primary-output.md)).
- **How a monitor is known:** by its EDID make, model and serial, or by its connector when it reports none. Two alike with no serial are told apart by connector.
- **Restoring:** at startup, before the first frame, and on hot-plug when the new set has a saved layout. A saved layout that no longer fits (a mode that's gone) is applied as far as it goes and healed ([82](82-layout-healing.md)).
- **Saving:** only when asked on `org.mate.Camino`. A configuration applied over wlr-output-management lasts for the Session and isn't saved.
- **Set, not default:** a restored scale and place count as set (58), so `window-scaling-factor` doesn't override them (ADR 0008).
- **msd:** a carried patch turns the wlrandr plugin off under Camino. `monitors-wayland.xml` isn't imported.
- **The file:** its place and its form are to decide here. Under `$XDG_CONFIG_HOME/camino/` is the proposal.

## Acceptance criteria

- [ ] Headless tests, written before the code: a layout saved for two monitors is restored when Camino starts with those two; a different set gets the defaults; plugging the second monitor back restores the layout and the primary output; an applied configuration that wasn't saved is gone after a restart; a saved mode that the monitor no longer has leaves a contiguous layout.
- [ ] On the tty with two monitors, a layout saved in one Session is there at the next login from the first frame.
- [ ] msd's wlrandr plugin isn't loaded in the session, and 73's checklist item on `monitors-wayland.xml` is rewritten.
- [ ] ADR 0008's and CONTEXT.md's "msd's saved layout" read "Camino's saved layout".

## Notes

- 60's hot-plug check with a real monitor is still open, and this ticket's hot-plug restore needs the same hardware.
