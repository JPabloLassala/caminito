# Docking and healing a layout

Type: implementation
Status: resolved
Milestone: M6 (proposed)
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

Every Layout is contiguous (CONTEXT.md, [ADR 0014](../../../docs/adr/0014-camino-keeps-the-layouts-and-camino-displays-edits-them.md)): no gaps, no overlaps, and each output touches another along at least 100 logical pixels, or along the whole of the shorter edge when that is less. This ticket is the logic that keeps it so, with no protocol and no toolkit in it, for Camino and for camino-displays ([84](84-camino-displays.md)) alike. The prototype's `Layout` module is the model to port.

- **Dock:** the place nearest to a wanted one where an output of a given size lies against the outline of the others, sharing enough edge with one and on top of none. With a snap distance, it takes the place where its edge lines up with the matching edge of an output it lies against (top with top, bottom with bottom, left with left, right with right). Outputs it doesn't touch don't pull, and centres don't.
- **Heal:** makes a layout contiguous, moving each output as little as it can, with one output held still. A contiguous layout comes back unchanged.
- **Resize:** when an output changes size, those to its right and below keep their side of it, and then the layout is healed.
- **The origin:** a healed layout's top-left corner is at 0,0.
- **Where Camino heals:** on hot-plug, when an output is enabled or disabled, and when a mode or a scale changes an output's logical size. Today `wlr-randr --output A --off` leaves a gap (58's notes); after this it doesn't.
- **A configuration that isn't contiguous:** whether Camino heals what a wlr-output-management client asks for, or fails it, is to decide here. Healing is the proposal, since `wlr-randr` and kanshi users write places by hand.
- **A new output with no saved layout** goes to the right of the rightmost, with its top at the layout's top.

## Acceptance criteria

- [x] Property tests in the crate, written before the code: a healed layout has no gap, no overlap and no output cut off; healing it again changes nothing; a docked output leaves the layout contiguous.
- [x] Unit tests for the snap: it takes the matching edge of an output it lies against, and not that of one it doesn't touch.
- [x] Headless tests: unplugging the middle output of three closes the gap; a scale change that grows an output pushes its right-hand neighbour along.
- [ ] Nested or on the tty, `wlr-randr --output A --off` with three outputs leaves the other two touching.

## Notes

- **Which crate:** `camino-policy` is the proposal, since it is already free of protocols (ADR 0010) and camino-displays can depend on it. A crate of its own is the other way, if policy pulls in too much for a settings program. To decide here.
- **Logical pixels and rounding:** an output's logical size is its mode over its scale, rounded. The prototype rounds to whole pixels; Camino's own rounding (51) is what to match.
- The prototype that settled the design is `.scratch/camino-displays-prototype/index.html` on the branch `prototype/camino-displays` (8a4981d), with its verdicts in the commit message. It is a source, not code to keep.

## Resolution

The logic is the crate `camino-layout`: `dock`, `heal`, `resized`, `at_origin`, `at_the_right` and `is_contiguous`, over a `Rect` of its own, with its tests in `camino-layout/src/tests.rs`. Camino's side is `heal_layout` in `camino/src/output.rs`, which every change to the outputs goes through, with its tests in `camino/src/tests/output_lifecycle.rs` and `output_management.rs`.

- **Which crate (decided here):** one of its own, with no dependencies ([ADR 0015](../../../docs/adr/0015-the-layout-logic-is-a-crate-of-its-own.md)). `camino-policy` would have brought Smithay, pinned by git revision, into camino-displays.
- **A configuration that isn't contiguous (decided here):** Camino heals it and answers `succeeded`. The client is then sent the places the outputs took, which may not be the ones it asked for. A place beyond 2^24 on either axis fails the configuration, as a scale of no use does, so that the sums of places and sizes stay within an `i32`.
- **A place is no longer marked as set.** 58's `ChosenBy` stays for the scale alone. 51's rule, the outputs at the default place laid left to right after the placed ones, is gone: an output is given a place once, when it's shown, and after that only healing moves it. So an output a client placed goes to the origin when it's the only one left, and `wlr-randr --output A --off` closes the gap though it sends the others' places back. [83](83-saved-layouts.md)'s "a restored place counts as set" has nothing to set, for the place.
- **The output held still** is the first one, in the order the outputs were shown. [81](81-primary-output.md) should hold the primary output instead, as the prototype does.
- **The order within one configuration:** the modes and the outputs turned on and off first, each healed as it happens (58's order). Then the scales, whose changes of size push the neighbours. Then the places the configuration gives, and one last heal. So a resize pushes only the outputs the configuration doesn't place; `wlr-randr` places them all, and then an output that grew moves its neighbour off itself by the shortest way.
- **A new output** goes right of the rightmost at the layout's top, and is then healed, so where it would touch no other output it is docked: beside a wider output below the first one, it ends 100 pixels down that output's side.
- **Logical sizes:** the crate takes them and doesn't work them out. Camino's is `output_geometry`'s: the mode over the scale, each axis rounded half away from zero, at a scale already snapped to 120ths. camino-displays ([84](84-camino-displays.md)) has to round the same way.
- **Snapping** is in the crate and tested there, but nothing in Camino calls `dock` with a snap distance: that's camino-displays's drag.
- **Differences from the prototype:** distances are compared as squared integers, not floats, and where two places are equally near, the one beside an output wins over the one above or below it.
- **Not tested:** a mode change from the backend that resizes an output with neighbours, which goes the same way as a scale change; and more than one output changing size in one configuration.

The fourth criterion needs three monitors on the tty, and is the developer's. It's in [the checklist](../../../docs/testing/checklist.md) as "Turning an output off leaves the others touching". Its headless stand-in is `disabling_the_middle_output_of_three_leaves_the_other_two_touching`, whose client sends every other head's place back as `wlr-randr` does.
