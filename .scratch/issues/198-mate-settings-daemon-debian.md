# mate-settings-daemon, the patched package for Ubuntu

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 191, 194
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "The packages" in [the spec](../debian-packaging/spec.md) and [ADR 0029](../../docs/adr/0029-on-debian-a-patched-package-keeps-the-stock-names-and-is-pinned.md). It's Arch's `mate-settings-daemon-caminito`, as a Debian source package.

- `packaging/debian/mate-settings-daemon/debian/`, from Ubuntu 26.04's mate-settings-daemon 1.26.1 `debian/`, moved to the baseline, 1.29.1, with Ubuntu's patches dropped where the baseline has them.
- The series from `packaging/patches/mate-settings-daemon/`, and its `debian/` build patches if any, copied into `debian/patches/` at build time.
- The version is `<baseline>-0caminito1`, and the pin `/etc/apt/preferences.d/caminito-mate-settings-daemon` is installed as 195's is.
- msd's `0004`, which takes out the commit that needs mate-desktop 1.29.0, applies too: Ubuntu has mate-desktop 1.28.2, Arch's. If 191 left it in `arch/`, it moves to the shared series or is copied into `debian/`.
- It's configured with `--enable-wayland`, as on Arch.

## Acceptance criteria

- [ ] It builds and tests in `tools/debian-build`, with every patch applied. Its binary packages are the ones Ubuntu's has, with the differences listed here.
- [ ] In the container it replaces Ubuntu's, and `apt-cache policy` shows the pin.
