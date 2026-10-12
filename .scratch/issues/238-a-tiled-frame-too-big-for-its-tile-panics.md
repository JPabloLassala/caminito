# A tiled window whose frame doesn't fit its tile panics in a debug build

Epic: caminito-v0.1
Type: bug
Status: needs-triage
Blocked by: none

## What happens

`relayout_on` (`caminito-policy/src/policy.rs:2333`) lays out a tiled window ([158](158-edge-and-corner-tiling.md)) as its tile's frame less the window's visible extents. It builds a `Size` from that subtraction first, and clamps it to at least 1 only in the next `Size::new`. When the extents are larger than the tile, the first `Size::new` gets a negative width or height. Smithay's `debug_assert` there panics: "Attempting to create a `Size` of negative size".

A release build has no debug assertions, so it goes on and lays the window out at least 1 × 1, as the clamp means it to. A debug build panics: the tests, and a Caminito run with `cargo run`.

## Seen

A proptest run found it on 2026-10-11, at `PROPTEST_CASES=20000`, in a session that built [187](187-one-interaction-for-a-seat.md) on a branch that didn't land. With the default 256 cases it doesn't come up. The first failing case had a size of `(1397, -31)`. Proptest shrank it to these four operations from `tests/invariants.rs`:

1. `Plug` an output of 20 × 30.
2. `Map` a normal SSD window of 1 × 1 with extents of 5, 5, 25 and 5 (left, right, top, bottom).
3. `Action` `Tile(Left)`.
4. `Reserve` 1 pixel at the output's bottom.

The work area is then 29 high, and the window's top and bottom extents together are 30. The panic comes through `set_outputs`, then `rehome_windows`, `relayout` and `relayout_on`.

On `master` at `f8779ff` (2026-10-12), a throwaway Policy test made the same four calls: `set_outputs`, `map_window` with `TestWindow::new(1, 1, 1).with_frame(5, 5, 25, 5)`, `action(Tile(Left))`, and `set_outputs` with the bottom reserved. It panicked with `(0, -1)`.

## Not known

- Whether a real output can get there. The output in the shrunk case is tiny. The unshrunk case was 1397 wide, so its height came from the reserved edges too. A panel reserving most of an output's height would do it.
- What Marco does with a tile smaller than a window's frame. `meta_window_get_current_tile_area` and the constraints may keep the window's least size there.

## To reproduce

`PROPTEST_CASES=20000 cargo test -p caminito-policy random_operations`. Proptest saves the seed to `caminito-policy/proptest-regressions/tests/invariants.txt`. Put that file back afterwards unless the fix is in, since every later run replays the seed first. A Policy example test with the four operations above should fail the same way.

## Acceptance criteria

- [ ] A Policy test with the four operations above fails first, then passes.
- [ ] `relayout_on` builds no `Size` with a negative side.
- [ ] `PROPTEST_CASES=20000 cargo test -p caminito-policy random_operations` passes, or anything else it finds goes into the Done section.
