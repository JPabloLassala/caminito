# A workspace the slide wraps back to is drawn at both its places

Epic: workspace-slide
Type: implementation
Status: ready-for-agent
Blocked by: 235
Spec: [Workspace slide](../workspace-slide/spec.md)

## What happens

Ticket [235](235-a-second-switch-retargets-the-slide.md) retargets a running slide, and draws each workspace at one place. With `wrap-style` on, a second switch can wrap back to a workspace that the slide still draws on its way out. That workspace is then drawn only where it arrives, so its windows jump from one side of the output to the other.

The smallest case: two workspaces in one row, `wrap-style` at `toroidal` or `classic`, and `switch-to-workspace-right` two times, the second part-way through the first. At the second switch the first workspace's windows are on their way out on the left. They vanish there and come in from the right.

The developer decided on 2026-10-13: Caminito does what Plasma's Slide effect does. The workspace is drawn at both places, where it leaves and where it arrives, so nothing jumps.

## What to build

- **A slide can hold one workspace at more than one place.** `Slide::retargeted` (`caminito/src/workspace_slide.rs`) drops the `Left` of the workspace the switch goes to. It keeps it when the place is not the one the workspace arrives at, which is the wrap's case. A switch back with no wrap (right, then left) still has the workspace at one place, since the two places are the same.
- **The Scene draws a window once for each place.** `Slide::shift` gives one offset for a window today, and `window_elements` (`caminito/src/scene.rs`) draws the window one time. A window of a workspace at two places is drawn at each, with its Frame, its shadow and its popups, in its place in the stack.
- **The window is one window.** Policy, focus and input have it at its own place, as now. The second drawing is a picture only.
- **The debug dump** lists such a window one time for each place where it is drawn.
- **The spec's "Timing"** says today that a workspace is drawn at one place. Change that line to say what this ticket builds.

## For the agent

- A workspace can be left at two places and also arrive, after more wraps during one slide (two workspaces, the binding held down). Keep each place until the slide ends, as 235 does for the other workspaces, unless two of them are the same place.
- The unit test `a_wrap_back_to_the_workspace_it_left_draws_that_one_where_it_arrives_alone` holds 235's behaviour. It changes with this ticket.
- The headless tests stop Caminito's clock (`Fixture::stop_clock`), and compare each window's drawn rectangle before a switch and after it.

## Acceptance criteria

- [ ] A headless test with two workspaces and `wrap-style` on: right, and right again part-way. Each rectangle the dump had before the second switch is in the dump after it, and the first workspace's window is in the dump two times, one output's width times two apart.
- [ ] The same test reads the output's pixels at a spot of each of the two drawings that is on the output.
- [ ] A switch right and back with no wrap still has each window in the dump one time.
- [ ] A click part-way reaches the window at its own place, and not at the second drawing.
