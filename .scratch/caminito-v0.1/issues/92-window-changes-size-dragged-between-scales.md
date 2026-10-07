# A window that follows the scale changes size as it's dragged between outputs

Type: implementation
Status: resolved
Milestone: M5 (proposed: it's a failure of M4's scale item in 73's closing run)
Blocked by: none
Map: [Caminito v0.1](../map.md)

## What to build

foot dragged between the AOC (scale 1.0) and the LG (scale 1.7) comes out another size each time, and after a few crossings it's unusable. The developer saw it on 2026-10-06 in [73](73-packaging-and-closing-m5.md)'s closing run, on bare `caminito` as `caminitotest` on the tty, `caminito-git r225.c221860-1`: "when I drag from the AOC to the LG, the window gets wider. Every time I do this it gets wider and wider. It doesn't happen in caminito full session", where the LG is at 1.75.

- **Measured,** with a fresh foot at 699x489 on the LG and `WAYLAND_DEBUG=1`, over two round trips with no resize: it ended 72x304 on the AOC. An earlier foot, after more crossings, was 1074x306. The trimmed log is [92-checks/foot-two-round-trips.log](92-checks/foot-two-round-trips.log).
- **What the log shows:**
  - While the window spans both outputs, Caminito sends `preferred_scale` 120 and 204 in turn, about every 6 ms, with the window not moving: 142 of them in the two round trips, where four would do.
  - foot keeps its grid of cells, and a cell isn't the same logical size at both scales, so each `preferred_scale` makes it ask for another window geometry: 699x474 at 1.7 and 648x496 at 1.0, to start with.
  - Caminito also sends `xdg_toplevel.configure` with a size foot had before the last scale, such as `configure(699, 474)` arriving while foot is at 1.0. foot fits that size to the other scale's cells and answers 696x464, then 751x444 at the next scale, and so the size walks.
- **Supposed, not shown:** the scale follows Smithay's primary scanout output (`send_preferred_scales` in `caminito/src/window/mod.rs`), which each output's render updates with `default_primary_scanout_output_compare`. A window that changes size with the scale changes how much of it is on each output, so each output's frame hands the surface to the other. In the Session the LG is at 1.75, and whether foot's two sizes there just don't cross the line, or something else differs, isn't known.
- **To find out:** what flips the main output, in a headless test with two outputs at 1.0 and 1.7 and a client that changes its size with the scale; and which of Caminito's configures carry a size the client has already left, and why they're sent.
- **The fix is likely two:** a window's scale that doesn't depend on a size the client changes because of that scale (the output under the pointer during a drag, or the one that has the window's centre, with the choice kept until the window is wholly elsewhere, as 52 has it for GTK3's `wl_surface.enter`), and no configure of an old size to a floating window that hasn't been asked to resize.

## Acceptance criteria

- [x] The cause is named: what flips the scale, and which configure carries a stale size.
- [x] A headless test, written before the change: a client that takes another size at each preferred scale, moved from a 1.0 output to a 1.7 one and back, gets each scale once per crossing and ends at the size it started with.
- [x] On the developer's machine, on bare `caminito` at the default scales, foot dragged from the LG to the AOC and back five times is the size it was.

## What was found

- **What flips the scale** is the two outputs' refresh rates, not the window's size. Smithay's `default_primary_scanout_output_compare` (`renderer/element/mod.rs:428-460`) hands a surface to another output when that one shows twice the pixels, *or when it has the higher refresh rate*, however little of the surface is on it. On the tty each output draws at its own vblank, so while a window spans both, the faster output takes it at its frame and the slower one takes it back at its own, with twice the pixels. The headless test shows it with the 1.0 output at 75 Hz and the 1.7 one at 60: a scale at every frame before the change, and with both at 60 Hz one per crossing. The supposition above, that the client's change of size moves the line, is wrong: the size plays no part.
- **Why the Session doesn't show it** isn't known. If the outputs' modes there have the same refresh rate, or the LG is the faster one and the window is dragged where the AOC never has twice its pixels, it wouldn't. `wlr-randr` in both would say.
- **The configure with a stale size** is the one a move sends. A floating client that picks its own size has policy's geometry follow it without a request (`Policy::commit`). At the next step of a drag the position changes, policy sends the window its geometry, and the xdg adapter puts that size, which the client was never configured with, in the pending configure; it differs from the last one sent, so a configure goes out for the size alone. With the scale flipping every 6 ms, foot had left that size by the time it arrived.

## What was done

- `Mapped::update_main_outputs` compares with Caminito's own `main_output_compare`: twice the pixels, and nothing about refresh rates. The ticket's other idea, the output under the pointer or the window's centre, wasn't needed: twice the pixels each way is already a choice that's kept.
- The xdg adapter's `flush` sends no configure whose only news is the size a floating client draws at, and keeps the pending size at the client's own while policy hasn't asked for another, so the next configure with a reason to go (focus, a state) carries the size the client has, not one it left. A maximized, fullscreen or resizing window is configured as before.
- Left as they are: a configure for the size alone still goes out while an earlier configure isn't acked, with the size the client had when it was sent; and a window maximized one way, which xdg-shell has no state for, counts as floating here, so if its client picks another size the next configure carries that one until policy next sends its geometry.
- Tests, in `caminito/src/tests/preferred_scale.rs`: `a_window_that_resizes_with_its_scale_gets_each_scale_once_as_its_dragged_across_and_back`, with a client that keeps a grid of cells as foot does and each output drawn in turn, and `a_window_that_picked_its_own_size_isnt_configured_with_it_as_its_dragged`.

## On the developer's machine (2026-10-06)

Bare `caminito` as `caminitotest` on tty4, `caminito-git r238.ea769c7-1`, the LG at 1.7 (3840x2160 at 59.997 Hz) and the AOC at 1.0 (1920x1080 at 60.000 Hz), so the refresh rates do differ there. A foot with `WAYLAND_DEBUG=1` was dragged across and back many times; the trimmed log is [92-checks/foot-after-ea769c7.log](92-checks/foot-after-ea769c7.log).

- **The scales no longer come in turn at each frame:** 26 `preferred_scale` over about a dozen crossings in 80 s, none closer than 0.3 s to the one before, against 142 in two round trips.
- **The size no longer walks:** foot is 699x474 at 1.7 and 648x496 at 1.0 throughout, and ends 648x496 on the AOC.
- **The developer still sees the window change height as it crosses.** That's the one change per crossing, 474 to 496 and back, which is foot's: it keeps its grid, and a cell isn't the same logical size at both scales. The developer confirmed it: one change of height at each crossing, and nothing else. The third criterion is ticked on that and on the log.
- **One configure with a stale size was left,** at 23:39:36.617: `configure(699, 474)` reached foot at 1.0, foot answered 696x464, and the next configure, `(648, 496)`, 1 ms later, put it back. It was the review's own addition, which owed a configure whenever policy named another size than the pending one: a late commit of foot's and a step of the drag read in one pass of the loop made policy name the size of that commit before the adapter had followed it. The adapter now decides that at the flush, where it knows the size the client draws at, with `a_window_isnt_configured_with_the_size_it_picked_in_the_same_pass_as_a_drag_step` written first. Not yet run on the tty.
- **The second run (2026-10-07),** on `caminito-git r239.c14283a-1`, with that change: three crossings, three `preferred_scale`, foot at 699x474 and 648x496, and no configure during the drags.
- **The change of size at each crossing is foot's, and stays.** mate-terminal and pluma, dragged the same way, keep their size, as the developer saw. A foot started with `resize-keep-grid=no` changes size too (696x496 at 1.0, 751x474 at 1.7): the option is for changes of font size, not of scale. The developer closed the ticket on that.

## Notes

- GTK3 has no preferred scale (52), so pluma doesn't show it. A client with `wp_fractional_scale_v1` that keeps its logical size wouldn't walk either, but would still get the scales in turn.
