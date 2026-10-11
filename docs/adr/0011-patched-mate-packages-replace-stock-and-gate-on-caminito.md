# 11. Patched MATE packages replace the stock ones, and every carried patch is gated on Caminito

Date: 2026-09-28
Status: Accepted. Qualified on 2026-10-05 by ticket 71: a patched package can also have a build patch, which isn't a carried patch. Qualified the same day by ticket 81: caja is a sixth patched component, `caja-caminito`, on Arch's own version, since its one patch needs nothing newer. Qualified on 2026-10-06 by ticket 88, and still to be confirmed by the developer: a patched package can also have an upstream fix, which isn't a carried patch. Qualified on 2026-10-06 by ticket 73: a patched package can also install the stock library of an soname its baseline dropped. Qualified on 2026-10-07 by ADR 0017: a patched package can also build a panel applet from outside MATE, `vala-panel-appmenu-mate-caminito`, whose baseline is the AUR's package and not one of Arch's. Qualified on 2026-10-08 by ticket 107: mate-power-manager is a seventh patched component, `mate-power-manager-caminito`, on Arch's own version, since its one patch needs nothing newer; it is the package ADR 0014 starts on XWayland. Qualified on 2026-10-10 by ticket 111: `appmenu-gtk-module-caminito` is Arch's appmenu-gtk-module with two upstream fixes for Wayland and no carried patch, an optional dependency of `caminito-git` (ADR 0017). Qualified on 2026-10-11 by ADR 0029: on Debian and Ubuntu a patched package keeps the stock names at a `caminito` version and is pinned through apt, in place of `-caminito` names. Qualified the same day by ADR 0030: the patch series are kept once, at `packaging/patches/<component>/`, not beside each PKGBUILD.

## Context

The session needs carried patches to five MATE components: mate-session-manager, mate-panel, mate-control-center, mate-settings-daemon and mate-utils (06, 08, 09, 11, 13). None of them can be installed next to the stock version without renaming binaries, gsettings schemas and D-Bus names. pacman has no divert mechanism, so two packages owning one path is a hard conflict.

The developer's machine also runs X11 MATE and the Wayfire-based `mate-wayland-session` from the AUR, and both must keep working (14).

The patches written so far didn't agree on when they turn on:
- mate-session and some of msd were gated on `MATE_WM_NAME=Caminito`;
- some mcc patches were gated on "any Wayland display", which includes Wayfire;
- several had no gate at all.

## Decision

- **Replace, don't install alongside.** Each patched component is a `<arch-pkgname>-caminito` package with `provides=(<arch-pkgname>=$pkgver)` and `conflicts=(<arch-pkgname>)`, built from Arch's own PKGBUILD with the smallest possible diff. There's no `replaces`, so pacman never switches between stock and patched on its own. `caminito-git` depends on the `-caminito` packages by their real names, so a stock msd can never satisfy it (ADR 0006).
- **Every carried patch is inert without Caminito.**
  - Start-up behaviour is gated on `MATE_WM_NAME=Caminito`.
  - Calls to Caminito's D-Bus API are gated on an owner of `org.mate.Caminito`.
  - The only ungated changes are protocol-generic ones that are correct on any compositor:
    - the foreign-toplevel minimise rectangle;
    - the pager's use of ext-workspace `coordinates`;
    - the window list's use of each window's workspace, where libxfce4windowing has it (ADR 0013);
    - Shut Down through `org.gnome.SessionManager` when it has an owner.
- **A build patch isn't a carried patch.** Where a baseline doesn't build against Arch's libraries, the series can end with a patch that takes out the upstream change that needs the newer library. It has no gate, and it's allowed only when what it leaves is what the stock package does. It's removed when Arch has the library. It's kept in the series' `arch/`, and applies after the rest (ADR 0030). The first is `mate-settings-daemon-caminito`'s `0004`: msd 1.29.1 needs mate-desktop 1.29.0 for one commit of the X11 xrandr plugin, and Arch has 1.28.2.
- **An upstream fix isn't a carried patch either.** Where the baseline has a bug of upstream's own that the session meets, the series can have a patch that fixes it as upstream would. It has no gate, since a gate would only keep the bug for the other sessions, and it's allowed only when it's right on any compositor and changes nothing where the bug doesn't show. It's meant to be sent upstream, and it's removed when the baseline has the fix. The first is `mate-settings-daemon-caminito`'s `0006`: msd's Wayland background left more layer surfaces at each hot-plug (88).
- **A dropped soname's library is kept.** Where the baseline's library has a newer soname than Arch's, and Arch's other packages are linked to the old one, the patched package also builds the old library from Arch's version and installs it beside the new one, without the link that programs are built against. Otherwise installing the package would break those programs in every session. The first is `mate-panel-caminito`'s `libmate-panel-applet-4.so.1`, for the applets of seven other packages (73). It helps only a program of its own: one loaded into the patched component's process meets both libraries there, as `mate-notification-daemon`'s applet does in the panel.
- **The patches are `git format-patch` series, kept once at `packaging/patches/<component>/`** (ADR 0030), applied to a 1.29.x tag or, where a component has none, a pinned master commit. They move only on purpose.

## Consequences

- X11 MATE and the Wayfire session run the patched binaries but get stock behaviour, but for an upstream fix, which they get too. So "a carried patch falls back to normal behaviour" (CONTEXT.md) covers Wayfire too, not just X11. A patch that turns on under Wayfire is a bug.
- The mcc touchpad and 5% scale-step patches (06, 13) are now gated on Caminito instead of Wayland. The Mouse and Keyboard capplets pick the X11 backend in code under Caminito, instead of through `.desktop` overrides that pacman couldn't install.
- The patched packages don't follow Arch's MATE updates, including 1.30. Every baseline move is a manual rebase, made cheaper by the minimal PKGBUILD diffs and 14's weekly patch-on-head job.
- Rejected:
  - Parallel installs under a Caminito prefix, which would duplicate schemas and D-Bus files and need `XDG_DATA_DIRS` and `PATH` games in the session.
  - Gating patches on "Wayland", which would change the Wayfire session.
  - Forks pushed to GitHub, with a second place for the patches to live.
  - A `mate-caminito` meta-package, since Caminito without MATE is out of scope.
