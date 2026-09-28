# Golden harness

Type: implementation
Status: open
Milestone: M2
Blocked by:
Map: [Camino v0.1](../map.md)

## What to build

The reference side of the theme goldens ([14](14-development-and-testing-loop.md), [ADR 0009](../../../docs/adr/0009-committed-marco-goldens-from-libmarco-private.md)): Marco's own C renders every case once on the developer's machine, and the results are committed so CI never needs Marco.

- **`tools/marco-frame-dump`:** a C program, its own Meson project, never packaged and not included by the top-level `meson.build` (15, 16).
  - It links the installed `libmarco-private` and calls `meta_theme_load`, `meta_theme_calc_geometry` and `meta_theme_draw_frame` onto a cairo image surface.
  - For each case it writes a PNG, and a JSON file of the geometry: borders, button rects and the title rect.
  - It also writes the colours its `GtkStyleContext` produces for the fixed GTK theme, which [28](28-palette-helper.md)'s exact palette test compares against.
  - It needs a GDK display (`theme.c:4173`), so it runs under `xvfb-run`, with `GDK_SCALE=2` for the scale-2 cases.
- **Pinned inputs (14):** both sides of the comparison use the same ones.
  - A vendored font under `tests/fonts/` behind a private `FONTCONFIG_FILE`, passed as an explicit font description.
  - Grayscale antialiasing, `hintstyle` slight, 96 DPI, ASCII titles.
  - A fixed GTK theme. GTK3's built-in Adwaita is recommended, since it needs nothing installed.
  - Fixed icon pixbufs, committed beside the cases.
- **Themes (14):** Menta, ClearlooksRe, Atlanta, eOS, HighContrast, BlackMATE and TraditionalOk from the installed Marco and mate-themes, and Arc, Mint-Y and Yaru vendored under `tests/themes/` at a recorded upstream commit.
- **Cases:** a case list under `tests/goldens/` that both sides read. Each dimension varies from a base case of a focused normal window:
  - frame types normal, dialog, modal dialog, utility and border;
  - focused and unfocused, maximised, shaded, tiled left and tiled right;
  - each button in normal, prelight and pressed;
  - all at scales 1 and 2.
- **Regeneration:** a script that rebuilds `marco-frame-dump` and rewrites `tests/goldens/`, recording the Marco version and commit, the mate-themes version and the GTK theme beside the files. It's run when Marco is bumped or a case is added.

## Acceptance criteria

- [ ] The goldens, geometry files, palette colours and metadata for all ten themes are committed under `tests/goldens/`.
- [ ] Running the regeneration script twice in a row leaves the tree unchanged.
- [ ] `marco-frame-dump` builds with `meson setup` in its own directory, and nothing in the top-level Meson project or the PKGBUILD refers to it.
