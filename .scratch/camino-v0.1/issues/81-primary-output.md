# The primary output

Type: implementation
Status: open
Milestone: M6 (proposed)
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

Camino has no Primary output: wlr-output-management carries none, and nothing in a Session agrees on which output the panel and new windows prefer. [ADR 0014](../../../docs/adr/0014-camino-keeps-the-layouts-and-camino-displays-edits-them.md) gives it to Camino (decided 2026-10-05).

- **The setting:** one enabled output is the primary output. It is read and set on `org.mate.Camino` (ADR 0002's third channel), with a signal when it changes. The interface's shape is to decide here.
- **What follows it:**
  - mate-panel and caja's desktop go to it. How they learn of it is to decide here: the order of the `wl_output` globals, the RandR primary through GDK, or a carried patch that asks `org.mate.Camino`.
  - A new window with nothing better to go by opens on it.
  - XWayland's RandR primary is it.
- **The stand-in:** when the primary output is unplugged or disabled, the built-in panel stands in, else the leftmost output. The user's choice returns when its monitor does.
- **With no choice made,** the primary output is the built-in panel, else the leftmost output.
- **The rename:** the code's "primary output", the output a surface is mostly on, becomes Main output (CONTEXT.md), in the code and in comments. Old tickets keep their wording.

## Acceptance criteria

- [ ] Headless tests, written before the code: the primary output with no choice made; a choice set over D-Bus is read back and signalled; unplugging it puts the stand-in in, and plugging it back returns the choice; a new window opens on it.
- [ ] Under XWayland, `xrandr` shows the chosen output as primary, and shows the change.
- [ ] In the session with two monitors, changing the primary output moves the panel and the desktop icons.
- [ ] No "primary output" in the code means the output a surface is mostly on.

## Notes

- Keeping the choice across Sessions is [83](83-saved-layouts.md)'s. Here it lasts for the Session.
- The prototype that settled the design is `.scratch/camino-displays-prototype/index.html` on the branch `prototype/camino-displays` (8a4981d), with its verdicts in the commit message. It is a source, not code to keep.
