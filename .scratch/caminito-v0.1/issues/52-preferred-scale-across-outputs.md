# Preferred scale across outputs

Type: implementation
Status: resolved
Milestone: M4
Blocked by: 51
Map: [Caminito v0.1](../map.md)

## What to build

A window between outputs of different scales gets the enter, leave and preferred scale 13 settles, checked headless on a 1.0 and a 2.0 output ([13](13-hidpi-and-accessibility-scope.md), [14](14-development-and-testing-loop.md)).

- **Primary output (13):** one per surface, from Smithay's `update_surface_primary_scanout_output` with its default comparison (it switches at twice the visible area), run after each render. It replaces today's largest-overlap rule in `window::update_outputs`. Before the first render, the parent's primary output, then the output under the pointer.
- **Preferred scale (13):** the primary output's fractional scale through `with_fractional_scale`, and its rounded-up scale as `preferred_buffer_scale`. Output-management scale changes re-send them (13).
- **Enter and leave:** to every output the window overlaps, as now. Frames stay at the largest rounded-up scale.
- **Test client:** records `preferred_buffer_scale` and `wp_fractional_scale_v1`'s preferred scale, as it records enter and leave.

## Acceptance criteria

- [x] Headless insta snapshots, written before the code: a window dragged from a 1.0 output onto a 2.0 one gets enter, then leave, and switches its preferred scale only past twice the visible area; a new window's first preferred scale is its parent's output's, or the pointer's.
- [x] Headless tests: changing an output's scale re-sends the preferred scales of the windows on it.
- [x] Existing snapshots that change with the new primary-output rule are reviewed and the changes explained in the notes.

## Notes

- **Where the primary output is picked.** The backends now return the render element states of each frame. `State::redraw_queued_outputs` draws every queued output first, then `Mapped::update_primary_outputs` runs `update_surface_primary_scanout_output` for each surface (toplevel, subsurfaces, popups) over the outputs drawn, in order, and only then are frame callbacks sent. Smithay drops a surface's primary output when it isn't drawn there, even if an output drawn earlier in the pass showed it; a surface left with none gets a second pass over the outputs so that output takes over.
- **`Mapped::primary_output`** is the toplevel surface's primary output once it's drawn, and before that the parent's primary output, then the output under the pointer (`Caminito::undrawn_primary_output`). It stays while the window isn't drawn anywhere (another workspace, minimised), so frame timing and scale don't change for a hidden window. A surface without a primary output of its own prefers the window's.
- **The first preferred scale** is sent with a pending window's configure, before its first buffer, so the client draws that buffer at the scale it'll likely be shown at.
- **Unplugging:** `leave_output` resets Smithay's primary output of the surfaces on the removed output, since Smithay would otherwise keep comparing other outputs with it while the `Output` lives on. Until the window is next drawn, it takes the before-drawing rule (parent's, then the pointer's output), so for one frame it can prefer a scale other than the output it moved to.
- **`wl_compositor` is now v6** (`CompositorState::new_v6`). At v5, which Caminito advertised, Smithay sends no `preferred_buffer_scale`. GTK3 binds v3 and Xwayland a lower version too, so they see no change. v6 clients also get `preferred_buffer_transform`, the primary output's transform.
- **The pointer and placement can disagree:** policy places a new window on the focused window's output, or the first, not the pointer's. A window mapped with the pointer on another output first prefers the pointer's scale, then its own output's once drawn (`a_new_window_first_prefers_the_scale_of_the_output_under_the_pointer`). Ticket 13's rule is followed as it is.
- **Tests:** `tests/preferred_scale.rs` on a 1.0 and a 2.0 output: a 400-wide window dragged across enters the 2.0 output at 1640 but switches only at 1660, where the 2.0 output shows 168 000 physical pixels against 78 000; dragged back, it switches only once it has left the 2.0 output, as the 1.0 output never shows twice the 2.0 one's pixels while the window spans both. A new window first prefers the pointer's output's scale; a dialog its parent's. An output's scale change re-sends the scales of its windows only. The test client records `buffer scale <n>` and `fractional scale <s>` among the output events, and `WindowSpec::fractional_scale` makes the object before the first commit.
- **Snapshots changed:** `a_client_lists_the_globals` shows `wl_compositor v6`; `a_window_leaves_an_output_that_is_unplugged_and_enters_the_one_it_moves_to` gains `buffer scale 2`, the LG's 1.7 rounded up, now that v6 sends it. No enter or leave changed.
