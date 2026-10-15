# mate-session-manager, the patched package for Ubuntu

Epic: debian-packaging
Type: implementation
Status: open: the podman run of `tools/debian-build mate-session-manager` is the developer's, as 194's is: what runs in the container passed in an Ubuntu 26.04 chroot, but this Session had no podman
Milestone: none: after v0.1
Blocked by: 191, 194
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "The packages" in [the spec](../debian-packaging/spec.md) and [ADR 0029](../../docs/adr/0029-on-debian-a-patched-package-keeps-the-stock-names-and-is-pinned.md). It's Arch's `mate-session-manager-caminito`, as a Debian source package.

- `packaging/debian/mate-session-manager/debian/`, from Ubuntu 26.04's mate-session-manager 1.26.1 `debian/`, moved to the baseline, master `57a9178`, with Ubuntu's patches dropped where the baseline has them.
- The series from `packaging/patches/mate-session-manager/`, and its `debian/` build patches if any, copied into `debian/patches/` at build time.
- The version is `<baseline>-0caminito1`, and the pin `/etc/apt/preferences.d/caminito-mate-session-manager` is installed as 195's is.

## Acceptance criteria

- [x] It builds and tests in `tools/debian-build`, with every patch applied. Its binary packages are the ones Ubuntu's has, with the differences listed here.
- [x] In the container it replaces Ubuntu's, and `apt-cache policy` shows the pin.

## Done (2026-10-12)

- **Run in a chroot, not in podman, as 194 to 196 were.** This Session had no podman. `tools/debian-build --in-container` ran as root in an `mmdebstrap --variant=apt` chroot of resolute (main and universe), under `env -i` with only the proxy's variables, with the worktree bind-mounted read-only. It cloned master `57a9178` with mate-submodules, applied Ubuntu's 0005 and the four carried patches, built, ran `dh_auto_test`, passed lintian and installed both `.deb`s. It was run again after review, at the final commit.
- **mate-session-manager has no tests.** `make check` runs nothing, and Arch's PKGBUILD has no `check()`. Ubuntu's rules had turned `dh_auto_test` off. That override is gone, so the test step runs even though it does nothing. caminito was built by the same script in the same chroot, with mate-session-manager added to its list. It passed everything: 2707 passed, none failed, 12 ignored.
- **The version is `1.28.0+git20260721.57a9178-0caminito1`.** Master's `AC_INIT` is 1.28.0, and master comes 23 commits after the v1.28.0 tag, so the separator is `+git` and not mate-panel's `~git`, which would sort below 1.28.0. The spec's version rule now says so. The date is the commit's. Ubuntu's 1.26.1-3 is below it.
- **Ubuntu's patches:** master has 0006 (its commit `6c53bde`) and 0009, so both were dropped. 0005, Debian's lock-before-user-switch, isn't in master. It still applies cleanly and is kept, ahead of the carried series.
- **Binary packages:** Ubuntu's two, `mate-session-manager` and `debian-mate-default-settings`. The paths are Ubuntu's: `--libexecdir=/usr/libexec` and `--localstatedir=/var/lib`. The `x-session-manager` alternative and `55mate-session_materc` are unchanged. The build-dependency floors are master's `configure.ac`'s, and `mate-submodules-source` is gone, as in 196. Sound events, new since 1.26, are configured `--without-libcanberra`, because libcanberra isn't a build dependency here or on Arch.
- **The pin is mate-panel's `rules` target,** written to `debian/mate-session-manager`, for both binary packages. lintian's `package-installs-apt-preferences` is overridden. In the chroot, Ubuntu's 1.26.1-3 was installed first and then `apt install ./*.deb`. Both packages were upgraded and nothing was removed. `apt-get -s upgrade` then had nothing to do, and `apt-cache policy` showed 1001 for the caminito version and 99 for Ubuntu's.
- **lintian:** no error. Its warnings were `old-fsf-address-in-copyright-file`, from Ubuntu's copyright, and `package-has-long-file-name`.
- **Not checked:** starting a session, because the chroot had no display. Also not done: the spec's item that 0003 and 0004 learn `ubuntu-mate-desktop`'s X11-only autostarts. That's not this ticket's.
- **Review:** the build floors were raised and the libcanberra comment was corrected. Reviewers also noted that the pin's `printf` is now in three `rules` files. It's left as 196's comment intends: each patched package copies it.
