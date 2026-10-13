# Spec: Per-output workspaces

Written on 2026-10-12 from a grilling session with the developer, whose decisions are in [the map](map.md) and in [ADR 0032](../../docs/adr/0032-each-output-has-its-own-workspaces.md). Where this file and the ADR differ, the ADR is right and this file is to be fixed.

Nothing here is built.

## Problem

Caminito has one active workspace for every output, as Marco has. With two monitors, switching the workspace changes both, so a window kept in view on one monitor has to be made sticky or moved along by hand. The developer wants to switch one output's workspace and leave the others as they are.

## Solution

**Per-output workspaces** (CONTEXT.md): each output has its own workspaces 1 to N and its own active one. The count, the names and the **Workspace grid** are the same on every output. A window is on a workspace of its **Main output**. The workspace bindings act on the seat's **Active output**. It is Caminito's default, and a key turns it off for **Spanning workspaces**, which is what Caminito does today.

## User stories

1. As a user with two monitors, I want to switch the workspace of the monitor I'm working on and have the other stay as it is.
2. As a user, I want every monitor to have the same workspaces, with the same names in the same grid, so that "workspace 3" and "the workspace to the right" mean one thing.
3. As a user, I want the switch binding to act on the monitor my focused window is on, not the one the pointer was left on.
4. As a user who has switched a monitor to an empty workspace, I want the next binding and the next window I open to go to that monitor still.
5. As a user, I want to click on a monitor's panel, desktop or wallpaper to say that monitor is the one I mean.
6. As a user, I want a window I drag to the other monitor to stay in view there, and to belong to what that monitor shows.
7. As a user, I want a dialog to stay with its window when the window's workspace is put away, wherever I dragged the dialog.
8. As a user, I want a sticky window to stay on its monitor through that monitor's switches.
9. As a user, I want Alt+Tab to reach every window I can see, on any monitor.
10. As a user, I want a window I pick from the window list to come into view on its own monitor, and nothing else to move.
11. As a user, I want Show Desktop to clear every monitor.
12. As a user, I want the pager on a monitor's panel to show and switch that monitor's workspaces.
13. As a user with one panel, I want its window list to have a button for every window I can see, on any monitor.
14. As a user who unplugs a monitor, I want its windows to keep their workspace numbers on the monitor that takes them, and not to flood the workspace I'm looking at.
15. As a user who wants Marco's workspaces, I want one key to give them back, at once.
16. As a user with one monitor, I want nothing to change.

## Implementation decisions

### The key

`workspaces-per-output` in `org.mate.Caminito` (`data/org.mate.Caminito.gschema.xml`), a boolean, default `true`, read by the settings thread and translated like the others.

A change applies at once, and no window changes its workspace number:

- turned on, every output's active workspace is the one that was active;
- turned off, the Active output's active workspace becomes the one active workspace, so the other outputs may change what they show.

With the key off, everything below this heading is as it is today. The parity tests run with it off.

### The model

- **Each output has an active workspace** and a previous one (for `switch-to-workspace-prev`). `num-workspaces`, the names and the three `workspace-layout-*` keys stay single values, used for every output.
- **A window's workspace is a number and its output is its Main output**, the output its frame is most on, which policy derives already (`output_of`). Policy stores no output for a window.
- **A window is showing** when its Main output's active workspace is its workspace, or it is sticky, and it isn't hidden for another reason. What the scene draws, what input hits and what Xwayland's stacking puts on top are the windows showing on every output.
- **A window that straddles two outputs** is drawn on both while it is showing. An output clips; it doesn't select.
- **A window dragged across** changes its Main output when most of it has crossed. Its number stays. If the new output shows another number, the window takes the new output's active workspace, so it stays in view. `move-to-monitor-*` does the same.
- **A sticky window** is on every workspace of its Main output.
- **A transient family** is on the workspace of its root's Main output, all of it, wherever a member has been dragged.
- **ADR 0025's one stack stays**, with a workspace's stacking a filter over it.
- **The work area** stays one per output for every workspace.

### The Active output

Each seat has one, as each seat has a focus, and as on macOS.

- It is the focused window's Main output.
- When a switch leaves no window focused, it stays that output. Focus doesn't go to a window on another output because a workspace is empty.
- It changes when a window on another output is focused, or when the user clicks on anything on another output: a window, a panel, caja's desktop or the wallpaper. Moving the pointer doesn't change it.
- While a session has none yet, the Primary output stands in.

It decides:

- which output the `switch-to-workspace-*` bindings switch, and which output `rename-workspace` reads the active workspace from (the names are one set);
- where the Workspace popup opens, and whose workspaces and windows it shows;
- where a new window opens: on the Active output's active workspace. The `pointer` and `manual` placement modes keep the pointer's output;
- what X11 clients are told (below).

### Moving and activating

- `move-to-workspace-N` and the beside moves move a window within its own output's workspaces. The beside moves switch that output along with the window, as now.
- The Window menu's workspace items do the same, relative to the window's output's active workspace.
- **Activating a window that isn't showing** switches that window's output to its workspace, and no other output. This is the window list's click, a client's activation request and wlr-foreign-toplevel's `activate`.
- Unsticking a window puts it on its output's active workspace.

### The Window switcher and Show Desktop

