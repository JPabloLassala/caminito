# The session finds the patched appmenu-gtk-module by a file it installs

Epic: debian-packaging
Type: implementation
Status: open: the code and the Arch package are in; the check on Arch with the rebuilt appmenu-gtk-module-caminito is the developer's
Milestone: none: after v0.1
Blocked by: 
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "Caminito and the patches" in [the spec](../debian-packaging/spec.md).

- `caminito/src/session.rs` stops reading `/var/lib/pacman/local` (`PACKAGES`, `menu_module_is_installed`). The module counts as installed when `/usr/share/caminito/appmenu-gtk-module` exists.
- The look-up takes the path as an argument, so a test can point it at a temporary directory.
- Arch's `appmenu-gtk-module-caminito` installs that empty file, and its `pkgrel` goes up. The Debian package does the same in 203.
- The comments in `session.rs` and in `caminito-git`'s PKGBUILD that speak of pacman's database say what the file is.

## Acceptance criteria

- [x] Unit tests: with the file present the session's `GTK_MODULES` gets `appmenu-gtk-module`, and without it, it doesn't. They don't touch `/usr` or `/var`.
- [x] `grep -rn pacman caminito/src` finds nothing.
- [ ] By the developer, on Arch: with the rebuilt `appmenu-gtk-module-caminito`, mate-terminal's menu is in the panel's applet, as before.

## Done (2026-10-12)

- **Test first:** `the_menu_module_counts_as_installed_by_its_packages_file` in `session.rs` points `menu_module_is_installed` at a file in a temporary directory, checks the session's `GTK_MODULES` without the file and then with it, and failed to compile before the look-up took a path.
- **The look-up is `file.exists()` on the path it's given.** The session passes it `MENU_MODULE_FILE`, `/usr/share/caminito/appmenu-gtk-module`. `PACKAGES` and `MENU_MODULE_PACKAGE` are gone, and `grep -rn pacman caminito/src` finds nothing.
- **`appmenu-gtk-module-caminito` installs the empty file with `install -Dm644 /dev/null`,** so it needs no new source and no new checksum. `pkgrel` goes from 2 to 3. The comments in both PKGBUILDs say what the file is.
- **Not run here:** the check on Arch is the developer's. This was built in an Ubuntu 24.04 container on Rust 1.97, with no Xwayland and no MATE. In it, 401 of caminito's tests and caminito-theme's goldens fail, and the base commit cd5b238 fails the same 401 tests. clippy was run on caminito alone (`--no-deps`), because 1.97's clippy flags `manual_range_contains` in caminito-theme, which this ticket doesn't touch. The rest of the workspace's tests pass. `tools/check`'s meson, helper and extension steps weren't run, because nothing they cover changed.
