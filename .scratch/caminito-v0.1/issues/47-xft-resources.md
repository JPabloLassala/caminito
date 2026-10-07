# Xft resources in RESOURCE_MANAGER

Type: implementation
Status: resolved
Milestone: M3
Blocked by: none
Map: [Caminito v0.1](../map.md)

## What to build

Found by [46](46-nested-x11-checks-left-over.md): at a winit scale of 2, xterm's frame was drawn at scale 2 but its text stayed at scale-1 size. xterm's default core font never scales on any desktop. But Xft apps (`xterm -fa`, Tk, Emacs without GTK, xpdf) take their DPI from the `Xft.dpi` X resource, not from XSETTINGS. On X11, msd's xsettings plugin writes msd's Xft resources to the root window's `RESOURCE_MANAGER`. [40](40-xsettings.md) left that write out of Caminito's port, and under Caminito msd's plugin runs mirror-only ([ADR 0006](../../../docs/adr/0006-caminito-serves-xsettings.md)), so nothing writes them. At scale 2, Xft apps render at 96 DPI, half size.

- **The resources:** the ones msd's plugin writes, from the same Xft settings `xwayland::xsettings::table` already computes: `Xft.dpi` (the DPI × the X11 scale, as `Xft/DPI` without the 1024), the antialias, hinting, hint style and RGBA values, and the cursor theme and size. Take the exact names and formats from msd at `927e9dc`, as 40 did, and cite its lines.
- **Merged, not replaced:** as msd does, keep every other line already in `RESOURCE_MANAGER` and replace only these.
- **Written** by Caminito's X connection before `DISPLAY` is exported, again after every restart ([42](42-xwayland-restart.md)), and whenever the settings or the X11 scale change ([41](41-x11-scale.md)).

## Acceptance criteria

- [x] Unit tests, written before the code: the resource lines for `dpi` 96 and 120 at scales 1 and 2, and a merge that keeps unrelated lines and replaces stale Xft ones.
- [x] An X11-tier test: `RESOURCE_MANAGER` has `Xft.dpi: 96` at scale 1 and `Xft.dpi: 192` at scale 2, and again after an Xwayland restart.
- [x] Nested with `tools/dev-nested --bin --scale 2`, `xterm -fa Monospace -fs 11` renders its text at the same physical size as at scale 1, and sharp.

## Notes

- **The resources** are msd's `xft_settings_set_xresources` (`msd-xsettings-manager.c:630-677`) at `927e9dc`, in its order: `Xft.dpi`, `Xft.antialias`, `Xft.hinting`, `Xft.hintstyle`, `Xft.rgba`, `Xft.lcdfilter`, `Xcursor.theme` and `Xcursor.size`, each as `name:\tvalue`. `Xft.dpi` is `Xft/DPI` over 1024, rounded with msd's `+ 0.5`, so 96 at scale 1 and 192 at scale 2. `xwayland::xsettings` now works out msd's `MateXftSettings` once, as `Xft`, and both the XSETTINGS table and `resources` read it, so the two can't drift apart.
- **The merge** follows msd's `update_property` (`:599-628`): other lines are kept, a stale line is replaced where it is, and missing ones are appended. Where it differs from msd is in the module's doc: a line matches only when its resource name is the same (msd's `strstr` would also rewrite `XTerm*Xft.dpi`), whitespace around the name is ignored as Xrm ignores it, every line setting a name is replaced, not only the first, and blank lines are dropped.
- **Written** by Caminito's X connection, `XConnection::publish_resources`, which reads `RESOURCE_MANAGER`, merges and writes it only when Caminito's resources changed since it last did. `xwayland_ready` writes them after XSETTINGS and before `DISPLAY`. The refresh pass writes them again when the settings or the X11 scale change. A restart makes a new X connection, which has written nothing yet, so they're written again.
- **Tested:** unit tests for the lines at `dpi` 96 and 120 at scales 1 and 2, the font rendering and cursor settings, a merge that keeps unrelated lines and replaces stale Xft ones, and a merge of its own output. The X11-tier test `xft_clients_read_the_dpi_at_the_x11_scale_from_the_resources_and_again_after_a_restart` reads `Xft.dpi: 96`, then `192` after the output goes to scale 2, and `192` again after an Xwayland crash and restart.
- **The nested check, by hand (2026-09-30):** with `tools/dev-nested --bin --scale 2`, `xterm -fa Monospace -fs 11`'s text rendered at scale 2, where [46](46-nested-x11-checks-left-over.md)'s run had it at scale-1 size.
