# Spec: Overview

Written on 2026-10-12 from a grilling session with the developer, whose decisions are in [ADR 0033](../../docs/adr/0033-the-workspace-slide-and-the-overview-are-taken-from-plasma.md). Where this file and the ADR differ, the ADR is right and this file is to be fixed.

Nothing here is built. Its tickets are 241 to 247, under "Tickets". It builds on the [Workspace slide](../workspace-slide/spec.md), which comes first.

## Problem

To find a window the user has the Window switcher, which lists windows in order of use and closes when the modifier is released, and the window list's buttons. Neither shows every window of a workspace at once at a size that tells them apart, and neither moves a window to another workspace by dragging it.

## Solution

The **Overview** (CONTEXT.md): a view of each output's active workspace with its windows shrunk and set side by side, none over another, and a bar of the workspaces along the top. A window is clicked to activate it, or dragged onto a workspace of the bar to move it there. It opens and closes with each window moving between its real place and its place in the view. It is Plasma's Overview, without its search field.

## User stories

1. As a user, I want one shortcut to show every window of the workspace side by side, so that I can pick one by sight.
2. As a user, I want to click a window to go to it.
3. As a user, I want to pick a window with the arrow keys and Enter.
4. As a user, I want Esc, the shortcut again or a click on the empty area to put everything back.
5. As a user, I want to see the windows move to their places and back, so that I know which is which.
6. As a user, I want my minimised windows there too.
7. As a user, I want to drag a window onto another workspace to move it there.
8. As a user with two monitors, I want the Overview on both, and to drag a window from one to the other.
9. As a user, I want to click a workspace in the bar to see its windows, without leaving the Overview.
10. As a user, I want to close a window with a middle click.
11. As a user, I want the windows to keep drawing while I look at them.
12. As a user who has animations turned off, I want the Overview to open at once, with no movement.

## Implementation decisions

### Opening and closing

- **A System shortcut opens it**, Super+W by default, kept in `org.mate.Caminito` and listed in System Shortcuts. If Marco's or mate-settings-daemon's defaults have Super+W, it is Super+Tab. There is no hot corner, gesture or panel button.
- It is an **Interaction**. The shortcut does nothing while the seat has another, and none opens while the Overview is open. The session lock ends it.
- Pressed during a Workspace slide, it opens at once and the slide is cut to its end.
- It opens on a workspace with no windows, showing the bar alone, and on one with a focused fullscreen window, which is laid out like any other.
- It closes on Esc, the shortcut again, a click on the empty area, or a window being chosen.
- Removing the shortcut is how the Overview is turned off.

### Outputs

- It opens on every output at once. Each shows its own active workspace's windows and its own bar.
- Under Spanning workspaces every output's bar shows the same workspaces, and switching in one switches all.

### Which windows

- The windows the Window switcher lists for one workspace: minimised ones are there, and skip-taskbar ones aren't.
- A sticky window is in every workspace of its output.
- A window that opens while the Overview is open joins the layout, and one that closes leaves it.

### The layout

- Rows of windows, each keeping its aspect and its size relative to the others, none enlarged, none over another.
- The order follows the windows' real places, so a window on the left of the output is on the left of the layout.
- Each has its title and its icon beneath it.
- The layout is a pure function of the windows' rectangles and the area, as the Window switcher's is (`caminito/src/switcher/layout.rs`).

### The bar

- One row along the top of the output, in workspace order, whatever the Workspace grid's shape.
- Each cell is a small live picture of that workspace on that output, with its wallpaper. The active workspace is marked.
- It has no buttons to add or remove a workspace. The count is `num-workspaces`'s or `dynamic-workspaces`'s.

### The rest of the output

- The panel and caja's desktop fade out while it is open.
- The wallpaper stays, dimmed. There is no blur.

### The pointer

- **A click on a window** activates it and closes the Overview. A minimised window is unminimised.
- **A middle click on a window** closes that window. The Overview stays open.
- **A click on a workspace of the bar** switches that output to it. The Overview stays open, and the Workspace slide moves the laid-out windows.
- **A click on the empty area** closes the Overview.
- **A drag of a window**, past `drag-threshold`:
  - onto a workspace of the same output's bar: the window goes to that workspace and keeps its geometry;
  - onto another output, on its layout or on a workspace of its bar: the window goes to that output's active or chosen workspace, at the same place relative to the work area, shrunk to fit if it must be. A maximised or tiled window keeps that state there;
  - onto nothing: it returns to its place in the layout.

  The Overview stays open after any of them.

