# caminito-desktop, caminito's full dependencies, and tools/bootstrap-debian

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 195, 196, 197, 198, 199, 200, 201, 202, 203, 204
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "Layout and tools" and "The packages" in [the spec](../debian-packaging/spec.md).

- `caminito` depends on every patched package and on marco by the stock names, at ≥ their baselines' `caminito` versions, as `caminito-git` depends on the `-caminito` packages. It depends on `mate-screensaver` (the binary, for its PAM file) and the rest of `caminito-git`'s run-time list, by Ubuntu's names.
- `packaging/debian/caminito-desktop/debian/`: a metapackage depending on `caminito` and `mate-desktop-environment-extras`.
- `tools/bootstrap-debian [dpkg-buildpackage args…]`: on the machine, in `tools/debian-build`'s order, it installs the build tools and rustup, and runs `mk-build-deps -i`, `dpkg-buildpackage -b` and `apt install ./…deb` for each, ending with `caminito-desktop`.
- `tools/debian-build` with no argument builds all of them.

## Acceptance criteria

- [ ] `tools/debian-build` builds the whole set from nothing in the container, and `apt install ./caminito-desktop_*.deb` there installs with no stock MATE package left at a version below a baseline.
- [ ] `tools/bootstrap-debian` runs to the end in a fresh `ubuntu:26.04` container with `sudo`.
