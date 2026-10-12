# mate-panel, the patched package for Ubuntu

Epic: debian-packaging
Type: implementation
Status: open: the first criterion's podman run is the developer's, as 194's is: everything that runs in the container passed in an Ubuntu 26.04 chroot. ADR 0029's second pin stanza waits on the developer's word
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

- [x] It builds and tests in `tools/debian-build`, with every patch applied. Its binary packages are the ones Ubuntu's has, with the differences listed here.
- [x] In the container it replaces Ubuntu's, and `apt-cache policy` shows the pin.
- [x] With Ubuntu's `mate-applets` installed, `apt` keeps `libmate-panel-applet-4-1` and doesn't remove an applet.

## Done (2026-10-12)

- **Run in a chroot, not in podman, as 194's was.** This Session had no podman and no running Docker daemon. `tools/debian-build --in-container` ran as root in an `mmdebstrap --variant=apt` chroot of resolute (main and universe), with the worktree bind-mounted read-only. It cloned master `58ac6c1` with mate-submodules, applied all eleven patches, built, ran `dh_auto_test`, passed lintian and installed the six `.deb`s. The host half isn't changed from 194's.
- **mate-panel has no tests.** `make check` runs nothing, and Arch's PKGBUILD has no `check()`. caminito was rebuilt by the same script after the change and passed everything: 2656 passed, none failed, 10 ignored. lintian's warnings were the same as 194's.
- **The source comes from upstream's git.** A file `packaging/debian/mate-panel/upstream` names the git URL, the commit and the `packaging/patches/` component. `tools/debian-build` clones the commit with its submodules, then copies the shared series and then any `debian/` build patches into `debian/patches/` with a `series` file. It stops if one of the three keys is missing. mate-panel has no Debian build patch. mate-submodules comes from the submodule master names, not from Ubuntu's `mate-submodules-source`.
- **The version is `1.29.0~git20260925.58ac6c1-0caminito1`.** 1.29.0 is master's `AC_INIT`, and the date is the commit's. Arch says 1.28.7 so that pacman constraints hold. Ubuntu's 1.27.1 is below either.
- **Ubuntu's patches:** master has 0001 to 0006 and `fix-ftbfs-gettext`. The RDA patches, 0101 to 0106, were dropped and not rebased. 0103 doesn't apply. 0101 applies only with fuzz, and puts a table entry with an extra field into `PanelAction`, whose layout master changed. RDA only adds a "suspend session" action in remote sessions, which Arch's package doesn't have either. `librda-dev` left the build dependencies.
- **Binary packages:** Ubuntu's six, with `libmate-panel-applet-4-1` replaced by `libmate-panel-applet-4-2` (`.so.2`). The `-dev` package's `.so` link points at `.so.2`. `mate-panel` now has the four applets as modules in `/usr/lib/x86_64-linux-gnu/mate-panel/` (`--with-in-process-applets=all`). It no longer has their `/usr/libexec` programs or D-Bus services. The library has no symbols file: `dh_makeshlibs -V` makes the dependency `>= 1.29.0~git…`. Configured with `--enable-wayland` and `--disable-eds`, as on Arch. `Wayland support: yes`.
- **The pin needed a second stanza.** In a clean chroot, Ubuntu's `mate-panel` and `mate-applets` were installed first, then `apt install ./*.deb`. apt upgraded the panel, kept `libmate-panel-applet-4-1` and `mate-applets`, and removed nothing. But apt gives a `version` pin only to versions from a source. A `.deb` installed from a file is in dpkg's status alone, at 100. So `apt-cache policy` showed no pin, and a fake `mate-panel 1.30.0-1` from an `Origin: Ubuntu` repository was taken on `apt upgrade`. The pin file now has a second stanza for the same binaries, `Pin: release o=Ubuntu` at 99. With it, `apt-cache policy` lists Ubuntu's versions at 99, the upgrade keeps the patched package, and an explicit `apt install mate-panel=<version>` still works. ADR 0029 is qualified, "still to be confirmed by the developer". The spec's bullet and a note in 195 follow.
- **The pin is written by `rules`** (`execute_after_dh_install`) from `debian/control`'s package list. Other packages copy it, changing `debian/mate-panel`. It's a conffile, so `apt remove` leaves it until a purge. lintian's `package-installs-apt-preferences` error is overridden.
- **lintian:** no error. Its warnings: `gir-package-name-does-not-match`, which comes from Ubuntu's own layout, and `package-has-long-file-name` for the long version.
- **Not checked:** running the panel. There was no display in the chroot. Whether Ubuntu's 1.26 applets work with the patched panel is still the spec's open question.
