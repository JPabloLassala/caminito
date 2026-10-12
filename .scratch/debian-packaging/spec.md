# Spec: Debian packaging

Written on 2026-10-11 from a grilling session with the developer, whose decisions are in [the map](map.md). It gists them and links the ADRs that hold the two hard to reverse. Where they differ, the ADR is right and this file is to be fixed.

Nothing here is built.

## Problem

Caminito is packaged for Arch alone. `tools/bootstrap` builds and installs the compositor, its seven patched MATE packages, the global menu's two and the VS Code extension through `makepkg` and pacman, and the session reads pacman's database. A user of Ubuntu MATE can build the compositor by hand and run it nested, but can't install *MATE on Caminito*: Ubuntu's MATE is 1.26, older than every patched package's baseline, and nothing builds those baselines as Debian packages.

## Solution

Every Arch package under `packaging/arch/` gets a Debian source package under `packaging/debian/`, for Ubuntu 26.04 LTS on `amd64`. They're built locally with `dpkg-buildpackage`, either in a container (`tools/debian-build`), which leaves the `.deb`s, or on the machine (`tools/bootstrap-debian`), which installs them in order. Patched and backported packages take the stock packages' place under the stock names, at versions apt prefers and pins. The carried patches stay one copy for both distributions.

## User stories

1. As an Ubuntu MATE user, I want to install *MATE on Caminito* with one command, so that I can log into it from LightDM.
2. As an Ubuntu MATE user, I want my X11 MATE session to keep working after the install.
3. As an Ubuntu MATE user, I want an `apt upgrade` never to swap a patched package back to Ubuntu's without telling me.
4. As an Ubuntu MATE user, I want to install only the compositor, or only some components, as packages of their own.
5. As an Ubuntu user of VS Code or VSCodium from their `.deb`, I want its menus in the panel with nothing to set up, kept linked as the editor updates.
6. As the developer, I want to build every Ubuntu package from my Arch machine without touching it, and see them pass their tests.
7. As the developer, I want each carried patch written and rebased once, for both distributions.

## Implementation decisions

### Scope

- Ubuntu 26.04 LTS ("resolute"), `amd64`. Debian follows when forky freezes as Debian 14, through the same sources.
- Arch is the reference. `tools/check` and `tools/regenerate-goldens` stay Arch's. The goldens' themes are vendored, so every test runs in the Ubuntu container too.
- Local builds only. No apt repository, no signing, no `sbuild` chroot until there's a public repository.

### Layout and tools

- `packaging/debian/<source package>/debian/`, one directory per Arch package, beside `packaging/arch/`.
- `packaging/patches/<component>/` holds each carried patch series once, with `arch/` and `debian/` subdirectories for the build patches of one distribution ([ADR 0030](../../docs/adr/0030-carried-patches-are-kept-once-for-every-distribution.md)). A build step copies the series into the source package's `debian/patches/` with its `series` file, and the PKGBUILDs list them from there.
- **`tools/debian-build`**: builds every package in a podman container of `ubuntu:26.04`, in dependency order, installing each `.deb` in the container before the next is built. It runs each package's tests (`dh_auto_test`) and leaves the `.deb`s in `packaging/debian/out/`, which git ignores. The host needs podman alone.
- **`tools/bootstrap-debian`**: the same order on the machine itself. It installs the build tools, then for each package `mk-build-deps -i`, `dpkg-buildpackage -b` and `apt install ./<package>.deb`, ending with `caminito-desktop`. Its arguments go to `dpkg-buildpackage`, as `tools/bootstrap`'s go to `makepkg`.

### The packages

| Source package | Kind | Baseline | Its `debian/` starts from |
|---|---|---|---|
| `caminito` | Caminito's | the checkout's committed `HEAD` | new |
| `caminito-vscode` | Caminito's | the checkout's committed `HEAD` | new |
| `caminito-desktop` | meta | none | new |
| `marco` | backported | 1.29.1 | Ubuntu's marco 1.26.2 |
| `mate-panel` | patched | master `58ac6c1` | Ubuntu's 1.27.1 |
| `mate-session-manager` | patched | master `57a9178` | Ubuntu's 1.26.1 |
| `mate-settings-daemon` | patched | 1.29.1 | Ubuntu's 1.26.1 |
| `mate-control-center` | patched | 1.29.1 | Ubuntu's 1.26.1 |
| `caja` | patched | 1.28.0 | Ubuntu's 1.26.4 |
| `mate-utils` | patched | 1.29.1 | Ubuntu's 1.26.1 |
| `mate-power-manager` | patched | 1.28.1 | Ubuntu's 1.26.1 |
| `vala-panel-appmenu` | patched | 25.04 | Ubuntu's 25.04+dfsg-3ubuntu1 |
| `appmenu-gtk-module` | patched | 25.04 | Ubuntu's 25.04-1build1 |

