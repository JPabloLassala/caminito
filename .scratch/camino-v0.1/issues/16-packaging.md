# Packaging

Type: grilling
Status: resolved
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

How does 'MATE on Camino' get installed? Decide:

- the first target (Arch and the AUR?) and the package split: Camino itself, the palette helper, the locker if [12](12-screen-locking.md) ships one, and the patched MATE components;
- how carried patches are maintained and rebased, and which MATE baseline they apply to. The list so far:
  - mate-session: `GDK_BACKEND` unset and no `windowmanager` component (08);
  - mate-panel: Alt+F1/F2, Show Desktop, the pager's coordinates and layout keys, the minimise rectangle, and Shut Down (06, 08, 09);
  - mcc: WM identity and touchpad (06);
  - msd: media-keys, keybindings, OSD, background, and the xsettings mirror (06, [ADR 0005](../../../docs/adr/0005-msd-mirrors-mate-settings-into-gnome-schemas.md));
  - mate-screenshot: `<Alt>Print` (05).
- the MATE package conflicts, or the parallel installs, those patches need, given that they fall back to normal behaviour without Camino;
- the files Camino ships: `mate-camino.desktop` (08), the `org.mate.Camino` schema (09), the `50-camino-*.xml` keybinding manifests (05), and the capplet `.desktop` overrides (06);
- runtime dependencies, including Marco's schemas (06, 09).

## Answer

Decided on 2026-09-28 in three rounds of grilling. The user took the recommended answer on every question. The inputs were what 05–15 handed on, and a check of Arch's `extra` packages, the AUR rules and upstream `mate-desktop` on GitHub, all on 2026-09-28. The relevant facts are quoted inline.

Replacing stock MATE and gating every patch on Camino are recorded in [ADR 0011](../../../docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-camino.md).

### Summary

| Item | Decision |
|---|---|
| Target | Arch, as PKGBUILDs in the camino repo built with `makepkg -si`, for the developer only. They follow AUR conventions, so publishing them later only takes making the repo public (a separate decision after v0.1). |
| Packages | Six. `camino-git`, plus five patched packages: `mate-session-manager-camino`, `mate-panel-camino`, `mate-control-center-camino`, `mate-settings-daemon-camino` and `mate-utils-camino` |
| Replace or parallel | Replace: `provides=(foo=$pkgver)` and `conflicts=(foo)`, with no `replaces` and no parallel install |
| Baseline | Each component at its latest 1.29.x tag, or at a pinned master commit where it has none (mate-panel, mate-session). It moves to 1.30 tags once they exist. |
| Patches | `git format-patch` series beside each PKGBUILD, in `packaging/arch/<pkgname>/NNNN-*.patch`, applied in `prepare()` |
| Gating | Every carried patch is gated on Camino. Start-up behaviour is gated on `MATE_WM_NAME=Camino`, and calls to Camino's API on an owner of `org.mate.Camino`. Three protocol-generic changes stay ungated. |
| Capplet overrides | None. The mcc patch forces the X11 backend for the Mouse and Keyboard capplets under Camino. |
| Camino's build | A top-level Meson project for the two helpers and the data files. The PKGBUILD runs cargo separately, as the Arch Rust guidelines do. |
| Dependencies | `camino-git` depends on the five patched packages by their real names, plus the session's other components |
| Versions | `pkgver` is the upstream version, or `git describe` for a pinned commit; `pkgrel` for patch-only changes |
| CI | `makepkg` and `namcap` on every package when `packaging/arch/` changes. The weekly patch-on-head job stays as 14 defined it. Nothing is published. |
| Baseline moves | Only on purpose: a wanted upstream tag, or a failed weekly job. One commit per move. |

### Facts this rests on

- **Arch `extra`:**
  - Mostly MATE 1.28: mate-session-manager 1.28.0, mate-panel 1.28.7, mate-control-center 1.28.1, mate-settings-daemon 1.28.0, mate-utils 1.28.0, mate-screensaver 1.28.0, caja 1.28.0 and mate-desktop 1.28.2.
  - marco and mate-notification-daemon are on 1.29.0.
  - Every one of these is its own single-package pkgbase, built with autotools from a tag.
  - mate-screenshot comes from mate-utils.
  - marco ships `org.mate.marco.gschema.xml`, plus `libmarco-private.so.2` with its headers and `.pc`.
