# Theme parser and model

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M2
Blocked by:
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`caminito-theme`'s tier 0 from [04](04-metacity-theme-format.md): loading a Marco theme into a model that the geometry ([26](26-theme-geometry.md)) and drawing ([27](27-frame-drawing.md)) tickets evaluate. It's pure Rust with no cairo yet, and it's written test-first.

- **Lookup (09):** `themes/<name>/metacity-1/` in `$XDG_DATA_HOME`, then each of `$XDG_DATA_DIRS`. Marco loads the newest `metacity-theme-<N>.xml` a theme ships that it supports, and Caminito does the same, up to format 3.5.
- **Lenient parser (04):** it accepts exactly what GLib's gmarkup accepts, including raw `<` inside attribute values and missing spaces between attributes, which strict XML parsers reject. gmarkup's rules are the reference, so this is a small hand-written tokenizer rather than an XML crate.
- **Every format version up to 3.5**, including version-conditional elements and attributes, and Marco-only `appmenu`.
- **The model:** constants, frame geometries, draw-op lists (with `include` references resolved), piece and button definitions with every state variant, frame styles and style sets, and the window-type-to-style-set table. Expressions and colour specs are parsed here into trees; evaluation is 26's and 27's.
- **Parsed and ignored (07):** `menu_icon`, `<fallback>` and `theme:` images.
- **Validation as strict as Marco's (07):** a theme loads in Caminito exactly when it loads in Marco. Errors name the file and line.
- **`Palette`:** the type every colour lookup reads: background and text colour per state Marco maps, for the default and dark variants, plus the named `@define-color` table. Its wire and cache formats are [28](28-palette-helper.md)'s.
- **The built-in fallback theme (07, 16):** a small theme compiled into the binary with `include_str!`, which the parser loads like any other. Caminito uses it whenever the configured theme fails ([30](30-ssd-frames.md)).

## Acceptance criteria

- [x] All ten golden themes (Menta, ClearlooksRe, Atlanta, eOS, HighContrast, BlackMATE, TraditionalOk, and Arc, Mint-Y and Yaru from `tests/themes/`) load.
- [x] Unit tests cover each leniency 04 found in real themes, and one rejection per class of error Marco reports, each written before the code it covers.
- [x] The built-in fallback theme loads, and a test loads it with no theme directories on disk.
- [x] The crate still has no Smithay, GTK, Wayland or D-Bus dependency (18's check).

## Notes

- All ten golden themes are vendored under `tests/themes/`, the seven stock ones too, at the versions recorded in its README. ADR 0009 says CI needs neither Marco nor mate-themes, so the stock themes' files had to be in the repo. [25](25-golden-harness.md) still renders its goldens from the installed Marco, so its script should check that the installed versions match the README's.
- Every rejection test's document was also loaded with Marco 1.29.0's own `meta_theme_load` through `libmarco-private`. Marco rejects each one with the same message and, since the tokenizer reports lines as gmarkup does, on the same line. The one difference is a missing image. Marco doesn't report it: its parser loses its place and fails later, usually on a missing `<window>`. Caminito reports the image.
- Marco accepts v2 features in v1 files everywhere except buttons: `META_THEME_ALLOWS` compares the file's version (1000 or more) with 2. So BlackMATE's, GreenLaguna's and Arc's `metacity-theme-1.xml`, which use the shade and stick buttons, fail in both Marco and Caminito. That doesn't matter, because both load those themes' v3 files.
- A malformed expression doesn't stop Marco from loading a theme, so it loads as `Expr::Invalid` with the error Marco reports when it evaluates the expression.

- The lookup follows 09's directories, not all of Marco's: Marco also looks in `~/.themes/<name>/metacity-1/` first and in its own data directory last. The last is `/usr/share` on Arch, which the default `$XDG_DATA_DIRS` already covers. A theme only in `~/.themes` loads in Marco and not in Caminito.
- An image only has to exist here. Marco also needs gdk-pixbuf to recognise the file, so a corrupt image fails the theme in Marco. [27](27-frame-drawing.md) decodes images, and should decode them when the theme loads so that the theme fails the same way.
