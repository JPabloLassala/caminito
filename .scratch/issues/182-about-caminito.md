# caminito-about, and a version string with the build's revision

Epic: about-caminito
Type: implementation
Status: ready-for-agent
Spec: [About Caminito](../about-caminito/spec.md)

## Why

Nothing says which build of Caminito is running. The workspace version is `0.1.0` in every build, no program prints it, and the revision is only in the package's name. The developer wants a small "About Caminito" in MATE's Control Center.

## What to build

### The version string

- **The string is the Cargo version and then the revision in brackets,** `0.1.0 (r672.989d558)`, with the revision in the PKGBUILD's `pkgver` form.
- **A build step finds the revision:** from an environment variable that `packaging/arch/caminito-git/PKGBUILD` sets from `pkgver`, then from `git` in the checkout, and otherwise not at all, when the string is `0.1.0`.
- **The build step is in one place** that both programs below use, so they can't disagree.
- **`caminito --version` prints it** and exits, without starting the compositor.

### caminito-about

A new workspace crate, in GTK4 like caminito-displays. Its layout is variant C of the prototype (see the spec): one window with no second page.

- **On the left,** the `caminito` icon.
- **Beside it,** the name `Caminito` in large bold type with the version string after it on the same line, and under them the description, `A Wayland compositor for the MATE desktop`.
- **Under those, one small dim line:** "Built on Marco and Smithay", the licence, and the repository link.
  - `Marco` and `Smithay` link to their repositories.
  - `GPL-2.0-or-later` links to the GNU page of the licence's text.
  - The repository link is `https://github.com/JPabloLassala/caminito`.
- **The window** is titled "About Caminito" and can't be resized. Escape closes it and it has no Close button. Starting the program a second time raises the window that is open.
- **It is in English only,** as caminito-displays is.

### The Control Center entry

`data/caminito-about.desktop`, installed by Meson to `share/caminito/xdg/applications` beside caminito-displays's entry, so that only a Session lists it:

- `Name=About Caminito`, `Icon=caminito`, `Exec=caminito-about`;
- `Categories=GTK;Settings;HardwareSettings;`, which puts it in Hardware;
- `OnlyShowIn=MATE;`.

The PKGBUILD installs the binary beside `caminito-displays`.

## Acceptance criteria

- [ ] `caminito --version` prints the version string and exits 0 with no compositor started.
- [ ] Built by the PKGBUILD, the revision in the string is the package's `pkgver`. Built by `cargo build` in a checkout, it is that checkout's revision count and short hash. Built with neither, the string is `0.1.0` and the build doesn't fail.
- [ ] `caminito-about` shows the same string as `caminito --version` from the same build.
- [ ] In a Session, the Control Center lists "About Caminito" under Hardware with the `caminito` icon, and it opens the dialog.
- [ ] The entry isn't in `/usr/share/applications`.
- [ ] Escape closes the window, and a second `caminito-about` raises the first's window without opening another.
- [ ] Each of the four links opens in the browser.
- [ ] `caminito-about` is in the workspace's `members`, and passes what [CODING_STANDARDS.md](../../CODING_STANDARDS.md) asks of a crate.
- [ ] [checklist.md](../../docs/testing/checklist.md) has a by-eye entry for the dialog and its Control Center entry.

## For the implementing agent to settle

Recorded in the Done section:

- where the shared build step lives (a small crate both depend on, or another way to have one source);
- the name of the environment variable, and whether a change of revision rebuilds without a `cargo clean`.

## Prototype

`tools/prototype-about-caminito.py` on the branch `prototype/about-caminito` has the three variants that were tried; `--variant=C` shows the one chosen. It is PyGObject and throwaway, and isn't on master: write `caminito-about` afresh, taking only the layout from it.