- **Upstream:**
  - 1.28 is the stable series, and 1.29.x is the development series leading to 1.30.
  - 1.29.1 was tagged in Aug–Sep 2026 for marco, mcc, msd, mate-utils and caja, but not for mate-panel or mate-session.
  - mate-panel's libxfce4windowing window list and pager are only on master (14). mate-panel master checks for them in `configure.ac:219` as well as in `meson.build`, so Arch's autotools build keeps working.
  - msd's wlrandr plugin needs 1.29.1 (14).
- **Dependencies in `extra`:** gtk-session-lock 0.2.0, gtk-layer-shell 0.10.1, xorg-xwayland 24.1.13, seatd 0.9.3 (which provides `libseat.so`), libinput, libxkbcommon and mesa.
  - The Wayfire session is `mate-wayland-session` in the AUR. It's installed on the developer's machine and ships `MATE.desktop`.
- **AUR:**
  - A package that only adds patches to a repo package is allowed if it has its own name and `conflicts=(orig)`.
  - The usual pattern is `provides=(foo=$pkgver)` plus `conflicts=(foo)`, as in gnome-shell-performance, mutter-performance and xorg-server-tearfree. `replaces` is almost never used.
  - `-git` is required only for VCS sources that aren't pinned.
- **pacman** has no divert mechanism: two packages owning one path is a hard conflict.
- **Rust:**
  - The Arch guidelines use `cargo fetch --locked` in `prepare()`, then `cargo build --frozen` and `cargo test --frozen`.
  - Git-pinned dependencies are fetched from `Cargo.lock`. Arch's own niri and cosmic-comp packages pin Smithay by git `rev` this way.

### Target and channel

- **v0.1 is packaged for the developer only.** The PKGBUILDs live in the (private) camino repo and are built with `makepkg -si`. Nothing is published, not to the AUR and not as a binary repo.
- **They follow AUR conventions**, so any of them can be published later without changes: names, `provides` and `conflicts`, the `-git` suffix rules, and patches beside the PKGBUILD. Publishing needs a public source, so it's a separate decision after v0.1.
- **Only Arch.** 14 already found that Fedora and Debian are further behind, and nothing here is built to be distro-neutral.

### Layout

```
meson.build                     top-level: the helpers and the data files
packaging/arch/
  camino-git/PKGBUILD
  mate-session-manager-camino/{PKGBUILD,NNNN-*.patch}
  mate-panel-camino/{PKGBUILD,NNNN-*.patch}
  mate-control-center-camino/{PKGBUILD,NNNN-*.patch}
  mate-settings-daemon-camino/{PKGBUILD,NNNN-*.patch}
  mate-utils-camino/{PKGBUILD,NNNN-*.patch}
```

This moves the second round's `packaging/patches/<component>/` beside the PKGBUILDs, because makepkg and the AUR expect local sources in the PKGBUILD's directory.

### Camino's package

- **One package, `camino-git`.** It holds:
  - `/usr/bin/camino`, `camino-gtk-palette` and `camino-locker`;
  - `/usr/share/wayland-sessions/mate-camino.desktop` (08);
  - the `org.mate.Camino` schema in `/usr/share/glib-2.0/schemas/` (09);
  - the `50-camino-*.xml` keybinding manifests in mcc's keybindings directory (05, 06).

  There is no PAM file (12), no D-Bus service file, since Camino claims its names itself (15), and no systemd unit (08).
- **The locker isn't split out.** It's the default `locker-command`, and the lock fails closed (12), so a Camino without it would be a broken default.
- **It's `-git` until v0.1.0 is tagged**, because the source is an unpinned checkout.
  - The PKGBUILD builds the local checkout's committed `HEAD` through a `git+file://` source worked out from `$startdir`, so uncommitted work is never packaged. It switches to the GitHub URL if it is ever published.
  - `pkgver()` is `git describe --long --tags`, with the VCS guideline's `r<count>.<hash>` fallback before the first tag.
  - A tagged `camino` package replaces it at v0.1.0.
