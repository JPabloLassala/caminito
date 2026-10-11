# caja, the patched package for Ubuntu

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 191, 194
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "The packages" in [the spec](../debian-packaging/spec.md) and [ADR 0029](../../docs/adr/0029-on-debian-a-patched-package-keeps-the-stock-names-and-is-pinned.md). It's Arch's `caja-caminito`, as a Debian source package.

- `packaging/debian/caja/debian/`, from Ubuntu 26.04's caja 1.26.4 `debian/`, moved to the baseline, 1.28.0, with Ubuntu's patches dropped where the baseline has them.
- The series from `packaging/patches/caja/`, and its `debian/` build patches if any, copied into `debian/patches/` at build time.
- The version is `<baseline>-0caminito1`, and the pin `/etc/apt/preferences.d/caminito-caja` is installed as 195's is.
- Ubuntu's caja extensions are linked against caja 1.26. The ABI number (5) is the same, so they should load. Check it.

## Acceptance criteria

- [ ] It builds and tests in `tools/debian-build`, with every patch applied. Its binary packages are the ones Ubuntu's has, with the differences listed here.
- [ ] In the container it replaces Ubuntu's, and `apt-cache policy` shows the pin.
- [ ] `caja-open-terminal` and `caja-sendto` from Ubuntu load in the patched caja, in the container or in 207's VM.
