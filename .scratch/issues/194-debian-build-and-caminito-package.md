# tools/debian-build, and the caminito package

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "Layout and tools" and "The packages" in [the spec](../debian-packaging/spec.md).

- `packaging/debian/caminito/debian/`: `control`, `rules` (debhelper 13), `changelog` and `copyright`.
  - The source is `git archive HEAD`, versioned `0.0~r<count>.g<hash>-1`.
  - `rules` sets `RUSTUP_TOOLCHAIN=stable`, runs `cargo fetch --locked` then an offline `cargo build --release`, runs Meson at `--prefix=/usr`, and installs the two binaries to `/usr/bin`.
  - `dh_auto_test` runs the workspace's tests under `dbus-run-session`, as `check()` does.
  - Its `Depends` are the libraries the binaries link (from `${shlibs:Depends}`) and what `caminito-git`'s `depends` names at run time, by Ubuntu's names. The patched packages and marco come in 205, once they exist.
- `tools/debian-build [package…]`: builds the named source packages, or all of them in order, in a podman container of `ubuntu:26.04`.
  - It installs rustup and the build tools, and each package's build dependencies with `mk-build-deps`.
  - It installs each `.deb` built before building the next.
  - It writes the `.deb`s to `packaging/debian/out/`, which `.gitignore` lists.
  - It runs on the host with podman alone, and the source tree is mounted read-only.

## Acceptance criteria

- [ ] `tools/debian-build caminito` builds and tests `caminito` in the container, with no package installed on the host.
- [ ] `dpkg -c` of the `.deb` lists what `caminito-git`'s package has: the binaries, the helpers, the session entry, the schema, the logo, the keybinding manifests and caminito-displays's entry.
- [ ] `lintian` reports no error. Warnings that come from the local, networked build are listed in the ticket.
