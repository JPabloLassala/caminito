# The global menu's two patched packages for Ubuntu

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 192, 196
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "The packages" in [the spec](../debian-packaging/spec.md). These are Arch's `vala-panel-appmenu-mate-caminito` and `appmenu-gtk-module-caminito`, as patched packages under [ADR 0029](../../docs/adr/0029-on-debian-a-patched-package-keeps-the-stock-names-and-is-pinned.md).

- `packaging/debian/vala-panel-appmenu/debian/`, from Ubuntu's 25.04+dfsg-3ubuntu1, with the series from `packaging/patches/vala-panel-appmenu/`. The MATE applet links 196's `.so.2`, and its `.mate-panel-applet` file's `Location` is at Debian's multiarch path.
- `packaging/debian/appmenu-gtk-module/debian/`, from Ubuntu's 25.04-1build1, with its two fixes. It installs `/usr/share/caminito/appmenu-gtk-module` (192).
- Both are versioned `25.04-0caminito1`, above Ubuntu's 25.04, with their pins.
- `caminito` recommends `appmenu-gtk-module (>= 25.04-0caminito1)`, and `caminito-vscode` breaks the applet's packages older than the one with the chord patch, as Arch's conflicts do.

## Acceptance criteria

- [ ] Both build in `tools/debian-build` and replace Ubuntu's in the container, with their pins.
- [ ] The applet's `.mate-panel-applet` file names a library that exists.
