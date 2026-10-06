# The desktop icons don't come back with the primary output

Type: implementation
Status: open
Milestone: M6 (proposed)
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

When the primary output is unplugged and plugged back, the panels come back to it and caja's desktop doesn't. The checklist item *The primary output moves the panel, the desktop icons and RandR's primary* expects both to come back with the monitor ([81](81-primary-output.md)). [60](60-tty-checks-left-over.md) saw it on 2026-10-06, in the developer's Session on `camino-git r195.9e4baac-1` with `caja-camino 1.28.0-1` and `mate-panel-camino 1.28.7.r111.g58ac6c1-3`. The primary output was HDMI-A-1, the LG, chosen in the saved layout ([83](83-saved-layouts.md)).

- **Unplugging went as expected:** 5 s after HDMI-A-1 was unplugged, DP-1 was alone at 0,0 with both panels and the `desktop` layer surface on it.
- **Plugging back didn't:** 5 s after, `primary_output` was HDMI-A-1 again, at 0,0, with both panels. The `desktop` layer surface was still on DP-1, at 2194,0, and 2194x1234, which is HDMI-A-1's logical size and not DP-1's 1920x1080. It was the same 10 minutes later.
- **Not known:** whether caja heard of the primary output again. The restore of a saved layout makes the primary output a choice once more (83), after the output is added; whether `PrimaryOutput`'s change is signalled then, and whether caja's carried patch follows a name that is the same as before the unplug, are the first things to look at. The size says caja did resize the surface for HDMI-A-1 and didn't move it.

## Acceptance criteria

- [ ] The cause is written down in this ticket's notes, with a headless test written before the fix where the cause is Camino's.
- [ ] After the primary output is unplugged and plugged back, the `desktop` layer surface is on it, at its size, as the panels are.
