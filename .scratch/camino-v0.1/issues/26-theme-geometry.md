# Theme geometry

Type: implementation
Status: open
Milestone: M2
Blocked by: 24, 25
Map: [Camino v0.1](../map.md)

## What to build

Where every part of a frame goes, as Marco computes it: the first half of `camino-theme`'s API, and the part policy depends on through frame extents (15).

- **Expressions:** Marco's evaluator for position and size expressions, with constants, operators, `min`/`max`, and the variables it defines (`width`, `height`, `object_width`, `object_height`, `mini_icon_*`, `icon_*`, `title_width`, `title_height`, the border sizes and the rest).
- **Frame geometry:** from the theme, the frame type and flags (focused, maximised, shaded, tiled left or right), the client size, the title's pango layout size and the button layout, it computes:
  - the four borders, in logical pixels (07);
  - the title rect;
  - the button rects, including Marco's Fitts's-law extension to the frame edge and dropping buttons that don't fit;
  - the corner radii from `frames.c`'s rounded-corner shape.
- **Button layout:** Marco's `button-layout` string, with spacers, `appmenu` (parsed, and in practice never shown) and unknown button names ignored as Marco does.
- **Title height** comes from pango with the font and `title_scale`, the same way [27](27-frame-drawing.md) lays the title out, so both use one function.
- **Golden comparison:** the test harness in `camino-theme/tests/` that reads `tests/goldens/`'s case list with the pinned font, runs geometry for each case, and compares with Marco's JSON **exactly**. 27 adds pixels to the same harness.

## Acceptance criteria

- [ ] Every geometry golden matches exactly, for all ten themes at scales 1 and 2.
- [ ] The expression evaluator has unit tests for each operator and variable, written before the code.
- [ ] The golden tests run in CI's push workflow with no Marco installed.
