# 9. Theme goldens are committed Marco renderings from a libmarco-private harness

Date: 2026-09-28
Status: Accepted

## Context

ADR 0003 ports Marco's theme engine to Rust and tests it with golden images rendered by Marco's C, but not how those images are made or compared. `marco-theme-viewer` is interactive only, with frame flags and button states hardcoded (`theme-viewer.c:461-510`, `preview-widget.c:195-201`), and Marco has no rendering tests. The installed `libmarco-private` does export `meta_theme_load`, `meta_theme_calc_geometry` and `meta_theme_draw_frame`. The draw call takes every frame input as an argument and draws onto any `cairo_t` (`theme.c:5870-5913`), but it needs an open GDK display (`theme.c:4173`).

Frame pixels depend on the GTK theme's colours, the title font, pango, freetype and hinting, and the icon theme. Of these, only geometry (borders, button rects, title rect) is independent of the font. The surveyed themes that exercise most of the format (04, 07) include third-party ones, Arc, Mint-Y and Yaru, that no MATE package ships.

## Decision

- **A small C harness, `marco-frame-dump`, lives in the Camino repo.** It links the installed `libmarco-private`, runs under `xvfb-run`, and writes a PNG and a JSON file of geometry for each case.
- **The goldens are committed**, with the Marco version and commit and the GTK theme recorded beside them. A script regenerates them on the developer's machine when Marco is bumped or a case is added. CI compares Camino's output with the committed files and never runs Marco.
- **Both sides get the same pinned inputs:** a vendored font behind a private fontconfig, grayscale antialiasing, slight hinting, 96 DPI, ASCII titles, and a fixed GTK and icon theme.
- **Geometry must match exactly. Pixels must match within a small tolerance**, title text included. The palette is compared exactly in a separate test.
- **The set is** Menta, ClearlooksRe, Atlanta, eOS, HighContrast, BlackMATE and TraditionalOk, plus Arc, Mint-Y and Yaru vendored under `tests/themes/` at a recorded commit.

## Consequences

- Golden changes show up as reviewable diffs, CI needs neither Marco nor Xvfb, and a distribution's Marco update can't break CI with no change in Camino.
- The goldens can drift from current Marco until someone regenerates them. The recorded version makes that visible.
- The repo carries a C program that exists only to produce test data, plus three vendored theme directories.
- Rejected:
  - running the harness live in CI against the image's Marco (CI would depend on Marco and Xvfb, and would break on upstream changes);
  - adapting `marco-theme-viewer` (it's interactive, with no way to choose flags or button states);
  - exact pixel equality (antialiasing differences between pango and freetype builds would make it flaky);
  - masking the title text instead of comparing it within a tolerance (title placement and fading are part of what the port must get right);
  - stock themes only (they miss format features found in the wild).
