# Packaging and closing M2

Type: implementation
Status: resolved
Milestone: M2
Blocked by: 31, 32
Map: [Caminito v0.1](../map.md)

## What to build

M2's code installed through `caminito-git` (17: packaging keeps up), its checklist items, and the closing commit.

- **`caminito-git` (16):**
  - installs `caminito-gtk-palette` to `/usr/bin` through the top-level Meson project;
  - gains the runtime dependencies cairo, pango, librsvg, gdk-pixbuf2, gtk3 (the helper) and marco (its schemas, 09), and the build dependencies they need;
  - `namcap` reports nothing unexplained.
- **Checklist items tagged `M2`** in `docs/testing/checklist.md` (17), each with steps, the expected result and the debug tool:
  - frames at a nested scale of 2, checked with frame debug;
  - a theme switch while windows are open.
- **Closing:** M2's exit from [17](17-milestone-slicing.md) on the installed package, with the results of every `M1` and `M2` checklist item recorded in a `Close M2 Marco frames` commit.

## Acceptance criteria

- [x] `makepkg -si` in `packaging/arch/caminito-git/` installs `caminito` and `caminito-gtk-palette`, and the packaging job is green in CI.
- [x] The two checklist items are written.
- [x] Closing M2: every golden case passes for all ten themes at scales 1 and 2, the palette test passes, and M2's nested exit from 17 holds on the installed package, with the checklist results in a `Close M2 Marco frames` commit.
