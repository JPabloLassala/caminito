# marco 1.29.1, a backported package, and the first apt pin

Epic: debian-packaging
Type: implementation
Status: open: the podman run of `tools/debian-build marco` is the developer's, as 194's is: what runs in the container passed in an Ubuntu 26.04 chroot, but this Session had no podman
Milestone: none: after v0.1
Blocked by: 194
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "The packages" in [the spec](../debian-packaging/spec.md) and [ADR 0029](../../docs/adr/0029-on-debian-a-patched-package-keeps-the-stock-names-and-is-pinned.md).

- `packaging/debian/marco/debian/`, from Ubuntu 26.04's marco 1.26.2 `debian/`, moved to upstream's 1.29.1 with Ubuntu's patches dropped where 1.29.1 has them. The version is `1.29.1-0caminito1`.
- The pin: `debian/<binary>.install` or `rules` installs `/etc/apt/preferences.d/caminito-marco`, pinning each of the source's binaries at `version *caminito*`, priority 1001. Write it so 196 to 203 copy it with one name changed.
- `tools/debian-build` builds it before the packages that need it.

## Acceptance criteria

- [x] It builds and tests in `tools/debian-build`. Its binaries are the ones Ubuntu's marco has.
- [x] In the container, `apt install ./*.deb` replaces Ubuntu's marco, and `apt-cache policy marco` shows the pin at 1001.
- [x] `gsettings list-keys org.mate.Marco.general` has `action-scroll-up-titlebar`.

## Done (2026-10-12)

- **Run in a chroot, not in podman,** as 194 was: `tools/debian-build --in-container <checkout> marco caminito` ran as root in an `mmdebstrap --variant=apt` chroot of resolute (main and universe). The host half isn't changed but for the package list, and wasn't run. The criteria are checked against the chroot; the developer's first `tools/debian-build` closes them in podman.
- **The upstream source: an `upstream` file** beside `debian/`, `packaging/debian/marco/upstream`, with the release tarball's `url=` and `sha256=`. `tools/debian-build` reads it from the committed HEAD, fetches the tarball as `marco_1.29.1.orig.tar.xz`, checks its sha256 and unpacks it under HEAD's `debian/`; the changelog's version is left as committed. A package with no `upstream` file is built from HEAD, as caminito is. 196 to 203 add a file each; a git baseline (mate-panel's `58ac6c1`) can use GitHub's archive URL for the commit, or teach the file a `git=` line.
- **Ubuntu's patches:** `0001` and `0002` (compositing availability) and `1001` (`--no-keybindings`) are in 1.29.1 and are dropped. `0000_shadows` (Debian's shadows) and `2000` (the Y2K38-unsafe startup-notification call, still needed while Debian's startup-notification is unfixed) apply without fuzz and are kept.
- **`debian/` changes beyond the version:** the release tarball ships its `configure`, so `autogen.sh` isn't run and `autoconf-archive`, `mate-common` and `intltool` (which 1.29's `configure` doesn't use) are no longer build dependencies. 1.29.1 links libmate-desktop ≥ 1.27.1, which Ubuntu has at 1.28.2, so `libmate-desktop-dev (>= 1.27.1)` is one. Maintainer is the developer, with Ubuntu's team as `XSBC-Original-Maintainer`, and the salsa `Vcs-*` fields are gone.
- **`libmarco-private2`'s symbols:** 1.29.1 adds five and drops `meta_ui_get_{mini,window}_icon_from_name` at the same soname, `.so.2`. The symbols file follows. In resolute, only mate-control-center depends on the library: Ubuntu's 1.26.1 uses seven `meta_` symbols, neither dropped one, so it runs on 1.29.1 until mate-control-center 1.29.1 replaces it.
- **The binaries are Ubuntu's:** `marco`, `marco-common`, `libmarco-private2`, `libmarco-dev`, all `1.29.1-0caminito1`. Its `make check` has no tests. lintian has no error or warning, with `package-installs-apt-preferences` overridden in `marco.lintian-overrides`, since the pin is ADR 0029's.
- **The pin is `version /caminito/`, not `version *caminito*`.** apt 3.2 matched `*caminito*` to no version (the installed one stayed at 100), though it matches `1.29*`. The regex gives 1001. ADR 0029 and the spec now say so. `rules` writes the pin from `debian/control`'s `Package:` lines and `DEB_SOURCE`, so a copy changes `PIN_PACKAGE`, the binary that carries it, alone; each copy needs its own lintian override too. It is a conffile: purging removes it (checked), and after a plain remove it pins nothing apt can install. ADR 0029 says so instead of "removing the package removes its pin".
- **Checked in the chroot:** with Ubuntu's marco 1.26.2 installed, `apt install` of the four `.deb`s upgraded three and installed `libmarco-dev`; `apt-cache policy` gives `1.29.1-0caminito1` at 1001; `apt-get -s dist-upgrade` and `apt-get -s install marco` change nothing; `marco --version` is 1.29.1; `gsettings list-keys org.mate.Marco.general` has `action-scroll-up-titlebar` and `action-scroll-down-titlebar`.
- **caminito still builds after marco,** in the same run, with master's 212 merged: 2676 tests passed, none failed, 10 ignored; lintian's warnings are 194's.
- **Not run:** `/code-review` (the diff was read again instead), the podman run, and a `.deb` install on a machine with Ubuntu MATE's X11 session, which is 207's VM run.
