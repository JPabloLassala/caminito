# Golden harness

Type: implementation
Status: resolved
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

- [x] The goldens, geometry files, palette colours and metadata for all ten themes are committed under `tests/goldens/`.
- [x] Running the regeneration script twice in a row leaves the tree unchanged.
- [x] `marco-frame-dump` builds with `meson setup` in its own directory, and nothing in the top-level Meson project or the PKGBUILD refers to it.

## Notes

- **The cases.** "Each button" is read as every button a layout can name except `appmenu`, which Marco never shows in practice (26). The base layout is Marco's default, `menu:minimize,maximize,close`. The button cases use `menu,shade,above,stick:minimize,maximize,close`. Cases that set `shaded`, `above` and `stuck` draw the `unshade`, `unabove` and `unstick` buttons in all three states, and two maximised cases put the maximize button (which Marco draws as unmaximize) in prelight and pressed. That makes 34 cases, 680 images and 680 geometry files. Arc's title ellipsizes in the all-buttons cases at scale 1, so [27](27-frame-drawing.md)'s ellipsizing is covered too.
- **What an image holds.** It's the whole frame as `meta_theme_draw_frame` draws it on a transparent surface, client area included. `frames.c` cuts out the client area and rounds the corners with an X shape afterwards, and the geometry file records the radii for that.
- **Geometry is identical at scales 1 and 2**, since it's in logical pixels and the text height doesn't change with the scale. The three text heights (12, 16 and 19) come from the themes' `title_scale`.
- **Pinned inputs beyond the ticket's.** The pango context's language is set to `en-us` explicitly, where Marco takes the locale's. GTK 3.24.52 turns hint metrics on, so `cases.json` records `hint_metrics: on`. The dump checks the antialiasing, hint style, hint metrics, resolution and language the title's pango context actually has, and fails if any differs from `cases.json`. It doesn't check the subpixel order, since grayscale antialiasing ignores it.
- **The font** is Noto Sans Bold 2.015 (OFL-1.1), since MATE's default title font is `Sans Bold 10`.
- **The palette** is rendered through Marco's own `meta_color_spec_render` (`gtk:bg[STATE]` and `gtk:fg[STATE]`), so the `background` class and the transparent-background fallback are Marco's. The named colours are the 24 names the ten themes' `gtk:custom` specs use. Adwaita defines only some of them; the rest are `null`, where Marco uses the spec's fallback. [28](28-palette-helper.md)'s helper has to be given the same names to compare. Because the colours go through Marco's renderer, a state whose `background-color` is transparent is recorded with Marco's fallback, the background of a fresh toplevel window (`theme.c`'s `get_background_color`). No Adwaita state is transparent today, but a helper that reads raw `background-color` would only disagree in that case, so 28 should apply the same fallback.
- **Theme files.** Marco loads all ten themes from `tests/themes/` (`XDG_DATA_HOME`), so both sides read the same files. As [24](24-theme-parser-and-model.md) asked, the script checks that the installed marco and mate-themes are the versions `tests/themes/README.md` records, and that the seven stock themes there are identical to the installed ones.
- **The environment** is scrubbed with `env -i`: private home, config and cache directories, so no `settings.ini` or `gtk.css` reaches GTK. The only system data directories it sees are `glycin-loaders` and `mime`, which gdk-pixbuf 2.44 needs to decode any image, theme images included.
- **The style context** is a fresh one per theme, where Marco keeps one for the life of its frames. That makes no difference: Marco adds the `background` class while it renders a `gtk:` colour, but every draw op saves and restores the context around itself (`theme.c:3929`, `4435`), so the class never outlives the op.
- Regenerating runs on Arch only (it reads package versions with `pacman -Q`). It needs `meson`, `xorg-server-xvfb` (for `xvfb-run`), `json-glib`, `python3`, and the network, to resolve Marco's tag to a commit. It takes about 20 seconds.
