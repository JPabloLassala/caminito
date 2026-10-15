# Spec: Opening and Closing animations

Written on 2026-10-14 from a grilling session with the developer, whose decisions are in [ADR 0034](../../docs/adr/0034-the-opening-and-closing-animations-are-taken-from-plasma.md). Where this file and the ADR differ, the ADR is right and this file is to be fixed.

Nothing is built yet. Its tickets are 251 to 254, under "Tickets".

## Problem

A window that opens is there in one frame, and one that closes is gone in the next. Nothing draws the eye to a window that appears, and a window that vanishes leaves the user wondering whether it closed or went behind another.

## Solution

The **Opening animation** and the **Closing animation** (CONTEXT.md): a window grows into its place and fades in as it opens, and shrinks and fades out from its place as it closes. It is Plasma's Scale effect.

## User stories

1. As a user, I want a window that opens to grow into its place, so that I see where it came.
2. As a user, I want a window that closes to shrink and fade, so that I see it went and didn't hide behind another.
3. As a user, I want to type into a new window at once, without waiting for it to finish growing.
4. As a user, I want menus, tooltips, the panel and the desktop to appear as they do now.
5. As a user who closes a window just after opening it, I want no jump.
6. As a user who has animations turned off, I want none.
7. As a user who wants Marco's way, I want one key to give it back.

## Implementation decisions

### What opens and what closes

- **A window opens** when it is shown for the first time, or again after its client hid it (an X11 client that unmaps and maps the same window, say).
- **A window closes** when, while it's on screen, its client destroys it or hides it.
- **Neither:** minimising and unminimising (the minimise animation is Marco's and stays), a move to another workspace (the Workspace slide's), and a window that opens or closes on a workspace no output shows.

### Which windows

- A window with a Frame, whatever its type, and a window of type Normal, Dialog or ModalDialog. This is Plasma's rule (`isScaleWindow`).
- Never: an override-redirect window, a popup, a Layer surface (the panel, caja's desktop, notification bubbles), and windows of type Desktop, Dock, Toolbar, Menu, Utility or Splashscreen without a Frame.
- A fullscreen window animates like any other.

### Timing and shape

- 200 ms, drawn on every frame of the output from Caminito's clock, as the slide is, with a deadline (`caminito/src/deadlines.rs`).
- **Opening:** from 0.8 of the window's size to its own, and from no opacity to full, on a cubic ease-out.
- **Closing:** from its own size to 0.8 of it, and from full opacity to none, on a cubic ease-in.
- The window scales about its centre, with its Frame, shadow and popups.
- There are no keys for the length or the curve.

### Policy and input

- Policy changes at once, as for the minimise animation and the slide. An opening window is mapped, focused as policy has it and given input from its first frame, at its real place; a click on it part-way reaches it there, whatever size it is drawn at.
- A closing window is gone from policy at once and its focus has moved on. Only its picture is drawn, and a click goes through it to what is beneath.

### The closing window's picture

- It is the window's last frame, with its Frame and shadow, drawn at the window's place in the stack: a window that was above it stays above it, and the window beneath shows through as it fades.
- When there is no picture to keep (the client's buffer is already gone), the window closes at once, and nothing is logged above debug level.
- It belongs to the window's workspace and output, and is drawn only where the window would have been.

### Interruptions

Each starts from what is on screen at the time, so nothing jumps.

- **A window closes while it opens:** it closes from the size and opacity it had reached.
- **A window is shown again while its picture fades:** the picture is dropped and the window opens from the picture's size and opacity.
- **A window is minimised while it opens:** the minimise animation takes it from its real place, as now.
- **A window is maximised, resized or moved while it opens:** it grows towards its new place.

### The other effects

- **Workspace slide:** a window that opens on the workspace arriving, or closes on the one leaving, does both: it moves with its workspace and zooms and fades.
- **Overview:** neither animation runs while the Overview is open or during its transition. A closing window's picture still running when the Overview opens is dropped. A window that opens or closes in the Overview joins or leaves its layout as its spec says.
- **The session lock** ends every running animation at once, and none starts under it. **An output that goes** drops the pictures on it. Both are as the slide has them (235).

### Settings

- `open-close-animation` in `org.mate.Caminito` (`data/org.mate.Caminito.gschema.xml` and `tests/schemas/`), a boolean, default `true`.
- The animations run when that key is on and the minimise animation's three are as it needs them: `compositing-manager` on, `reduced-resources` off and `org.mate.interface enable-animations` on.
- A change applies to the next window that opens or closes: one that runs finishes.

## Testing

- Headless tests advance Caminito's clock, as the slide's and the minimise animation's do, and read from the debug dump each opening window and each closing window's picture with the place, size and opacity it is drawn at, part-way and at the end.
- They cover: a Wayland and an X11 window each way, the windows left out, each key off, input part-way, each interruption, the slide, the Overview, the lock and an output going, under Per-output workspaces and under Spanning workspaces.
- No golden image of a window part-way: a half-transparent picture differs by a pixel between machines.
- [checklist.md](../../docs/testing/checklist.md) gets an item for a real Session, which is the developer's to run.

## Not decided

Nothing.

## Tickets

Four slices, filed in `.scratch/issues/` as tickets 251 to 254.

| | Slice | Blocked by |
| --- | --- | --- |
| 251 | [**A window grows and fades in as it opens.**](../issues/251-a-window-grows-and-fades-in-as-it-opens.md) | None |
| 252 | [**A window shrinks and fades out as it closes.**](../issues/252-a-window-shrinks-and-fades-out-as-it-closes.md) | 251 |
| 253 | [**An interrupted animation hands over from what is on screen.**](../issues/253-an-interrupted-animation-hands-over-from-the-screen.md) | 252 |
| 254 | [**The animations beside the slide, the Overview and the lock.**](../issues/254-the-animations-beside-the-slide-the-overview-and-the-lock.md) | 252 |
