# Docking and healing a layout

Type: implementation
Status: open
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

- [ ] Property tests in the crate, written before the code: a healed layout has no gap, no overlap and no output cut off; healing it again changes nothing; a docked output leaves the layout contiguous.
- [ ] Unit tests for the snap: it takes the matching edge of an output it lies against, and not that of one it doesn't touch.
- [ ] Headless tests: unplugging the middle output of three closes the gap; a scale change that grows an output pushes its right-hand neighbour along.
- [ ] Nested or on the tty, `wlr-randr --output A --off` with three outputs leaves the other two touching.

## Notes

- **Which crate:** `camino-policy` is the proposal, since it is already free of protocols (ADR 0010) and camino-displays can depend on it. A crate of its own is the other way, if policy pulls in too much for a settings program. To decide here.
- **Logical pixels and rounding:** an output's logical size is its mode over its scale, rounded. The prototype rounds to whole pixels; Camino's own rounding (51) is what to match.
- The prototype that settled the design is `.scratch/camino-displays-prototype/index.html` on the branch `prototype/camino-displays` (8a4981d), with its verdicts in the commit message. It is a source, not code to keep.
