# Theme parser and model

Type: implementation
Status: open
Milestone: M2
Blocked by:
Map: [Camino v0.1](../map.md)

## What to build

`camino-theme`'s tier 0 from [04](04-metacity-theme-format.md): loading a Marco theme into a model that the geometry ([26](26-theme-geometry.md)) and drawing ([27](27-frame-drawing.md)) tickets evaluate. It's pure Rust with no cairo yet, and it's written test-first.

- **Lookup (09):** `themes/<name>/metacity-1/` in `$XDG_DATA_HOME`, then each of `$XDG_DATA_DIRS`. Marco loads the newest `metacity-theme-<N>.xml` a theme ships that it supports, and Camino does the same, up to format 3.5.
- **Lenient parser (04):** it accepts exactly what GLib's gmarkup accepts, including raw `<` inside attribute values and missing spaces between attributes, which strict XML parsers reject. gmarkup's rules are the reference, so this is a small hand-written tokenizer rather than an XML crate.
- **Every format version up to 3.5**, including version-conditional elements and attributes, and Marco-only `appmenu`.
- **The model:** constants, frame geometries, draw-op lists (with `include` references resolved), piece and button definitions with every state variant, frame styles and style sets, and the window-type-to-style-set table. Expressions and colour specs are parsed here into trees; evaluation is 26's and 27's.
- **Parsed and ignored (07):** `menu_icon`, `<fallback>` and `theme:` images.
- **Validation as strict as Marco's (07):** a theme loads in Camino exactly when it loads in Marco. Errors name the file and line.
- **`Palette`:** the type every colour lookup reads: background and text colour per state Marco maps, for the default and dark variants, plus the named `@define-color` table. Its wire and cache formats are [28](28-palette-helper.md)'s.
- **The built-in fallback theme (07, 16):** a small theme compiled into the binary with `include_str!`, which the parser loads like any other. Camino uses it whenever the configured theme fails ([30](30-ssd-frames.md)).

## Acceptance criteria

- [ ] All ten golden themes (Menta, ClearlooksRe, Atlanta, eOS, HighContrast, BlackMATE, TraditionalOk, and Arc, Mint-Y and Yaru from `tests/themes/`) load.
- [ ] Unit tests cover each leniency 04 found in real themes, and one rejection per class of error Marco reports, each written before the code it covers.
- [ ] The built-in fallback theme loads, and a test loads it with no theme directories on disk.
- [ ] The crate still has no Smithay, GTK, Wayland or D-Bus dependency (18's check).
