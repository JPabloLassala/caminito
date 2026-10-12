# Spec: Workspace slide

Written on 2026-10-12 from a grilling session with the developer, whose decisions are in [ADR 0033](../../docs/adr/0033-the-workspace-slide-and-the-overview-are-taken-from-plasma.md). Where this file and the ADR differ, the ADR is right and this file is to be fixed.

Nothing here is built.

## Problem

A change of workspace is a cut: one frame shows the old workspace's windows and the next shows the new one's. Nothing says which way the user went in the Workspace grid, or that the windows are still there beside the ones in view.

## Solution

The **Workspace slide** (CONTEXT.md): on a change of an output's active workspace, the old workspace's windows leave and the new one's arrive, along the line between the two in the **Workspace grid**. Going to the workspace on the right, the windows move to the left. It is Plasma's Slide effect.

## User stories

1. As a user, I want the windows to slide out and the next workspace's to slide in when I switch, so that I see which way I went.
2. As a user with workspaces in rows, I want a switch up or down to slide up or down.
3. As a user who jumps to a workspace by number, I want one straight movement to it, not a tour of the ones between.
4. As a user who holds the binding down, I want one continuous movement across the workspaces, not a series of jerks.
5. As a user, I want my sticky windows, the panel, the desktop icons and the wallpaper to stay still.
6. As a user who carries a window to the next workspace, I want that window to stay under my eyes while the rest move.
7. As a user with two monitors, I want only the monitor that switches to move, and no window to cross into the other.
8. As a user, I want to type into the new workspace's window at once, without waiting for the slide.
9. As a user who has animations turned off, I want no slide.
10. As a user who wants Marco's cut, I want one key to give it back.

## Implementation decisions

### What slides

- The windows of the old workspace and of the new one, with their Frames, shadows and popups.
- **What stays still:** sticky windows, Layer surfaces (the panel, caja's desktop, notification bubbles), the wallpaper, the window carried by a `move-to-workspace-*` binding, and a window being dragged at the time of the switch. The others slide beneath a window that stays.
- A focused fullscreen window slides like any other, in the place the Scene gives it, so the panel shows as it leaves.
- The Workspace popup is drawn over the sliding windows, as now.

### Direction and distance

- The slide follows the grid: from the old workspace's cell to the new one's, one output's width for each column and one output's height for each row. A jump is one straight line, diagonal if the cells differ in both, and no workspace between the two is drawn.
- A wrap under `wrap-style` slides in the direction of the key pressed, by one cell, not back across the grid.
- There is no gap between workspaces.

### Which switches slide

Every change of an output's active workspace, whatever its cause: the bindings, the panel's pager, activating a window that isn't showing, an ext-workspace or X11 request, the Workspace popup, the Window menu's moves, and dynamic workspaces removing a workspace.

These don't slide:

- `workspaces-per-output` turned on or off;
- an output that appears, or one that takes another's windows when it goes.

### Outputs

- A sliding window is clipped to the output that is switching. It never shows on another output during the slide. A window that straddles two outputs keeps being drawn on the other at its place, as ADR 0032 has it.
- Under Spanning workspaces every output slides its own windows, at the same time.

### Timing

- 250 ms, with an ease-out curve, drawn on every frame of the output and not in the minimise animation's 15 ms steps. It is read from Caminito's clock.
- **A second switch during a slide retargets it** from where the windows are, so the movement is continuous. The workspaces passed over are drawn as they go by.
- There are no keys for the length or the curve.

### Policy and input

- Policy switches at once, as it does for the minimise animation. Focus, keys, the Active output, ext-workspace, the toplevel protocols and X11's properties change at the first frame.
- Input follows policy: a click during the slide reaches the new workspace's windows at their final places, not where they are drawn.
- The session lock ends a slide at once.
- The switch's sounds (170) are unchanged.

### Settings

- `workspace-slide` in `org.mate.Caminito` (`data/org.mate.Caminito.gschema.xml`), a boolean, default `true`.
- The slide runs when that key is on and the minimise animation's three are as it needs them: `compositing-manager` on, `reduced-resources` off and `org.mate.interface enable-animations` on.
- A change applies to the next switch.

## Testing

- Headless tests advance Caminito's clock, as the minimise animation's do (`caminito/src/tests/minimise_animation.rs`), and read each window's drawn rectangle and the slide's offset from the debug dump, part-way and at the end.
- They cover: each direction, a jump, a wrap, a retarget, the windows that stay still, two outputs under Per-output workspaces and under Spanning workspaces, each key off, and input reaching the new workspace part-way.
- [checklist.md](../../docs/testing/checklist.md) gets an item for a real Session, which is the developer's to run.

## Not decided

- **The curve's formula.** "Ease-out" is settled; which one is the ticket's.

## Tickets

Five slices, filed in `.scratch/issues/` as tickets 233 to 237. The first waits on per-output workspaces' ticket 230, which changes what the scene draws for each output.

| | Slice | Blocked by |
| --- | --- | --- |
| 233 | [**A workspace switch slides.**](../issues/233-a-workspace-switch-slides.md) | 230 |
| 234 | [**Windows that stay still through a slide.**](../issues/234-windows-that-stay-still-through-a-slide.md) | 233 |
| 235 | [**A second switch retargets the slide.**](../issues/235-a-second-switch-retargets-the-slide.md) | 233 |
| 236 | [**The slide on two outputs.**](../issues/236-the-slide-on-two-outputs.md) | 233, 231, 225 |
| 237 | [**The Workspace slide in a real Session, by hand.**](../issues/237-the-workspace-slide-in-a-real-session.md) | 234, 235, 236 |
