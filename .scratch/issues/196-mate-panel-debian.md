# mate-panel, the patched package for Ubuntu

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 191, 194
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "The packages" in [the spec](../debian-packaging/spec.md) and [ADR 0029](../../docs/adr/0029-on-debian-a-patched-package-keeps-the-stock-names-and-is-pinned.md). It's Arch's `mate-panel-caminito`, as a Debian source package.

- `packaging/debian/mate-panel/debian/`, from Ubuntu 26.04's mate-panel 1.27.1 `debian/`, moved to the baseline, master `58ac6c1`, with Ubuntu's patches dropped where the baseline has them.
- The series from `packaging/patches/mate-panel/`, and its `debian/` build patches if any, copied into `debian/patches/` at build time.
- The version is `<baseline>-0caminito1`, and the pin `/etc/apt/preferences.d/caminito-mate-panel` is installed as 195's is.
- The library is `libmate-panel-applet-4.so.2`, packaged as `libmate-panel-applet-4-2`. The `.so.1` isn't built. Ubuntu's `libmate-panel-applet-4-1` stays installed, and the `-dev` package links against `.so.2`.
- The Wayland window list and pager are configured as on Arch (`--enable-wayland`, with libxfce4windowing and gtk-layer-shell), with `--with-in-process-applets=all`.

## Acceptance criteria

- [ ] It builds and tests in `tools/debian-build`, with every patch applied. Its binary packages are the ones Ubuntu's has, with the differences listed here.
- [ ] In the container it replaces Ubuntu's, and `apt-cache policy` shows the pin.
- [ ] With Ubuntu's `mate-applets` installed, `apt` keeps `libmate-panel-applet-4-1` and doesn't remove an applet.
