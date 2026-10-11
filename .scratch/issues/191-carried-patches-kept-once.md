# Carried patches are kept once, in packaging/patches

Epic: debian-packaging
Type: implementation
Status: resolved
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

- [x] No `.patch` file is left under `packaging/arch/`. Every one is under `packaging/patches/`, with `git log --follow` reaching its history.
- [x] Each PKGBUILD's `source` and `prepare()` apply the same series, in the same order, as before. `makepkg --verifysource` passes for each. Building the packages is the developer's, on their machine.
- [x] The README's rebase steps name the new directory.

## Done (2026-10-11)

- **The before and after were checked with Arch's makepkg 7.1, in a cloud container.** `makepkg --verifysource` passed for all nine patched packages on master and passes after the move. `makepkg -o` ran each `prepare()`, and the tree each one patched has the same git tree hash before and after, for all nine. No package was built: that's the developer's, on their machine.
- **makepkg finds a local source only beside the PKGBUILD,** by its basename, so `../../patches/…` can't be one. The PKGBUILDs take the patches as `file://` sources from `$startdir`, with the same checksums, and `prepare()` applies each by its file name (`${src##*/}`). Symlinks beside each PKGBUILD were the other way, and left `.patch` files under `packaging/arch/`, which the first criterion rules out.
- **makepkg keeps a copy of each `file://` source, and never fetches it again while the copy is there.** So a changed patch would be checked, or hashed by `updpkgsums`, from the old copy. `packaging/arch/.gitignore` ignores the copies, and the README says to remove them whenever a patch changes, in a rebase or on the same baseline.
- **msd's `0004` is the one Arch build patch,** going by ADR 0011 and the PKGBUILD comments. The upstream fixes (msd's `0006`, mate-panel's `0010`, appmenu-gtk-module's two, vala-panel-appmenu's `0003` and `0004`) are any distribution's, so they stay in the shared series.
- **`0004` now applies after the shared series, as ADR 0030 has it, and not fourth.** That's the second criterion's "same order" taken as the same tree: applied last, it gives the same tree as in its old place, and the shared series applies on v1.29.1 without it, as Debian's will. Its file keeps its name and number.
- **`tools/check` wasn't run whole.** The change touches no code, and the container hasn't the developer's Arch packages. The extension's step, whose link test reads `packaging/arch/`, passes: typecheck and 139 tests.