- **Caminito's own packages** are built from `git archive HEAD`, so uncommitted work is never packaged, as on Arch. The version is `0.0~r<commit count>.g<short hash>-1` until `v0.1.0` is tagged, and then the tag's.
  - `caminito` builds the compositor with Cargo through rustup's `stable` (`RUSTUP_TOOLCHAIN=stable`, `cargo fetch --locked` before an offline build), and the helpers and data with Meson at `--prefix=/usr`. It depends, by the stock names at its baselines' versions, on every patched package and on marco ≥ 1.29.1, so no stock version satisfies it. It recommends `appmenu-gtk-module` and `caminito-vscode`.
  - `caminito-desktop` depends on `caminito` and `mate-desktop-environment-extras`, as Arch's depends on `caminito-git` and the `mate` and `mate-extra` groups. It leaves out `ubuntu-mate-desktop`'s branding.
- **Patched and backported packages** ([ADR 0029](../../docs/adr/0029-on-debian-a-patched-package-keeps-the-stock-names-and-is-pinned.md)):
  - keep the source and binary names of Ubuntu's package, with Ubuntu's split into binary packages;
  - are versioned `<baseline>-0caminito<n>`, where `<baseline>` is the upstream version, with `~git<date>.<hash>` for a commit;
  - each install `/etc/apt/preferences.d/caminito-<source package>`, pinning its own binaries at `version /caminito/` with priority 1001;
  - start their `debian/` from Ubuntu's, moved to the baseline, with Ubuntu's own patches dropped where the baseline has them, and Debian's libexec and multiarch paths kept.
- **mate-panel** builds the master library, `libmate-panel-applet-4.so.2`, as `libmate-panel-applet-4-2`, and doesn't build `.so.1`: Ubuntu's `libmate-panel-applet-4-1` stays installed for the applets linked to it. `mate-notification-daemon`'s in-process applet stays broken, as on Arch.
- **marco** is built at 1.29.1 with no patch. mate-control-center 1.29.1 needs `libmarco-private` ≥ 1.29.0 and the `action-scroll-*-titlebar` keys, and the parity table is written against 1.29's schema. X11 MATE on the machine then runs marco 1.29.1.
- **appmenu-gtk-module** installs `/usr/share/caminito/appmenu-gtk-module`, an empty file, and so does Arch's `appmenu-gtk-module-caminito`. The session adds the module to `GTK_MODULES` when that file is there.
- **caminito-vscode** installs the extension to `/usr/share/caminito-vscode/global-menu` and the link script, shared with Arch's package, to `/usr/share/caminito-vscode/`. Its `triggers` file has `interest-noawait` on each editor's built-in extensions directory, and `postinst` runs the script for a trigger, a configure and a removal of an editor. The script's list of directories gains VSCodium's `.deb` path, `/usr/share/codium/resources/app/extensions`.

### Caminito and the patches

- `caminito/src/session.rs` stops reading `/var/lib/pacman/local`. It looks for `/usr/share/caminito/appmenu-gtk-module` instead.
- The link script leaves `packaging/arch/caminito-vscode-git/` for a place both distributions build from, and its tests follow it.
- mate-session-manager's `0003` and `0004` learn the X11-only autostarts of `ubuntu-mate-desktop`'s packages. They're still gated on Caminito.

### Docs

- README: *Building on Ubuntu 26.04*, beside the Arch section. It covers `tools/debian-build` and `tools/bootstrap-debian`, the pins, the backported marco and mate-panel's `.so.1`.
- `docs/testing/checklist.md`: a step that installs the `.deb`s in an Ubuntu 26.04 VM and logs into *MATE on Caminito* from LightDM, run before each milestone closes once this epic is done.
- ADR 0011 and ADR 0022 get a qualifying line each.

## Testing decisions

- Each package runs its own tests in `tools/debian-build`'s container, as each PKGBUILD's `check()` does. The compositor's run on a private session bus, as on Arch.
- The marker file's look-up is a unit test in `session.rs`, beside the `gtk_modules` tests.
- The link script's tests, which run its five directories through a fake root, gain the VSCodium `.deb` path, and keep running from `tools/check`.
- The VM run is the developer's and isn't automated.

## Out of scope

- Debian 13, Ubuntu 24.04, other releases, other architectures.
- An apt repository, signing, PPA or OBS, and `sbuild` or `pbuilder` builds.
- Ubuntu MATE's branding, and `ubuntu-mate-desktop` as a dependency.
- Debian policy's offline build, packaged crates, and a `debian/copyright` fit for the archive.

## Further notes

- Nothing was compiled on Ubuntu when this was written. The baselines' requirements were read from their build files, and the symbols they use from the dependencies' headers. A newer toolchain (glib 2.88, gcc) may add warnings that `-Werror` turns into errors.
- Unconfirmed: whether caja 1.28.0 loads Ubuntu's 1.26 caja extensions (the ABI number matches), whether Ubuntu's 1.27.1 applets work with the patched panel, and whether LightDM's slick-greeter lists the `wayland-sessions` entry.
- When Ubuntu's MATE reaches a baseline, apt still keeps the pinned package. Moving a baseline then is a bump of `<baseline>`, as on Arch.
