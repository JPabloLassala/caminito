# 40. A workspace switch is the pointer's output's

Date: 2026-10-23
Status: Accepted

## Context

ADR 0032 has the workspace bindings act on the Active output, the focused window's Main output, and rejected the pointer's output for them: the output switched would be the one the pointer was left on, not the one being worked on.

In use it goes the other way. With a window focused on one output, the user points at the other output and presses `switch-to-workspace-right`, and the output with the focused window switches, the one they weren't looking at. To switch the output they're looking at, they have to click on it first.

## Decision

- **A binding that switches the workspace acts on the pointer's output:** `switch-to-workspace-left`, `-right`, `-up`, `-down`, `-1` to `-12` and `-prev`, from a key or a mouse button. The pointer's output becomes the seat's Active output first, as a press there makes it, so the focus goes to its default on that output, and the Workspace popup opens there.
- **An open Interaction keeps its output.** A step of the open Workspace popup stays on the output the popup is on, and the Overview's switches stay on its selection's output, wherever the pointer is.
- **The `move-to-workspace-*` bindings still act on the focused window's output.** They move that window, so its output is the one that switches.
- **Moving the pointer alone still changes nothing.** The Active output changes when a switch binding runs, not when the pointer crosses to another output.
- **The other switches stay as ADR 0032 has them.** An X client's `_NET_CURRENT_DESKTOP` request switches the Active output, and ext-workspace's `activate` its group's output.

## Consequences

- The focused window loses the focus when the user switches the other output, as it does when they click there and switch.
- A new window opens on the output last switched, since it is the Active output.
- With one output, or with `workspaces-per-output` off, nothing changes.
- Rejected:
  - the Active output staying as it is (ADR 0032's rule, which this replaces for the switch bindings);
  - the pointer's output for the switch without making it the Active output (the Workspace popup, the focus and a new window would be on another output than the one that switched);
  - the pointer's output for the moves too (the window moved isn't on it).