### The keys

- The arrow keys move the selection, Enter chooses it, and Esc closes.
- The focused window is selected at opening, or none when no window is focused.
- **The workspace bindings switch the Active output's workspace**, with the Workspace slide and without the Workspace popup, since the seat already has an Interaction.
- Media keys work.
- Every other key and System shortcut is swallowed, Alt+Tab among them.

### The transition

- 250 ms with an ease-out curve, as the Workspace slide's.
- **Opening:** each window moves and shrinks from its real place to its place in the layout. A minimised window fades in at its place. The bar and the dimming fade in, and the panel and caja's desktop fade out.
- **Closing:** every window moves back to its real place at once. A chosen window is raised as they go. A minimised window that wasn't chosen fades out.
- The shortcut pressed again part-way reverses the transition from where it is.
- Policy has the Overview open or closed from the first frame, and only the drawing takes time.

### Live content

- The windows in the layout and in the bar's cells are live: clients keep drawing and keep getting frame callbacks.
- A window that only a bar's cell shows gets them at a reduced rate.

### Where its state lives

- **Policy** (ADRs 0010, 0016, 0026) has whether it is open, the windows listed for each output, the selection, the drag and what a choice or a drop does.
- **The compositor crate** has the layout, the bar, the drawing and the transition, and tells policy of keys and clicks.
- Caminito draws it in the scene, with cairo for its chrome in the GTK theme's colours and font (ADR 0003).

### Settings

- `overview-animation` in `org.mate.Caminito`, a boolean, default `true`. It turns off the transition, not the Overview.
- The transition runs when that key is on and the minimise animation's three are as it needs them: `compositing-manager` on, `reduced-resources` off and `org.mate.interface enable-animations` on. Without it the Overview opens and closes in one frame.
- The key for the shortcut, whose name is the ticket's.

## Testing

- Policy's tests cover the windows listed, the selection, the keys, each click and each drop, with no protocol objects.
- The layout's tests are unit tests of the pure function.
- Headless tests open it through the fixture, read the listed windows, their rectangles and the bar from the debug dump, and advance Caminito's clock to read the transition part-way, reversed and at its end. They run with one output and with two, under Per-output workspaces and under Spanning workspaces.
- [checklist.md](../../docs/testing/checklist.md) gets an item for a real Session, which is the developer's to run.

## Not decided

- **Whether Super+W is free** in Marco's and mate-settings-daemon's defaults, which ticket 241 checks.

The developer gave two start values on 2026-10-13, which ticket 247 checks in a Session:

- **The reduced rate** of frame callbacks for a window only the bar shows is 10 a second.
- **The size of the bar** is 15% of the output's height. Each cell has the output's aspect, and when the cells are wider than the output together they are all made smaller by the same amount to fit.

## Tickets

Seven slices, filed in `.scratch/issues/` as tickets 241 to 247.

| | Slice | Blocked by |
| --- | --- | --- |
| 241 | [**The Overview opens and closes.**](../issues/241-the-overview-opens-and-closes.md) | None |
| 242 | [**The Overview's keys, the middle click and windows that come and go.**](../issues/242-the-overviews-keys-the-middle-click-and-windows-that-come-and-go.md) | 241 |
| 243 | [**The Overview's transition.**](../issues/243-the-overviews-transition.md) | 241 |
| 244 | [**The Overview's bar of workspaces.**](../issues/244-the-overviews-bar-of-workspaces.md) | 241 |
| 245 | [**A window is dragged onto a workspace of the Overview's bar.**](../issues/245-a-window-is-dragged-onto-a-workspace-of-the-overviews-bar.md) | 244 |
| 246 | [**The Overview on two outputs.**](../issues/246-the-overview-on-two-outputs.md) | 244, 245 |
| 247 | [**The Overview in a real Session, by hand.**](../issues/247-the-overview-in-a-real-session.md) | 242, 243, 245, 246 |
