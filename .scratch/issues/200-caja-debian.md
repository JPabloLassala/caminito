# caja, the patched package for Ubuntu

Epic: debian-packaging
Type: implementation
Status: open: the first criterion's podman run, `tools/debian-build caja`, is the developer's: what runs in the container passed in an Ubuntu 26.04 chroot, as 194's did, but this Session had no podman
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
- [x] In the container it replaces Ubuntu's, and `apt-cache policy` shows the pin.
- [x] `caja-open-terminal` and `caja-sendto` from Ubuntu load in the patched caja, in the container or in 207's VM.

## Done (2026-10-12)

- **Run in a chroot, not in podman,** as 194 was. `tools/debian-build --in-container <worktree> caja` ran as root in an `mmdebstrap --variant=apt` chroot of resolute (main and universe), the worktree and the main `.git` bound read-only. It fetched the tag, applied both patches, built, ran the tests, passed lintian (`--fail-on error`), installed the six `.deb`s and left them in `out/`. The host half ran as far as its argument check and `podman run`, which isn't installed here: the developer's first `tools/debian-build caja` closes the first criterion.
- **Where the baseline comes from:** `packaging/debian/caja/upstream` names upstream's git URL, the tag and its commit, and the patch series. `tools/debian-build` clones the tag, fails if its commit differs, writes `caja_1.28.0.orig.tar.xz` from its `git archive` (no submodules, as Ubuntu's orig from git.mate-desktop.org is), unpacks it, adds the package's `debian/`, and copies `packaging/patches/caja/*.patch`, then `…/debian/*.patch` (there are none), into `debian/patches/` with a `series` (ADR 0030). A package with an `upstream` file keeps its changelog's version; Caminito's own still get theirs from the commit. The tarball came from GitHub because pub.mate-desktop.org and git.mate-desktop.org are out of this Session's reach; the tag's commit pins it either way.
- **Ubuntu's `debian/` was 1.26.4-2, with no patch,** so there was nothing to drop. It moved to 1.28.0 unchanged but for: Maintainer (Ubuntu's team is kept as `XSBC-Original-Maintainer`), no Uploaders or Vcs fields, `--enable-wayland` with `libgtk-layer-shell-dev` and `libwayland-dev` (Ubuntu's builds without gtk-layer-shell, so its desktop isn't the layer surface the patches change), the tests, the pin and its lintian override. mate-submodules still comes from resolute's `mate-submodules-source` (4074560, newer than 1.28.0's submodule commit). The symbols file needed no change: dpkg-gensymbols reported no new symbol.
- **The tests run, where Ubuntu's skips them:** eel's `check-eel` and `caja --check`, on `xvfb-run` and `dbus-run-session`, with caja's schemas compiled from the tree into `debian/test-schemas` and `mate-desktop-common` (`org.mate.background`) as a `<!nocheck>` build dependency. Both pass.
- **The binary packages are Ubuntu's six:** caja, caja-common, libcaja-extension1, libcaja-extension-dev, libcaja-extension-doc and gir1.2-caja-2.0. The differences, by `dpkg -c` and `Depends` against Ubuntu's 1.26.4-2: caja ships the pin and its lintian override and depends on `libgtk-layer-shell0`; caja-common lacks `caja-bookmarks-window.ui` and `caja-file-management-properties.ui`, which 1.28.0 builds into the binary; each package has an upstream `changelog.gz`.
- **The pin is `Pin: version /caminito/`, not `*caminito*`.** With the ADR's glob, `apt-cache policy caja` showed 100: apt takes a version pattern ending in `*` as a prefix, so `*caminito*` matched no version (`*caminito1` and `1.28*` did, which showed the cause). The regex matches. ADR 0029, the spec and 195 now say `/caminito/`. `rules` writes `/etc/apt/preferences.d/caminito-$(DEB_SOURCE)` into the binary named as the source, listing every `Package:` of `debian/control`, so it names no package and 195 to 203 can copy it as it is. lintian's error on it, `package-installs-apt-preferences`, is overridden in `caja.lintian-overrides`, since installing the pin is the point.
- **Replacing Ubuntu's, in a fresh resolute chroot:** with Ubuntu's caja, caja-open-terminal and caja-sendto installed, `apt install ./caja_*.deb ./caja-common_*.deb ./libcaja-extension1_*.deb` upgraded the three in place. `apt-cache policy` shows each at 1001 under "Pinned packages", Ubuntu's 1.26.4-2 at 500, and `apt-get -s upgrade` wants nothing.
- **Ubuntu's extensions load:** `ldd -r` of `libcaja-open-terminal.so` and `libcaja-sendto.so` (1.26.1-2build1) finds no undefined symbol against libcaja-extension 1.28.0. `caja --no-desktop` 1.28.0, on Xvfb in the same chroot, maps both and logs "Initializing caja-open-terminal extension" and "Initializing caja-sendto extension". On X11, not Wayland; nothing in the extension ABI depends on the backend.
- **lintian: no error.** One warning: `appstream-metadata-validation-failed`, from upstream's metainfo, whose screenshot and homepage URLs this Session's network refused, plus upstream's own infos.
- **Not run:** `tools/check`, which checks Caminito's code alone, none of which this changes; `/code-review` (the work was reviewed by reading it again); lintian on a source package, since only `-b` is built.

