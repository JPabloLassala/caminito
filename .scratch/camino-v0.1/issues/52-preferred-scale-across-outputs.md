# Preferred scale across outputs

Type: implementation
Status: open
Milestone: M4
Blocked by: 51
Map: [Camino v0.1](../map.md)

## What to build

A window between outputs of different scales gets the enter, leave and preferred scale 13 settles, checked headless on a 1.0 and a 2.0 output ([13](13-hidpi-and-accessibility-scope.md), [14](14-development-and-testing-loop.md)).

- **Primary output (13):** one per surface, from Smithay's `update_surface_primary_scanout_output` with its default comparison (it switches at twice the visible area), run after each render. It replaces today's largest-overlap rule in `window::update_outputs`. Before the first render, the parent's primary output, then the output under the pointer.
- **Preferred scale (13):** the primary output's fractional scale through `with_fractional_scale`, and its rounded-up scale as `preferred_buffer_scale`. Output-management scale changes re-send them (13).
- **Enter and leave:** to every output the window overlaps, as now. Frames stay at the largest rounded-up scale.
- **Test client:** records `preferred_buffer_scale` and `wp_fractional_scale_v1`'s preferred scale, as it records enter and leave.

## Acceptance criteria

- [ ] Headless insta snapshots, written before the code: a window dragged from a 1.0 output onto a 2.0 one gets enter, then leave, and switches its preferred scale only past twice the visible area; a new window's first preferred scale is its parent's output's, or the pointer's.
- [ ] Headless tests: changing an output's scale re-sends the preferred scales of the windows on it.
- [ ] Existing snapshots that change with the new primary-output rule are reviewed and the changes explained in the notes.
