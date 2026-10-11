# Map: Debian packaging

Status: closed on 2026-10-11. It reached its destination, [the spec](spec.md), in one grilling session with the developer.

## Destination

A spec, ready to cut into implementation tickets, for building and installing Caminito and every one of its components on Ubuntu as Debian packages, the way `tools/bootstrap` does on Arch.

## Notes

- **Domain:** **Patched package** now names any distribution's package, not Arch's alone, and **Backported package** is new. Both are in [CONTEXT.md](../../CONTEXT.md).
- **Not in a milestone.** It gates nothing of v0.1, and its tickets say `Milestone: none: after v0.1`.
- **Facts gathered for the session** (2026-10-11, from the archive's source indices, upstream tags and this tree). They weren't compiled, so the tickets confirm them:
  - Ubuntu 26.04 has mate-desktop 1.28.2 (Arch's), gtk-session-lock 0.2.0, libdisplay-info 0.3.0, libxfce4windowing 4.20.5, gtk-layer-shell 0.10.0, wayland-protocols 1.47 and rustc 1.93. Its other MATE packages are 1.26, and mate-panel is 1.27.1.
  - Every patch series applies to its baseline, and every baseline configures against 26.04 but mate-control-center 1.29.1, which needs marco ≥ 1.29.0. Ubuntu has 1.26.2.
  - Ubuntu's `libmate-panel-applet-4-1` is the same soname, `.so.1`, as Arch's, and Ubuntu's caja 1.26.4 has caja 1.28.0's extension ABI (5).
  - Ubuntu's `mate-screensaver` ships `/etc/pam.d/mate-screensaver`, and `mate-screensaver-common` ships `lock-dialog-default.ui` with every id caminito-locker reads.
  - The one runtime break in Caminito is `caminito/src/session.rs`'s look into `/var/lib/pacman/local`. The patches name no Arch path.
  - Debian 13 and Ubuntu 24.04 have MATE 1.26 and mate-desktop 1.26. Ubuntu 24.04 lacks gtk-session-lock and has rustc 1.75.

## Decisions so far

From the grilling session of 2026-10-11:

- **Built locally from source packages, with `dpkg-buildpackage`**, as `makepkg -si` builds on Arch. A public apt repository is later, after `v0.1.0`.
- **Every component is a package of its own**, as on Arch.
- **Arch stays the reference platform.** Ubuntu is checked by building every package in a container and by a run in a VM before a milestone closes.
- **Ubuntu 26.04 LTS only, `amd64` only.** Debian is next once forky freezes. Debian 13 and Ubuntu 24.04 are out.
- **Sources at `packaging/debian/<package>/debian/`.** `tools/debian-build` builds them in a podman container of `ubuntu:26.04`, running each package's tests. `tools/bootstrap-debian` installs the build dependencies, builds and installs on the machine itself.
- **Carried patches are kept once,** at `packaging/patches/<component>/`, with `arch/` and `debian/` beside them for build patches alone ([ADR 0030](../../docs/adr/0030-carried-patches-are-kept-once-for-every-distribution.md)).
- **On Ubuntu a patched or backported package keeps the stock names** at a version with `caminito` in it. Each pins its own binaries through apt ([ADR 0029](../../docs/adr/0029-on-debian-a-patched-package-keeps-the-stock-names-and-is-pinned.md)).
- **Each package's `debian/` starts from Ubuntu's own,** moved to the new baseline, as each PKGBUILD starts from Arch's.
- **mate-panel builds no `.so.1` on Ubuntu.** Ubuntu's `libmate-panel-applet-4-1` stays installed beside the patched `libmate-panel-applet-4-2`.
- **marco 1.29.1 is a backported package** on Ubuntu. It's built as upstream has it, for mate-control-center and for the parity table's schema.
- **Caminito finds the patched appmenu-gtk-module by a file that package installs,** not by asking a package manager.
- **The VS Code extension is linked by dpkg triggers** running the same link script as the alpm hooks, now shared, which learns VSCodium's `.deb` path.
- **Rust comes from rustup's `stable`**, as on Arch, and the build fetches crates with `cargo fetch --locked`.
- **Packages:** `caminito`, `caminito-vscode` and `caminito-desktop`, which depends on `mate-desktop-environment-extras`. The version is `0.0~r<count>.g<hash>-1` until `v0.1.0`.
- **mate-session's skip and move lists learn Ubuntu MATE's X11-only autostarts.**
- **Docs:** a README section, *Building on Ubuntu 26.04*, and a VM step in the checklist.
- **The epic is `debian-packaging`**, tickets 191 to 207.

## Not yet specified

Nothing. What still has to be seen is in the spec's further notes.
