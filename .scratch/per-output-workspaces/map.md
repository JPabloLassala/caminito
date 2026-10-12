# Map: Per-output workspaces

Status: closed on 2026-10-12. It reached its destination, [the spec](spec.md), in one grilling session with the developer.

## Destination

A spec, cut into implementation tickets, for workspaces that each output switches by itself, in place of Marco's one active workspace for every output.

## Notes

- **Domain:** **Per-output workspaces**, **Spanning workspaces** and **Active output** are in [CONTEXT.md](../../CONTEXT.md), and **Primary output** changed with them. The model and what was rejected are [ADR 0032](../../docs/adr/0032-each-output-has-its-own-workspaces.md).
- **What the code assumes today**, from an exploration on the same day:
  - policy has one `active` index (`caminito-policy/src/policy.rs`), read about 28 times in policy and at 10 `active_workspace()` call sites in the compositor, and about 216 test references in 26 files go through it;
  - policy stores no output for a window, and derives it as the output the frame is most on (`output_of`);
  - actions carry no output, and there was no notion of a current one: popups went by the pointer's output and placement by the focused window's;
  - ext-workspace offers one group that holds every output, with bare numbers as ids, and the Xfce toplevel protocol assumes one handle per id per client;
  - the panel's Wayland pager lists every group's workspaces flat, and the patched window list compares a window with its own group's active workspace;
  - a panel can be on any output (ticket 209);
  - an unplugged output's windows go to the Primary output, and nothing returns them.

## Decisions so far

From the grilling session of 2026-10-12:

- **A set per output**, not one pool shared out and not only the primary switching. One count, one set of names, one Workspace grid; dynamic workspaces keep one count.
- **A key, on by default**: `workspaces-per-output` in `org.mate.Caminito`. Off is Spanning workspaces. A change applies at once and no window changes its number.
- **A window is on a workspace of its Main output.** Dragged across, it joins the other output's active workspace when most of it has crossed, and is drawn across the boundary. A sticky window is on every workspace of its output. A transient family stays on its root's workspace.
- **The Active output** is per seat. It is the focused window's Main output, stays when a switch leaves nothing focused, and changes on focus elsewhere or any click on another output. Bindings, the Workspace popup, new windows and X11's current desktop go by it. Focus doesn't jump outputs because a workspace is empty.
- **Activating a window that isn't showing** switches that window's own output.
- **The Window switcher** lists the windows showing on every output, and its popup stays on the pointer's output.
- **Show Desktop** applies to every output's active workspace.
- **An unplugged output's windows** go to the same-numbered workspaces of the Primary output and don't return. A new or returning output starts on workspace 1.
- **A pager** shows and switches its own panel's output, through a carried patch. **A window list** shows the windows showing on any output.
- **ext-workspace** offers a group per output.
- **X11** clients see the Active output's workspace as the current desktop, and a window's number within its own output.
- **The epic is `per-output-workspaces`**, tickets 221 to 231.

## Not yet specified

The spec's "Not decided": two points left to tickets 226 and 227.
