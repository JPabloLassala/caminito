# 32. Each output has its own workspaces

Date: 2026-10-12
Status: Accepted

## Context

Caminito has one set of workspaces and one active workspace for every output, as Marco has: switching the workspace changes what every output shows. Policy keeps one `active` index, ext-workspace offers one group that holds every output (66), and X11 clients read one `_NET_CURRENT_DESKTOP`.

The user wants to switch one output's workspace and leave the others as they are. Three models go by that name:

- **a set per output**: each output has its own workspaces 1 to N and its own active one, as on macOS ("displays have separate Spaces"), COSMIC and niri;
- **one pool, shared out**: one set of workspaces, each shown on one output at a time and movable between them, as in i3, sway and xmonad;
- **only the primary output switches**: the other outputs keep one fixed workspace, as with GNOME's "workspaces on primary display only".

Marco has none of them, so there is no reference to follow, and X11's desktop properties have room for one current desktop only.

## Decision

- **Each output has its own workspaces and its own active one** (Per-output workspaces). The count, the names and the Workspace grid are the same on every output, from the keys they come from now. With `dynamic-workspaces` the count is one still: a workspace is in use when any output has a window on that number.
- **It is a key, on by default**: `org.mate.Caminito`'s `workspaces-per-output`. Off gives Spanning workspaces, which is Marco's behaviour and what the parity tests check. A change applies at once and no window changes its number: turned on, every output starts on the workspace that was active; turned off, the Active output's workspace becomes the one active workspace.
- **A window is on a workspace of its Main output**, the output it is mostly on. A dragged window joins the other output's active workspace when most of it has crossed, and the part of it on the first output is still drawn. A sticky window is on every workspace of its Main output. A transient family is the exception: all of it is on the workspace of its root's Main output, wherever a dialog of it has been dragged.
- **ADR 0025's one stack stays.** A workspace's stacking is still a filter over it, and what is showing is the windows of every output's active workspace.
- **Each seat has an Active output**: the focused window's Main output. When a switch leaves no window focused it stays that output, until a window on another output is focused or the user clicks on anything on another output. Focus doesn't go to another output's window because a workspace is empty. The workspace bindings act on the Active output, the Workspace popup opens on it, and a new window opens on its active workspace; the `pointer` and `manual` placement modes keep the pointer's output. The Primary output stands in only while a session has no Active output yet.
- **Activating a window that isn't showing switches that window's output**, and no other.
- **The Window switcher lists the windows showing on every output**, and the `switch-*-all` actions every workspace of every output.
- **Show Desktop applies to every output's active workspace**, and each of those workspaces keeps it for itself, as ADR 0025 has it.
- **An unplugged output's windows go to the same-numbered workspaces of the Primary output**, and don't return when the output does. A new or returning output starts on workspace 1.
- **ext-workspace offers a group per output**, each with the output's workspaces.
- **A carried patch makes the panel's pager show and switch the workspaces of its own panel's output.** The window list shows the windows showing on any output, each compared with its own output's active workspace, and every workspace of every output when `display-all-workspaces` is set.
- **X11 clients are told of the Active output.** `_NET_CURRENT_DESKTOP` is its active workspace and changes when the Active output does. A window's `_NET_WM_DESKTOP` is its number within its own output's set. A request to change the current desktop switches the Active output, and a request to move a window moves it within its own output.

## Consequences

- With one output nothing changes, and with the key off Caminito is as before on any number of outputs.
- Caminito's default differs from Marco's on two or more outputs. A user who wants Marco's turns the key off.
- Both behaviours have to be kept working, and every later workspace feature is checked under each.
- A window is hidden or shown by where it is as well as by its workspace: moved across to an output that shows another workspace's number, it stays in view and changes its workspace's output, not its number.
- A dialog dragged to another output hides when its parent's output switches away, though the output it is on hasn't switched.
- An output with no panel has no pager, and is switched with the bindings and the Workspace popup.
- An X11 pager or taskbar is right about the Active output only. The panel doesn't use them.
- An ext-workspace client that takes every group's workspaces as one list, as the unpatched pager does, shows each number once per output.
- The Workspace popup, which opened on the pointer's output, opens on the Active output. The Window switcher's popup stays on the pointer's output: it lists every output's windows, so it belongs to none of them.
- Rejected:
  - one pool shared out (a workspace already showing on another output has to swap or refuse, and `num-workspaces`, the names and the Workspace grid would stop describing what an output has);
  - only the primary output switching (the other outputs aren't independent, only exempt, and their windows can't be put away);
  - no key (Marco's behaviour would be lost, and with it the meaning of the parity tests on more than one output);
  - a count, names or grid per output (MATE's settings hold one of each, and "the workspace to the right" would differ from one output to the next);
  - the pointer's output for the bindings (the output switched would be the one the pointer was left on, not the one being worked on);
  - remembering where an unplugged output's windows were (Caminito keeps no such memory for anything else).
