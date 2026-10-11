# Carried patches are kept once, in packaging/patches

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

[ADR 0030](../../docs/adr/0030-carried-patches-are-kept-once-for-every-distribution.md), under "Layout and tools" in [the spec](../debian-packaging/spec.md).

- Move each patched package's series from `packaging/arch/<pkgname>-caminito/*.patch` to `packaging/patches/<component>/`, keeping the files and their order.
  - The component names are upstream's: `mate-panel`, `mate-session-manager`, `mate-settings-daemon`, `mate-control-center`, `caja`, `mate-utils`, `mate-power-manager`, `vala-panel-appmenu`, `appmenu-gtk-module`.
- Move Arch's build patches to `packaging/patches/<component>/arch/`. msd's `0004`, which takes out the commit that needs mate-desktop 1.29, is one. Also check each series for others: ADR 0011's qualifications and the PKGBUILD comments name them.
- Each PKGBUILD lists its patches from the new place, and keeps its checksums.
- The README's "Patched MATE packages" section, its rebase steps and ADR 0011's mentions of where the patches live follow the move.

## Acceptance criteria

- [ ] No `.patch` file is left under `packaging/arch/`. Every one is under `packaging/patches/`, with `git log --follow` reaching its history.
- [ ] Each PKGBUILD's `source` and `prepare()` apply the same series, in the same order, as before. `makepkg --verifysource` passes for each. Building the packages is the developer's, on their machine.
- [ ] The README's rebase steps name the new directory.