- `switch-windows`, `switch-group` and `switch-panels` list the windows showing on every output. The `-all` actions list every workspace of every output.
- The switcher's popup opens on the pointer's output, as now, and not on the Active output.
- A workspace switch on one output closes the switcher and the Window menu, as now.
- **Show Desktop** applies to every output's active workspace. Each of those workspaces keeps it for itself (ADR 0025): switching an output away shows the other workspace's windows, and switching back shows the desktop again. `ShowingDesktop` on `org.mate.Caminito` is true while every output's active workspace shows its desktop.

### Outputs coming and going

- **An output goes**: its windows go to the same-numbered workspaces of the Primary output. They don't return when the output does.
- **An output appears**, new or returning: it starts on workspace 1.
- **A smaller `num-workspaces`** clamps every output's active workspace and every window, as it clamps the one now.
- **Dynamic workspaces** keep one count. A workspace is in use when any output has a window on that number, and an empty one in the middle goes only when it is empty on every output. The renumbering applies to every output's active and previous workspace together.

### ext-workspace and the toplevel protocols

- ext-workspace offers **a group per output**, each holding that output's workspaces, with the same names and coordinates, and `active` per group. With the key off it offers the one group it offers now.
- A workspace's id has to name its output as well as its number, since the Xfce toplevel protocol (ADR 0013) resolves a toplevel's workspace to one handle by id per client.
- A toplevel's Xfce handle enters the workspace of its window's output's group.
- wlr-foreign-toplevel is unchanged.

### The panel

- **The pager** shows and switches the workspaces of the output its panel is on, through a carried patch to mate-panel's Wayland pager (`applets/wncklet/wayland-workspace.c`), patch `0012`, which listed every group's workspaces flat before. The pager takes its output from its panel's layer surface, the one the panel asked for, and shows the group that holds it. It has no gate of its own (ADR 0011): only under Caminito does the panel name its output, and a compositor with one group gives the same result as before.
- **The window list** shows the windows showing on any output: each window compared with its own group's active workspace, which patch `0003` nearly does already. `display-all-workspaces` lists every workspace of every output.
- An output with no panel has no pager.

### X11

- `_NET_CURRENT_DESKTOP` is the Active output's active workspace, and is written again when the Active output changes.
- A window's `_NET_WM_DESKTOP` is its number within its own output's workspaces, or `0xFFFFFFFF` when sticky.
- A request to change `_NET_CURRENT_DESKTOP` switches the Active output. A request to change a window's `_NET_WM_DESKTOP` moves it within its own output.
- Xwayland's stacking keeps the windows that aren't showing below those that are.

## Testing

- Policy's unit and property tests cover the model with the key on, and the existing ones keep running with it off. The `SCENE_QUERIES` invariant (`caminito-policy/src/tests/invariants.rs`) names `active_workspace` and `stacking`, and changes shape with them.
- Headless tests with two outputs cover each ticket's behaviour end to end.
- [checklist.md](../../docs/testing/checklist.md) gets the items that need two real monitors and the panel, run in ticket 229.

## Not decided

- **The ext-workspace ids**, which ticket 226 settles.
- **How the pager learns its panel's output**, which ticket 227 settled: see "The panel".

## Tickets

Eleven slices, filed in `.scratch/issues/` as tickets 221 to 231. The first three were one ticket, split so that the change of policy's shape lands by itself with no change of behaviour. Each is run by itself once the ones it's blocked by are in.

| | Slice | Blocked by |
| --- | --- | --- |
| 221 | [**Policy keeps an active workspace for each output.**](../issues/221-policy-keeps-an-active-workspace-for-each-output.md) |  |
| 230 | [**A window shows by its Main output's workspace.**](../issues/230-a-window-shows-by-its-main-outputs-workspace.md) | 221 |
| 231 | [**The workspace bindings switch the Active output.**](../issues/231-the-workspace-bindings-switch-the-active-output.md) | 230 |
| 222 | [**The Active output follows clicks, and new windows open on it.**](../issues/222-the-active-output-follows-clicks-and-new-windows-open-on-it.md) | 231 |
| 223 | [**Moves, sticky windows, families and activation, per output.**](../issues/223-moves-sticky-windows-families-and-activation-per-output.md) | 230 |
| 224 | [**The Window switcher and Show Desktop across outputs.**](../issues/224-the-window-switcher-and-show-desktop-across-outputs.md) | 230 |
| 225 | [**Outputs coming and going, and the workspace count.**](../issues/225-outputs-coming-and-going-and-the-workspace-count.md) | 231 |
| 226 | [**ext-workspace offers a group per output.**](../issues/226-ext-workspace-offers-a-group-per-output.md) | 230 |
| 227 | [**The pager shows its own output's workspaces.**](../issues/227-the-pager-shows-its-own-outputs-workspaces.md) | 226 |
| 228 | [**X11 clients are told of the Active output.**](../issues/228-x11-clients-are-told-of-the-active-output.md) | 222 |
| 240 | [**The pager shows two pressed buttons after a switch down.**](../issues/240-the-pager-shows-two-pressed-buttons-after-a-switch-down.md), found by 227's check |  |
| 229 | [**Per-output workspaces in a real Session, by hand.**](../issues/229-per-output-workspaces-in-a-real-session.md) | 222, 223, 224, 225, 227, 228, 240 |
