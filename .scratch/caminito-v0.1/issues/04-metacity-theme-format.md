# Metacity/Marco theme format

Type: research
Status: resolved
Blocked by:
Map: [Caminito v0.1](../map.md)

## Question

What exactly does it take to render Marco/Metacity themes? Cover the format versions (metacity-theme-1/2/3.xml), the frame geometry model, the draw operations, the dependencies on GTK style colours (and how Marco gets them under GTK3), button layouts, and how Marco's own theme code (ui/theme*.c) is structured. Survey the real-world themes shipped by mate-themes and the popular third-party ones to see which versions and features they actually use. Are there existing non-Marco renderers (in any language) we could learn from? Output: a spec summary plus a list of the feature subsets that real themes need.

## Answer

Full findings are on branch `research/metacity-theme-format` (commit 23f3178), in `research/metacity-theme-format.md`. Sources: Marco's `src/ui/theme*.c`, `frames.c` and `doc/theme-format.txt`, Metacity's libmetacity, GLib's gmarkup, and a survey of 87 theme files (mate-themes, Marco's bundled themes, and Arc, Adapta, Materia, Numix, Greybird, Blackbird, Qogir, Mint-Y/X and Yaru).

- **Format:** Marco supports v3.5 (Metacity only 3.4; `appmenu` is Marco-only) and loads the newest file a theme ships, so most themes render from v3. The parser must be as lenient as GLib's gmarkup: raw `<` inside attribute values and missing spaces between attributes are common, and strict XML parsers reject them.
- **GTK colours:** these come from the GTK3 theme's `window` node CSS. Caminito needs the background and text colour per state, plus the named `@define-color` table (24 of 87 files use it). GTK can't run inside the compositor, and the only GTK-less Wayland port (metacity-decor for Wayfire) collapses all colours into one.
- **Feature tiers:** tier 0 is the parser and model. Tier 1 is core v1 drawing and geometry. Tier 2 is the v3 features used by the MATE defaults and popular themes (tiled states, named GTK colours, arc, image colorize, title ellipsis). Tier 3 is the long tail, including 3 GTK-rendered draw ops used by only 3 legacy themes.
- **Prior art:** everything descends from Metacity's C code (metacity-decor, wf-external-decoration with its GTK3 helper process, libmetacity, Mutter up to 3.14). There's no reimplementation in any other language, and Muffin has dropped these themes.
- **Open items handed on to Decoration strategy:** how to get the GTK palette (helper process, CSS parse, or lossy fallback), whether to port ~12k lines of theme C to Rust or wrap them over FFI, and per-window dark variants (no Wayland equivalent of `_GTK_THEME_VARIANT`).
