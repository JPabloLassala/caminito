# 11. Patched MATE packages replace the stock ones, and every carried patch is gated on Camino

Date: 2026-09-28
Status: Accepted. Qualified on 2026-10-05 by ticket 71: a patched package can also have a build patch, which isn't a carried patch.

## Context

The session needs carried patches to five MATE components: mate-session-manager, mate-panel, mate-control-center, mate-settings-daemon and mate-utils (06, 08, 09, 11, 13). None of them can be installed next to the stock version without renaming binaries, gsettings schemas and D-Bus names. pacman has no divert mechanism, so two packages owning one path is a hard conflict.

The developer's machine also runs X11 MATE and the Wayfire-based `mate-wayland-session` from the AUR, and both must keep working (14).

The patches written so far didn't agree on when they turn on:
- mate-session and some of msd were gated on `MATE_WM_NAME=Camino`;
- some mcc patches were gated on "any Wayland display", which includes Wayfire;
- several had no gate at all.

## Decision

- **Replace, don't install alongside.** Each patched component is a `<arch-pkgname>-camino` package with `provides=(<arch-pkgname>=$pkgver)` and `conflicts=(<arch-pkgname>)`, built from Arch's own PKGBUILD with the smallest possible diff. There's no `replaces`, so pacman never switches between stock and patched on its own. `camino-git` depends on the `-camino` packages by their real names, so a stock msd can never satisfy it (ADR 0006).
- **Every carried patch is inert without Camino.**
  - Start-up behaviour is gated on `MATE_WM_NAME=Camino`.
  - Calls to Camino's D-Bus API are gated on an owner of `org.mate.Camino`.
  - The only ungated changes are protocol-generic ones that are correct on any compositor:
    - the foreign-toplevel minimise rectangle;
    - the pager's use of ext-workspace `coordinates`;
    - the window list's use of each window's workspace, where libxfce4windowing has it (ADR 0013);
    - Shut Down through `org.gnome.SessionManager` when it has an owner.
- **A build patch isn't a carried patch.** Where a baseline doesn't build against Arch's libraries, the series can end with a patch that takes out the upstream change that needs the newer library. It has no gate, and it's allowed only when what it leaves is what the stock package does. It's removed when Arch has the library. The first is `mate-settings-daemon-camino`'s `0004`: msd 1.29.1 needs mate-desktop 1.29.0 for one commit of the X11 xrandr plugin, and Arch has 1.28.2.
- **The patches are `git format-patch` series beside each PKGBUILD**, applied to a 1.29.x tag or, where a component has none, a pinned master commit. They move only on purpose.

## Consequences

- X11 MATE and the Wayfire session run the patched binaries but get stock behaviour. So "a carried patch falls back to normal behaviour" (CONTEXT.md) covers Wayfire too, not just X11. A patch that turns on under Wayfire is a bug.
- The mcc touchpad and 5% scale-step patches (06, 13) are now gated on Camino instead of Wayland. The Mouse and Keyboard capplets pick the X11 backend in code under Camino, instead of through `.desktop` overrides that pacman couldn't install.
- The patched packages don't follow Arch's MATE updates, including 1.30. Every baseline move is a manual rebase, made cheaper by the minimal PKGBUILD diffs and 14's weekly patch-on-head job.
- Rejected:
  - Parallel installs under a Camino prefix, which would duplicate schemas and D-Bus files and need `XDG_DATA_DIRS` and `PATH` games in the session.
  - Gating patches on "Wayland", which would change the Wayfire session.
  - Forks pushed to GitHub, with a second place for the patches to live.
  - A `mate-camino` meta-package, since Camino without MATE is out of scope.
