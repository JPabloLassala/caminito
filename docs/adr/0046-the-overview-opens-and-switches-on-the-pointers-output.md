# 46. The Overview opens and switches on the pointer's output

Date: 2026-10-23
Status: Accepted

## Context

ADR 0040 has a workspace switch binding act on the pointer's output, but keeps the Overview's switches on its selection's output wherever the pointer is. The Overview's shortcut and the Application overview's open with the selection on the focused window's output.

In use the Overview has the same problem ADR 0040 fixed outside it. With the focused window on one output, the user points at the other output and opens the Overview, and the selection is on the output they weren't looking at. Pressing a switch binding in the Overview then switches that output too.

## Decision

- **The Overview's shortcut and the Application overview's open with the selection on the pointer's output**, which becomes the Active output, as a Hot corner opens them on its own output. The focused window is selected only if it's listed there. The Application overview still lists the focused window's Application.
- **A switch binding in the open Overview or Application overview acts on the pointer's output.** The selection goes there first, with nothing selected, as a click on that output's bar takes it, and the switch is that output's.
- **The Workspace popup keeps its output**, as ADR 0040 has it: a step of the open popup stays on the output the popup is on.
- **Moving the pointer alone still changes nothing**, in the Overview or out of it.

## Consequences

- With the pointer on another output than the focused window's, the Overview opens with nothing selected there: Enter chooses nothing until an arrow key selects that layout's first window.
- The arrow keys move the selection within the selection's output's layout, as before, wherever the pointer is.
- With one output, nothing changes. With `workspaces-per-output` off, a switch still switches every output, and only the selection moves.
- Rejected:
  - the selection staying on its output for the switch bindings (ADR 0040's rule, which this replaces for the Overview);
  - opening on the focused window's output and following the pointer only for the switches (the selection would jump on the first switch).
