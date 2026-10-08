# Theme geometry

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M2
Blocked by: 24, 25
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Where every part of a frame goes, as Marco computes it: the first half of `caminito-theme`'s API, and the part policy depends on through frame extents (15).

- **Expressions:** Marco's evaluator for position and size expressions, with constants, operators, `min`/`max`, and the variables it defines (`width`, `height`, `object_width`, `object_height`, `mini_icon_*`, `icon_*`, `title_width`, `title_height`, the border sizes and the rest).
- **Frame geometry:** from the theme, the frame type and flags (focused, maximised, shaded, tiled left or right), the client size, the title's pango layout size and the button layout, it computes:
  - the four borders, in logical pixels (07);
  - the title rect;
  - the button rects, including Marco's Fitts's-law extension to the frame edge and dropping buttons that don't fit;
  - the corner radii from `frames.c`'s rounded-corner shape.
- **Button layout:** Marco's `button-layout` string, with spacers, `appmenu` (parsed, and in practice never shown) and unknown button names ignored as Marco does.
- **Title height** comes from pango with the font and `title_scale`, the same way [27](27-frame-drawing.md) lays the title out, so both use one function.
- **Golden comparison:** the test harness in `caminito-theme/tests/` that reads `tests/goldens/`'s case list with the pinned font, runs geometry for each case, and compares with Marco's JSON **exactly**. 27 adds pixels to the same harness.

## Acceptance criteria

- [x] Every geometry golden matches exactly, for all ten themes at scales 1 and 2.
- [x] The expression evaluator has unit tests for each operator and variable, written before the code.
- [x] The golden tests run in CI's push workflow with no Marco installed.

## Notes

- **The API.** `Theme::frame_geometry` is `meta_theme_calc_geometry` and `Theme::frame_borders` is the part policy needs. Both take the text height from `Title::new`, which lays out the title as `meta_frames_ensure_layout` does. [27](27-frame-drawing.md) draws its `layout`. `title_context` builds the pango context from the font options, resolution and language. `ButtonLayout::parse` reads `button-layout`. `Expr::eval`, `x`, `y` and `size` evaluate an expression against an `ExprEnv` the way Marco draws it, with a failure giving 0. Filling an `ExprEnv` from a frame (`fill_env`) is left to 27.
- **Rename.** Ticket 24's `FrameGeometry`, the parsed `<frame_geometry>`, is now `FrameLayout`, which is Marco's own name for it (`MetaFrameLayout`). `FrameGeometry` is now the computed result, which Marco calls `MetaFrameGeometry`.
- **Every theme shows the toggle buttons.** `rect_for_function` means to show shade, above and stick only from format 2, but it compares the format version, which is 1000 or more, with 2. The v1 golden themes show the toggles in Marco's goldens too.
- **Marco's quirks are kept:**
  - Backgrounds are assigned before any button is dropped, and each button keeps its own, so a side can have gaps in its middle backgrounds.
  - The spacer flags aren't shifted when a button is dropped.
  - A spacer is dropped by clearing the position before the spacer count.
  - Aspect sizing adds an unset button height of -1 to the titlebar's minimum.
  - Right-side buttons past `x < 0` are left out.
- **Where Caminito differs from Marco:**
  - A `button-layout` side that is missing is empty here, where Marco keeps the previous layout's side.
  - Marco mirrors the layout for right-to-left languages; here that is left to the caller ([29](29-settings-thread.md) or [30](30-ssd-frames.md)).
  - If only the toggles' opposites are left and they still don't fit, Marco aborts (`meta_bug`). Caminito drops them last.
  - Marco asserts on a window tiled both ways, and on one that is tiled, maximized and shaded. Here the left tile wins, and tiling wins over maximizing.
- **Coverage.** Geometry doesn't depend on the scale, so the scale-2 goldens repeat the scale-1 ones until 27 adds pixels. The goldens place no spacers, drop no buttons, and have no fullscreen or `hide_buttons` frames, so unit tests in `geometry.rs` cover those, with values worked out from Marco's code.
- **Harness.** The harness in `caminito-theme/tests/goldens.rs` has its own `main`, because it sets `FONTCONFIG_FILE` to `tests/fonts/fonts.conf` before any thread starts. CI installs `pango` and `pkgconf`. The PKGBUILD has `pango` as a makedepend, because the workspace build and `check()` compile caminito-theme. The caminito binary doesn't link it until 30.