- **The build, as a top-level Meson project.**
  - The root `meson.build` sets `subproject_dir: 'helpers'` and calls `subproject('camino-gtk-palette')` and `subproject('camino-locker')`. Each helper stays a standalone Meson project (15).
  - The root project installs the session file, the schema and the manifests. `tools/marco-frame-dump` isn't included, because it never ships (14).
  - Schemas are compiled by Arch's pacman hook, not by `meson install`.
- **Cargo stays plain cargo**, driven by the PKGBUILD:
  - `prepare()` runs `cargo fetch --locked`;
  - `build()` runs `cargo build --frozen --release`, then `meson setup` and `meson compile`;
  - `check()` runs `cargo test --frozen` for the unit and policy tests;
  - `package()` runs `meson install` and `install -Dm755 target/release/camino`.

  Meson never builds Rust, and cargo never builds C (15). The release binary has no headless backend, pixman renderer or test binaries (14, 15).
- **Dependencies:**
  - **The patched packages, by their real names:** the five `-camino` packages, not the stock names they provide. The patched msd is a hard dependency (ADR 0006, 11). The `conflicts` lines stop stock packages from satisfying it or coming back.
  - **The rest of the session:** caja, marco (for the `org.mate.Marco.*` schemas, 09), mate-screensaver (its PAM service and `lock-dialog-*.ui`, 12), mate-notification-daemon, mate-polkit, zenity (05) and xorg-xwayland.
  - **Libraries:** gtk-layer-shell, gtk-session-lock, libxfce4windowing, gtk3, glib2, mate-desktop (`MateBG`, 12), pam, seatd, libinput, libxkbcommon, mesa, systemd-libs, cairo, pango and librsvg. Anything else the binaries link is found by `namcap`.
  - **Optional:** swaylock and gtklock (through `locker-command`), onboard (with its udev rule, 13) and orca.
  - **Build:** rust, cargo, meson, git, and the headers above.
- **No meta-package.** The map rules out Camino without MATE, so `camino-git` itself pulls in the session.

### The patched MATE packages

- **Five pkgbases**, each named after the Arch package it replaces, with a `-camino` suffix:

  | Package | Replaces | Baseline | Patches |
  |---|---|---|---|
  | `mate-session-manager-camino` | mate-session-manager | pinned master commit (no 1.28 or 1.29 tag on master) | `GDK_BACKEND` unset, no `windowmanager` component (08) |
  | `mate-panel-camino` | mate-panel | pinned master commit, with libxfce4windowing on | Alt+F1/F2 over `org.mate.Panel`, Show Desktop, pager layout and coordinates, the workspace count and names, the minimise rectangle, Shut Down (06, 08, 09) |
  | `mate-control-center-camino` | mate-control-center | 1.29.1 or later 1.29.x tag | WM identity, touchpad, 5% scale steps (06, 13), the X11 backend for Mouse and Keyboard (below) |
  | `mate-settings-daemon-camino` | mate-settings-daemon | 1.29.1 or later 1.29.x tag | media-keys, keybindings, OSD, touchpad, background, the xsettings mirror-only path, the typing-break skip (06, 09, 11, 13) |
  | `mate-utils-camino` | mate-utils | 1.29.1 or later 1.29.x tag | `<Alt>Print` in mate-screenshot (05, 06) |

- **Each starts from Arch's own PKGBUILD** for that package, with the smallest possible diff. The diff changes:
  - the source (tag, or `#commit=`);
  - `provides=(foo=$pkgver)` and `conflicts=(foo)`;
  - the patch files, with their `sha256sums`, and `prepare()` applying them in order;
  - any dependency the baseline or the patches add: libxfce4windowing for mate-panel, and gtk-layer-shell for msd's OSD.

  Arch's autotools build is kept, so when Arch changes its PKGBUILD, the diff shows what to carry over.
- **Pinned master commits are not `-git`.** AUR convention requires `-git` only for sources that aren't pinned.
- **The replaced packages keep working as stock MATE.** X11 MATE and the Wayfire session behave exactly as before (Gating, below). Anything that depends on `mate-panel>=1.28`, including `mate-wayland-session`, is satisfied through `provides`.
- **mate-screensaver, caja and marco aren't patched** (06, 12). They come from `extra`.

### Carried patches

