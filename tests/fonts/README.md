# Golden font

The one font the theme goldens are rendered with (14, ADR 0009). `fonts.conf` is a private
fontconfig that sees only this directory; both Marco's side (`tools/marco-frame-dump`) and
Caminito's side run with `FONTCONFIG_FILE` pointing at it, and pass the font as an explicit
description (`font` in `tests/goldens/cases.json`) rather than reading it from gsettings.

| File | Source | Licence |
|---|---|---|
| `NotoSans-Bold.ttf` | Noto Sans Bold, version 2.015, as installed by Arch's `noto-fonts 1:2026.09.01-1` (sha256 `1df075a380fc7cb898acf64c1f7b3b4dd780de3caa860178bf929de35817a913`) | OFL-1.1 (`OFL.txt`) |

Bold, because MATE's default title font is `Sans Bold 10`. Replacing the font changes every
golden, so it means regenerating them with `tools/regenerate-goldens`.
