# Frame drawing

Type: implementation
Status: resolved
Milestone: M2
Blocked by: 26
Map: [Caminito v0.1](../map.md)

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

- [x] Every pixel golden passes, for all ten themes at scales 1 and 2.
- [x] Drawing the four strips of a case and joining them gives the same pixels as the whole frame, to within 1 in a channel where a gradient rounds the other way (see Notes).
- [x] A failing case writes the Caminito image and a diff image next to the golden, so a CI failure can be inspected from its artifacts.

## Notes

- **The API.** `Theme::render_frame` draws a whole frame and `Theme::render_strip` one `Strip`, both onto a new ARGB32 surface with a device scale of the integer scale, from a `Frame`: type, flags, client size, `Title`, button layout, `ButtonStates`, icons and the `PaletteColors` of the window's variant. `FrameGeometry::strip_rect` says where a strip goes and `corner_cutouts` gives `frames.c`'s rounded-corner shape in device pixels, which 30 can use for input too. `ColorSpec::render` evaluates a colour against a palette. `surface_from_pixbuf` turns an icon into a surface as GDK does.
- **How it matches.** Each op makes Marco's cairo calls in Marco's order, on a surface with the device scale GTK gives a scale-2 window, so cairo rasterizes them the same way. Eight themes match Marco's goldens exactly, every case at both scales, so the pixel tolerance is 0 for them.
- **Images** load through gdk-pixbuf at their size times the scale, which is how Marco loads them, and gdk-pixbuf reads SVG through glycin and librsvg. The ticket's "SVG through librsvg" is kept that way, since loading through the same library is what makes the pixels match. A theme's images are cached per file and scale on the `Theme`. Icons are cairo surfaces, since xdg-toplevel-icon buffers already are. CI installs `gdk-pixbuf2`. glycin runs its loaders without bubblewrap where the container can't make namespaces. The PKGBUILD has `gdk-pixbuf2` as a makedepend.
- **Marco's quirks are kept:**
  - The image stripe checks step through bytes by pixel count.
  - `object_width` and `object_height` stay set for the rest of an op list after an `image`.
  - A `clip` stays set on the caller's context after its list.
  - The title fade's gradient ends at the text's space and height, not at a position.
  - A background's state comes from the button layout by position among the buttons shown, so toggles' opposites and dropped buttons are counted.
  - The middle-background loop visits one slot more than there are, which in Marco's struct is the side's right background. A background with no button gets `META_BUTTON_STATE_LAST`, which reads the next button type's normal ops. Together these draw each side's right background a second time, in its normal state.
- **Where Caminito differs from Marco:**
  - `gtk_box`, `gtk_arrow` and `gtk_vline` are approximated (07). The box is filled with `bg` and has a 1px `dark` border. The arrow is a triangle in `fg`. The line is 1px in `fg`, since GTK draws lines in the border colour, which is the text colour unless the GTK theme sets one.
  - `gtk_arrow` with `arrow="none"` returns without leaving an extra cairo save behind.
  - A `tile` whose tile size can't be evaluated draws nothing, where Marco loops forever.
  - `theme:` images are ignored (07).
  - A cairo error stops the drawing. Marco's context ignores everything after one, so the pixels are the same.
- **The rounded corners** are cut from every strip they reach, not only the top and bottom, since a bottom radius can be taller than the bottom border.
- **Tolerances**, written in `caminito-theme/tests/goldens.rs` with the reasons:
  - The eight themes without the approximated ops match exactly.
  - Atlanta and HighContrast may differ in at most 160 pixels at scale 1 and 600 at scale 2. That is what the approximated ops account for: Atlanta's pressed buttons, which have all three, differ in 154 and 594. The count isn't confined to those ops' rects, so the bounds stay close to what they need.
  - A strip may differ from the whole frame by 1 in a channel. Cairo works a gradient's transform out from where the drawing is on the surface, and a strip's surface starts somewhere other than the whole frame's. So a colour that falls on a rounding boundary can round the other way: 20 pixels of ClearlooksRe's utility buttons at scale 2. Drawing each strip at the whole frame's device position would take a frame-sized surface.
- **For 28.** A palette colour has to reach Caminito as the exact double GTK gave. HighContrast's unfocused title blends to a blue exactly between two 8-bit values, and it only matched once the harness read `palette.json` with serde_json's `float_roundtrip`.
- **Keeping the goldens exact.** The comparison is exact against the cairo, pixman, pango, freetype and librsvg that `metadata.json` records. If CI's Arch container moves to a version that renders differently, the goldens need regenerating, not a looser tolerance.
- **Coverage.** No golden theme uses `tile` or an alpha gradient (`alpha="a:b"`) on any op. Both follow Marco's code, and each has a unit test. Single alphas on `tint`, `image` and `icon` are in the goldens. The corner shape is checked against `frames.c`'s formula by a unit test only, since the goldens are drawn before `frames.c` shapes them.
