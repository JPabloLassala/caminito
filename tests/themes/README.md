# Golden themes

The ten themes Caminito's theme engine is tested against (14, ADR 0009), vendored so that the
tests need neither Marco nor mate-themes. Only each theme's `metacity-1/` directory is kept.

| Theme | Source | Licence |
|---|---|---|
| Menta, HighContrast, BlackMATE, TraditionalOk | mate-themes 3.22.26, as installed by Arch's `mate-themes 3.22.26-3` | LGPL-2.1-or-later |
| ClearlooksRe, Atlanta, eOS | Marco 1.29.0, as installed by Arch's `marco 1.29.0-1` | GPL-2.0-or-later |
| Arc | [jnsh/arc-theme](https://github.com/jnsh/arc-theme) `94ac8c7d67d68de0cc688bbd4c3105b9815b446e`, `common/metacity-1/`, light variant | GPL-3.0 |
| Mint-Y | [linuxmint/mint-themes](https://github.com/linuxmint/mint-themes) `a022d8d99c3c33d9f1b0c823eeb3a89530f20ab7`, `src/Mint-Y/metacity-1/` | GPL-3.0-or-later |
| Yaru | [ubuntu/yaru](https://github.com/ubuntu/yaru) `7f18973e05607c609c4b972f0dd9bff36fa2a13e`, `metacity/src/default/` | GPL-3.0 (see the upstream `COPYING`) |

Each is laid out as its build installs it:

- Arc's meson build installs `metacity-theme-{1,2,3}.xml` and the SVGs for the light variant. The
  thumbnail is left out.
- Yaru's build fills in `@FlavourThemeName@` and `@ThemeName@` with `Yaru`, installs
  `metacity-theme-1.xml`, and symlinks `metacity-theme-2.xml` and `metacity-theme-3.xml` to it,
  so Marco loads it as format 3.

EsmerilGlass and EsmerilSolid aren't golden themes and come from no project: they're Caminito's own, from its built-in theme, for the tests of Esmeril (ADR 0048), which need a frame with known see-through pixels and one with none.

These are test data only; nothing here is built into Caminito.
