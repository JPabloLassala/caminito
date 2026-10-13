# A workspace the slide wraps back to is drawn at both its places

Epic: workspace-slide
Type: implementation
Status: resolved
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

- [x] A headless test with two workspaces and `wrap-style` on: right, and right again part-way. Each rectangle the dump had before the second switch is in the dump after it, and the first workspace's window is in the dump two times, one output's width times two apart.
- [x] The same test reads the output's pixels at a spot of each of the two drawings that is on the output.
- [x] A switch right and back with no wrap still has each window in the dump one time.
- [x] A click part-way reaches the window at its own place, and not at the second drawing.

## Done (2026-10-13)

- **How it's built:** `Slide::retargeted` (`caminito/src/workspace_slide.rs`) dropped the `Left` of the workspace the switch goes to. It now drops it only when its place is the one the workspace arrives at, which is the switch back's case. After a wrap the place is another one, the `Left` stays, and the workspace also arrives. `Slide::shifts` replaces `Slide::shift`: it gives one offset for each place of a window, and `window_elements` (`caminito/src/scene.rs`) draws the window at each, with its Frame, shadow and popups, one after the other in the stack. Policy, focus and input have no change.
- **For the developer: the places of one slide have no limit.** The ticket says to keep each place until the slide ends. Each retarget gives the slide another 250 ms, so a binding held down with `wrap-style` on adds one place for each repeat, and the slide doesn't end while it's held. The places are far off the output, so each costs one overlap check for each of its windows in a frame, and one line in the debug dump. I measured nothing. A later ticket can drop a place that can't come back onto the output, if 237's Session shows a cost.
- **The two drawings are never on the output at one time** for a window that fits on it: they're the grid's width or height apart, two outputs at the least. So the test of the second criterion reads the pixels at two times: the drawing on its way out directly after the second switch, and the one on its way in 100 ms later. Only the first read fails on 235's code. The spec's "Timing" says this now.
- **Two places of one workspace are never the same**, so there's no code for the ticket's "unless two of them are the same place": a switch moves every place by one distance, and the one place that could meet the arriving workspace's is the one that's dropped. A unit test has a wrap and then a switch back, which leaves the first workspace on each side of the second.
- **The damage tracker** (smithay's `OutputDamageTracker`) keeps several instances of one element id, so one surface drawn two times in a frame is correct. `shifts` gives a window's own place last, so the scene draws it there under its other drawings: when those go at the slide's end, its index among the frame's elements changes and the tracker draws again where they were. No test has this; a window must be larger than the output to show it.
- **A window that left a workspace and shows again without arriving** (made sticky during the slide) is drawn at its own place alone, as 234 has it.
- **Tests:** written with the code, and not seen to fail before it. Unit tests beside the code: the wrap back (235's `..._where_it_arrives_alone`, changed), two more wraps during one slide, and a wrap then a switch back. Headless tests: the wrap back under `toroidal` and under `classic`, with the dump and the pixels; a switch right and back with no wrap, `wrap-style` on; a click at the drawing on its way out, which reaches no window, and one at the window's own place, which reaches it.
- **Not tested:** a wrap back on two outputs, a wrap up or down, an XWayland window, and a real Session (237's).
- **What was run** (OrbStack machine, aarch64): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, the slide's 47 tests, and 114 of `caminito`'s beside them: `debug`, `minimise_animation`, `workspace_popup`, `workspace_moves`, `per_output_workspaces`, `deadlines`, `tests::lock` and `scene`. All passed. **Not run:** the full suite, at the developer's standing word.
- **Code review:** two reviews, of the standards and of the spec, found no bug. Fixed from them: the commit's body, names in `shifts` and in the unit tests, the word "pictures", and a helper for the two drawings in the tests. Left: `shifts` gives a `Vec` whose order has a meaning that only its doc says, and the click test has its press and release two times.
