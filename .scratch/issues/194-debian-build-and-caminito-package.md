# tools/debian-build, and the caminito package

Epic: debian-packaging
Type: implementation
Status: open: the first criterion's podman run is the developer's: what runs in the container passed in an Ubuntu 26.04 chroot, but this Session had no podman
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
- [x] `dpkg -c` of the `.deb` lists what `caminito-git`'s package has: the binaries, the helpers, the session entry, the schema, the logo, the keybinding manifests and caminito-displays's entry.
- [x] `lintian` reports no error. Warnings that come from the local, networked build are listed in the ticket.

## Done (2026-10-12)

- **Run in a chroot, not in podman.** The Session had no podman and no Docker daemon, so the in-container half, `tools/debian-build --in-container <checkout> caminito`, ran as root in an `mmdebstrap --variant=apt` chroot of resolute (main and universe, as the image has), with the checkout bind-mounted read-only at its own path and `out/` bound at `/out`. It built, ran every test, passed lintian and installed `caminito_0.0~r209.gdd21b0c-1_amd64.deb`. The host half (argument checks, the mounts, `podman run`) ran only as far as its argument check: the developer's first `tools/debian-build caminito` closes the first criterion.
- **Tests: every one passes in 26.04,** caminito's 1832 (5 ignored) among them, under `dbus-run-session` as `check()` runs them.
- **Two things a bare container lacks broke the X11 tier and were fixed in the script, not in rules:** `/tmp/.X11-unix`, which systemd-tmpfiles makes on a machine and smithay doesn't, and an open files' limit above 1024. `runuser`'s PAM session dropped the soft limit to 1024, which the parallel tests went past, so the build user is entered with `setpriv`, which keeps the container's limit. On a machine (205's `tools/bootstrap-debian`) both are already there.
- **Ubuntu's `pam.pc` requires audit's** and `libpam0g-dev` doesn't bring it, so `libaudit-dev` is a build dependency.
- **Three binaries, not two:** `rules` installs `caminito`, `caminito-displays` and `caminito-about`, as the PKGBUILD does; caminito-about came after the spec.
- **`dpkg -c`** lists the five binaries in `/usr/bin` (the three and the two helpers), `wayland-sessions/mate-caminito.desktop`, the schema, the logo, the three keybinding manifests, and caminito-displays's, caminito-about's and System Shortcuts's entries under `/usr/share/caminito/xdg/applications`.
- **`Depends`** are `${shlibs:Depends}` and the run-time ones by Ubuntu's names: librsvg2-common, mate-themes, xwayland, libegl1, libegl-mesa0, libgl1-mesa-dri, zenity, mate-screensaver, mate-notification-daemon, mate-polkit, hicolor-icon-theme. The patched packages and marco are 205's. `Recommends` are appmenu-gtk-module and caminito-vscode, unversioned until 205 and 195 say otherwise.
- **The version** is set by `tools/debian-build` in the changelog's first line; the committed entry carries `0.0~r0.g0000000-1`. `CAMINITO_REVISION` is the Debian upstream version, so `caminito --version` agrees with `dpkg -l`. The package's `debian/` is taken from the archived HEAD, not the working tree.
- **lintian: no error.** Its warnings: `no-manual-page` for the five binaries, `initial-upload-closes-no-bugs`, and `debug-file-with-no-debug-symbols` three times on `caminito-dbgsym`, since the release build has no debug info (the PKGBUILD's `!debug`). Turning the dbgsym package off with `dh_strip --no-automatic-dbgsym` was tried, but only on an incremental rebuild, which left a `.dwz` warning behind, so it was left out rather than shipped unverified. The `.ddeb` isn't copied to `out/`.
- **After 210, four binaries.** Merging master brought `caminito-wallpaperd`, which `rules` now installs beside the other three, and `mate-desktop-common`, whose `org.mate.background` it reads, is a run-time dependency. Rebuilt in the chroot: four `active_toplevel::*::x11` tests timed out (5 s) on the first run, the first X11 tests to start, on 4 cores; alone and in a second full run every test passed, so they look slow on a cold Xwayland rather than broken. `dpkg -c` then lists the six binaries, and lintian has no error (`debug-file-with-no-debug-symbols` is four times now).
- **Not run:** `/code-review` (the work was reviewed by reading it again), and lintian's check of a source package, since only `-b` is built.