- **Storage:** each patch is a `git format-patch` file, numbered in application order, beside its PKGBUILD. The patch files are the source of truth. There are no pushed forks.
- **Rebasing:**
  1. The developer keeps a local checkout of each component at its baseline.
  2. They run `git am` on the series, move to the new baseline with `git rebase`, and fix what breaks.
  3. They re-export with `git format-patch`, then update `source`, `pkgver` and `sha256sums` in the same commit.
- **Gating** ([ADR 0011](../../../docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-camino.md)). Every carried patch is inert unless Camino is running, because the packages replace stock MATE system-wide.
  - **Start-up behaviour** is gated on `MATE_WM_NAME=Camino`, which Camino exports into the session, D-Bus activation and systemd (08). This covers:
    - mate-session's two changes;
    - msd's media-keys, keybindings, OSD, touchpad, background, xsettings mirror-only and typing-break changes;
    - mcc's WM identity, touchpad, 5% steps and X11 backend;
    - mate-panel's `org.mate.Panel` `MainMenu` and `RunDialog` export, and the pager's writes to `org.mate.Camino`.
  - **Calls to Camino's API** are gated on an owner of `org.mate.Camino`: mate-panel's Show Desktop through `ShowingDesktop`, and mate-screenshot's `GetActiveToplevel`. With no owner, each keeps its stock behaviour, which is libxfce4windowing's minimise-all and the existing Wayland refusal of `--window`.
  - **Ungated**, because they're protocol-generic and correct on any compositor:
    - the minimise rectangle through wlr-foreign-toplevel `set_rectangle`;
    - the pager's use of ext-workspace `coordinates` when every workspace has them;
    - Shut Down taking the session-manager path whenever `org.gnome.SessionManager` has an owner (08).
  - **What changes from earlier tickets:** 06's mcc touchpad patch and 13's 5% steps were "on Wayland". They are now gated on Camino, so the Wayfire session's capplets stay stock.
- **The Mouse and Keyboard capplets:** 06's `.desktop` overrides are dropped.
  - pacman can't let one package override a file another package owns.
  - Editing `Exec=` in mcc's own files would push the capplets onto XWayland under Wayfire too.

  Instead, the mcc patch calls `gdk_set_allowed_backends("x11")` before `gtk_init` in those two capplets when `MATE_WM_NAME=Camino`. The touchpad check uses the same gate, since under Camino the capplet itself runs on X11.

### Versions

- **`pkgver`** is the upstream version: `1.29.1` for a tag, or `git describe`'s form for a pinned commit, such as `1.28.7.r<N>.g58ac6c1` for mate-panel.
- **`pkgrel`** goes up when only the patches or the PKGBUILD change, and when Arch bumps a linked library's soname (for example libxfce4windowing).
- **`provides` carries the same version**, so version constraints on the stock names still hold.

### CI

This extends 14's CI.

- **On push and pull request, when `packaging/arch/` changes:** `makepkg` on each package in the existing `archlinux:latest` container, followed by `namcap` on the PKGBUILDs and on the built packages. Packages are built in dependency order and installed into the container as they're built, so `camino-git` builds against the patched ones. The built packages are thrown away, and nothing is published.
- **Weekly (14):** each package's patch series is applied to its component's upstream git head and built. A failure opens or updates an issue, as the Smithay canary does.

### Moving the baseline

- **The baseline only moves on purpose.** The `-camino` packages conflict with the stock ones and carry no `replaces`, so `pacman -Syu` never swaps them for Arch's 1.30 packages.
- **It moves when the developer wants a new upstream tag, or when the weekly job fails.** Each move is one commit per component that updates the source, re-exports the patches and sets `pkgver`, following 14's rule for Smithay: one pinned revision, bumped on purpose, with a canary.
- **At 1.30:** once mate-panel and mate-session have 1.30 tags, every component moves to a tag, and the pinned commits go away.

### Handed on

- **17 Milestone slicing:**
  - The first milestone includes the top-level `meson.build` and `camino-git`'s PKGBUILD, so the developer runs a packaged Camino from the start.
  - The five patched packages come with the milestone that first needs the full session. mate-session's patch is needed first, for `camino --session`.
  - The PKGBUILD CI job arrives with the first package, and the weekly patch-on-head job with the first patch.
  - "v0.1 done" includes `camino-git` and the five patched packages building in CI and installed on the developer's machine, with X11 MATE still working (the fall-back rule).
