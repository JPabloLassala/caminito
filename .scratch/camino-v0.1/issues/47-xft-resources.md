# Xft resources in RESOURCE_MANAGER

Type: implementation
Status: open
Milestone: M3
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

Found by [46](46-nested-x11-checks-left-over.md): at a winit scale of 2, xterm's frame was drawn at scale 2 but its text stayed at scale-1 size. xterm's default core font never scales on any desktop. But Xft apps (`xterm -fa`, Tk, Emacs without GTK, xpdf) take their DPI from the `Xft.dpi` X resource, not from XSETTINGS. On X11, msd's xsettings plugin writes msd's Xft resources to the root window's `RESOURCE_MANAGER`. [40](40-xsettings.md) left that write out of Camino's port, and under Camino msd's plugin runs mirror-only ([ADR 0006](../../../docs/adr/0006-camino-serves-xsettings.md)), so nothing writes them. At scale 2, Xft apps render at 96 DPI, half size.

- **The resources:** the ones msd's plugin writes, from the same Xft settings `xwayland::xsettings::table` already computes: `Xft.dpi` (the DPI × the X11 scale, as `Xft/DPI` without the 1024), the antialias, hinting, hint style and RGBA values, and the cursor theme and size. Take the exact names and formats from msd at `927e9dc`, as 40 did, and cite its lines.
- **Merged, not replaced:** as msd does, keep every other line already in `RESOURCE_MANAGER` and replace only these.
- **Written** by Camino's X connection before `DISPLAY` is exported, again after every restart ([42](42-xwayland-restart.md)), and whenever the settings or the X11 scale change ([41](41-x11-scale.md)).

## Acceptance criteria

- [ ] Unit tests, written before the code: the resource lines for `dpi` 96 and 120 at scales 1 and 2, and a merge that keeps unrelated lines and replaces stale Xft ones.
- [ ] An X11-tier test: `RESOURCE_MANAGER` has `Xft.dpi: 96` at scale 1 and `Xft.dpi: 192` at scale 2, and again after an Xwayland restart.
- [ ] Nested with `tools/dev-nested --bin --scale 2`, `xterm -fa Monospace -fs 11` renders its text at the same physical size as at scale 1, and sharp.
