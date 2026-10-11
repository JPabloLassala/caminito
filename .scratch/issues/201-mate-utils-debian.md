# mate-utils, the patched package for Ubuntu

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 191, 196
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "The packages" in [the spec](../debian-packaging/spec.md) and [ADR 0029](../../docs/adr/0029-on-debian-a-patched-package-keeps-the-stock-names-and-is-pinned.md). It's Arch's `mate-utils-caminito`, as a Debian source package.

- `packaging/debian/mate-utils/debian/`, from Ubuntu 26.04's mate-utils 1.26.1 `debian/`, moved to the baseline, 1.29.1, with Ubuntu's patches dropped where the baseline has them.
- The series from `packaging/patches/mate-utils/`, and its `debian/` build patches if any, copied into `debian/patches/` at build time.
- The version is `<baseline>-0caminito1`, and the pin `/etc/apt/preferences.d/caminito-mate-utils` is installed as 195's is.
- The dictionary applet is built into the panel's process, so it links 196's `.so.2`.

## Acceptance criteria

- [ ] It builds and tests in `tools/debian-build`, with every patch applied. Its binary packages are the ones Ubuntu's has, with the differences listed here.
- [ ] In the container it replaces Ubuntu's, and `apt-cache policy` shows the pin.
