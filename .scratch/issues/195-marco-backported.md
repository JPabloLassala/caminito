# marco 1.29.1, a backported package, and the first apt pin

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 194
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "The packages" in [the spec](../debian-packaging/spec.md) and [ADR 0029](../../docs/adr/0029-on-debian-a-patched-package-keeps-the-stock-names-and-is-pinned.md).

- `packaging/debian/marco/debian/`, from Ubuntu 26.04's marco 1.26.2 `debian/`, moved to upstream's 1.29.1 with Ubuntu's patches dropped where 1.29.1 has them. The version is `1.29.1-0caminito1`.
- The pin: `debian/<binary>.install` or `rules` installs `/etc/apt/preferences.d/caminito-marco`, pinning each of the source's binaries at `version *caminito*`, priority 1001. Write it so 196 to 203 copy it with one name changed.
  - 196 came first (2026-10-12): mate-panel's `rules` has the target to copy, `execute_after_dh_install`, and its pin file has a second stanza, Ubuntu's versions at 99 (ADR 0029's qualification). `apt-cache policy` lists 1001 only for a version from a source, so on a `.deb` installed from a file it shows the 99s, not 1001.
- `tools/debian-build` builds it before the packages that need it.

## Acceptance criteria

- [ ] It builds and tests in `tools/debian-build`. Its binaries are the ones Ubuntu's marco has.
- [ ] In the container, `apt install ./*.deb` replaces Ubuntu's marco, and `apt-cache policy marco` shows the pin at 1001.
- [ ] `gsettings list-keys org.mate.Marco.general` has `action-scroll-up-titlebar`.
