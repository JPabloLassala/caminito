# The session finds the patched appmenu-gtk-module by a file it installs

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
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

- [ ] Unit tests: with the file present the session's `GTK_MODULES` gets `appmenu-gtk-module`, and without it, it doesn't. They don't touch `/usr` or `/var`.
- [ ] `grep -rn pacman caminito/src` finds nothing.
- [ ] By the developer, on Arch: with the rebuilt `appmenu-gtk-module-caminito`, mate-terminal's menu is in the panel's applet, as before.
