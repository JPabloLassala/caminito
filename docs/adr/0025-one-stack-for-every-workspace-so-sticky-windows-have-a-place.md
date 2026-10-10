# 25. One stack for every workspace, so that a sticky window has a place in it

Date: 2026-10-10
Status: Accepted

## Context

Ticket 155 makes sticky windows, which show on every workspace, compositor state. Until then policy kept a stack per workspace, bottom to top, and each window was in its own workspace's. A sticky window has no stack to be in there.

Marco keeps one stack (`MetaStack`) for the screen, and a workspace's stacking is the windows of that stack located on it: a sticky window, or one whose workspace it is (`meta_window_located_on_workspace`, window.c:1346-1351). A sticky window keeps its `workspace`, which unsticking goes back to, though Marco then moves it to the active one. Its geometry is one, so it's at the same place on every workspace, and raising it on one raises it on all.

The ticket offered three models: a sticky window in every workspace's stack, kept in step; one stack for sticky windows, merged into each workspace's at its layer; or one stack with a workspace filter.

## Decision

- **Policy keeps one stack** of every window, bottom to top, and a workspace's stacking is the windows on it, as Marco's is. `Policy::stacking(workspace)` filters the stack; `Policy::stacking_all` gives the whole of it, which XWayland's stacking order is made from.
- **A window is on a workspace** when it's sticky or that's its workspace. A sticky window keeps its workspace, which `workspace_of` still answers and the debug dump shows. Unsticking puts it on the active workspace, as Marco's `window_unstick_impl` does, and moving it to a workspace unsticks it, as `meta_window_change_workspace` does.
- **A transient family is sticky together**, as policy keeps a family on one workspace: sticking or unsticking a window goes from its transient root, and a transient that maps, or becomes one, takes its parent's. Marco sticks a window's transients with it, and a new transient of a sticky parent, but lets a transient alone be sticky.
- **Show Desktop is per workspace still** (94): a sticky window is hidden on the workspace that shows its desktop, which for it is the active one, as in Marco's `meta_window_showing_on_its_workspace`.
- **The work area stays one for every workspace** (64), from every window's struts, so a sticky dock's count on each, as a dock's always did.

## Consequences

- A sticky window's place in the stack is one, as in Marco: raised on one workspace, it's over the windows of the others too.
- Moving windows between workspaces leaves the stacking alone, as in Marco. Before, a window moved by a smaller `num-workspaces` or by dynamic workspaces went above the windows already there; now it keeps its place among them. A window moved by `move-to-workspace-*` or `_NET_WM_DESKTOP` still goes on top of its new workspace's windows.
- Policy's filters over the stack cost a pass over every window where a workspace's stack was at hand; with the windows a desktop has, that's nothing.
- Docks and the desktop aren't sticky by default, though Marco makes them so (window.c:702-711): they stay on the workspace they mapped on, as before, and are sticky when they or the user ask.
- Rejected:
  - a sticky window in every workspace's stack (every move, raise, lower, unmap and change of the count would have to keep the copies in step, and a window could be in two places in a stack);
  - a stack of sticky windows merged into each workspace's at its layer (a sticky window would be over every other window of its layer on each workspace, which no stacking order of Marco's does).
