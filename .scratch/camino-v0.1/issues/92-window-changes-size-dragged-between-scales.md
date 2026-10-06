# A window that follows the scale changes size as it's dragged between outputs

Type: implementation
Status: open
Milestone: M5 (proposed: it's a failure of M4's scale item in 73's closing run)
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

foot dragged between the AOC (scale 1.0) and the LG (scale 1.7) comes out another size each time, and after a few crossings it's unusable. The developer saw it on 2026-10-06 in [73](73-packaging-and-closing-m5.md)'s closing run, on bare `camino` as `caminotest` on the tty, `camino-git r225.c221860-1`: "when I drag from the AOC to the LG, the window gets wider. Every time I do this it gets wider and wider. It doesn't happen in camino full session", where the LG is at 1.75.

- **Measured,** with a fresh foot at 699x489 on the LG and `WAYLAND_DEBUG=1`, over two round trips with no resize: it ended 72x304 on the AOC. An earlier foot, after more crossings, was 1074x306. The trimmed log is [92-checks/foot-two-round-trips.log](92-checks/foot-two-round-trips.log).
- **What the log shows:**
  - While the window spans both outputs, Camino sends `preferred_scale` 120 and 204 in turn, about every 6 ms, with the window not moving: 142 of them in the two round trips, where four would do.
  - foot keeps its grid of cells, and a cell isn't the same logical size at both scales, so each `preferred_scale` makes it ask for another window geometry: 699x474 at 1.7 and 648x496 at 1.0, to start with.
  - Camino also sends `xdg_toplevel.configure` with a size foot had before the last scale, such as `configure(699, 474)` arriving while foot is at 1.0. foot fits that size to the other scale's cells and answers 696x464, then 751x444 at the next scale, and so the size walks.
- **Supposed, not shown:** the scale follows Smithay's primary scanout output (`send_preferred_scales` in `camino/src/window/mod.rs`), which each output's render updates with `default_primary_scanout_output_compare`. A window that changes size with the scale changes how much of it is on each output, so each output's frame hands the surface to the other. In the Session the LG is at 1.75, and whether foot's two sizes there just don't cross the line, or something else differs, isn't known.
- **To find out:** what flips the main output, in a headless test with two outputs at 1.0 and 1.7 and a client that changes its size with the scale; and which of Camino's configures carry a size the client has already left, and why they're sent.
- **The fix is likely two:** a window's scale that doesn't depend on a size the client changes because of that scale (the output under the pointer during a drag, or the one that has the window's centre, with the choice kept until the window is wholly elsewhere, as 52 has it for GTK3's `wl_surface.enter`), and no configure of an old size to a floating window that hasn't been asked to resize.

## Acceptance criteria

- [ ] The cause is named: what flips the scale, and which configure carries a stale size.
- [ ] A headless test, written before the change: a client that takes another size at each preferred scale, moved from a 1.0 output to a 1.7 one and back, gets each scale once per crossing and ends at the size it started with.
- [ ] On the developer's machine, on bare `camino` at the default scales, foot dragged from the LG to the AOC and back five times is the size it was.

## Notes

- GTK3 has no preferred scale (52), so pluma doesn't show it. A client with `wp_fractional_scale_v1` that keeps its logical size wouldn't walk either, but would still get the scales in turn.
