# Frame drawing

Type: implementation
Status: open
Milestone: M2
Blocked by: 26
Map: [Camino v0.1](../map.md)

## What to build

Everything Marco draws (07), with cairo-rs and pangocairo, matching Marco's goldens. 17 names this the largest single piece of M2: a port of `theme.c`'s drawing with golden-exact output.

- **Colour specs:** `gtk:fg/bg/base/text[state]`, `gtk:light/dark/mid/text_aa`, `gtk:custom(name,fallback)` from the palette's `@define-color` table, `blend`, `shade`, and theme constants. They're evaluated against a `Palette` ([24](24-theme-parser-and-model.md)).
- **Draw ops:** `line`, `rectangle`, `arc`, `clip`, `tint`, `gradient`, `image` (with `colorize`), `icon`, `title`, `include` and `tile`, with Marco's half-pixel emulation of X lines.
  - `gtk_box`, `gtk_arrow` and `gtk_vline` are **approximated** (07): a filled rect with a 1px bg/dark border, a triangle, and a 1px line.
- **Images:** SVG through librsvg, PNG through gdk-pixbuf or `image`, cached per theme and size.
- **Pieces and buttons:** every piece, the button backgrounds including the positional and single ones, and every state variant, including the tiled states.
- **Title:** pangocairo with the font, `title_scale`, Marco's 30px fade and `ellipsize_width`, and the font-rendering options passed in.
- **API:** draw a whole frame into a cairo surface at an integer scale, or one of its four **strips** (top including the titlebar, left, right, bottom) with the draw list clipped to it, and the rounded-corner alpha mask applied to the top and bottom strips (07). The compositor ([30](30-ssd-frames.md)) draws strips only; whole frames are for the goldens.
- **Golden comparison:** pixels added to [26](26-theme-geometry.md)'s harness, using the palette colours [25](25-golden-harness.md) recorded. The per-channel tolerance and the count of differing pixels allowed are tuned once the first themes pass, then written in the test with the reason (14).

## Acceptance criteria

- [ ] Every pixel golden passes, for all ten themes at scales 1 and 2.
- [ ] Drawing the four strips of a case and joining them gives the same pixels as the whole frame.
- [ ] A failing case writes the Camino image and a diff image next to the golden, so a CI failure can be inspected from its artifacts.
