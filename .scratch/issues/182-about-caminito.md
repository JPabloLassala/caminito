# caminito-about, and a version string with the build's revision

Epic: about-caminito
Type: implementation
Status: open: the code is in, and the checklist item, which needs a Session and a package built by the PKGBUILD, is the developer's
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

- [x] `caminito --version` prints the version string and exits 0 with no compositor started.
- [ ] Built by the PKGBUILD, the revision in the string is the package's `pkgver`. Built by `cargo build` in a checkout, it is that checkout's revision count and short hash. Built with neither, the string is `0.1.0` and the build doesn't fail.
- [x] `caminito-about` shows the same string as `caminito --version` from the same build.
- [ ] In a Session, the Control Center lists "About Caminito" under Hardware with the `caminito` icon, and it opens the dialog.
- [ ] The entry isn't in `/usr/share/applications`.
- [x] Escape closes the window, and a second `caminito-about` raises the first's window without opening another.
- [ ] Each of the four links opens in the browser.
- [x] `caminito-about` is in the workspace's `members`, and passes what [CODING_STANDARDS.md](../../CODING_STANDARDS.md) asks of a crate.
- [x] [checklist.md](../../docs/testing/checklist.md) has a by-eye entry for the dialog and its Control Center entry.

## For the implementing agent to settle

Recorded in the Done section:

- where the shared build step lives (a small crate both depend on, or another way to have one source);
- the name of the environment variable, and whether a change of revision rebuilds without a `cargo clean`.

## Prototype

`tools/prototype-about-caminito.py` on the branch `prototype/about-caminito` has the three variants that were tried; `--variant=C` shows the one chosen. It is PyGObject and throwaway, and isn't on master: write `caminito-about` afresh, taking only the layout from it.

## Done (2026-10-11)

- **The build step is a crate, `caminito-version`.** It has the constant `VERSION` and a build script, and `caminito` and `caminito-about` both depend on it, so there is one string in a build. The other way, one script that two `build.rs` files include, would run twice and could give two answers.
- **The variable is `CAMINITO_REVISION`,** which the PKGBUILD exports from `pkgver` in `build()` and again in `check()`, since a different value there would build the crate and what depends on it a second time. An empty variable counts as unset.
- **A change of revision rebuilds without a `cargo clean`.** The script reruns when the variable changes and when the checkout's `HEAD` or its reflog changes, which covers a commit, a reset, a pull and a checkout of another branch. Seen here: the commit of this work moved the string from `r674.f2cff1f` to `r675.ff911ed` on the next `cargo build`, and setting and unsetting the variable changed it each time.
- **The checkout's revision is always `r<count>.<hash>`.** `pkgver()` prefers `git describe` once the repository has a tag, which it doesn't. With a tag, only a packaged build would have `pkgver`'s form, through the variable.
- **Only Caminito's own checkout gives a revision.** The script asks git whether its own `build.rs` is tracked, so a tarball unpacked below some other repository has `0.1.0`. Seen with the crate copied into an unrelated repository, and outside any.
- **`caminito --version` prints the string alone,** `0.1.0 (r675.ff911ed)`, with no program name before it, as the ticket words it. It returns before logging or a backend is touched. `caminito/tests/version.rs` runs the binary with an empty environment and was written first.
- **The small dim text is two lines, not one,** as the prototype's variant C has it: "Built on Marco and Smithay. GPL-2.0-or-later." and the repository under it. This ticket says "one small dim line" and the prototype the developer chose from has the break. The prototype won; it's one `\n` in `small_markup` to change.
- **The title can be selected,** as in the prototype, so the version can be copied. GTK selects all of a selectable label that gets a new window's focus, so the window starts with the focus on nothing.
- **The licence links to `https://www.gnu.org/licenses/old-licenses/gpl-2.0.html`.**
- **`gtk4` is in `[workspace.dependencies]` now,** since two crates use it. caminito-displays keeps its `v4_12` feature and caminito-about asks for none.
- **caminito-about has no tests.** It is a window over three constants. It was run under Xvfb on GDK's X11 backend: the layout was compared by eye with a screenshot, a synthetic Escape closed it, and a second start exited 0 and left one window.
- **Not run: anything in a Session.** The Control Center's entry under Hardware, the four links in a browser, the raise of the open window under Caminito, and the PKGBUILD's build are the checklist's new item, "About Caminito is in the control center, under Caminito only, and shows the installed build". The entry passes `desktop-file-validate`, and `meson install` puts it in `share/caminito/xdg/applications`.
- **`tools/check` passes but for four `tests::keymap` tests,** which fail the same way on master on this machine: they read `name[Group…]` lines from the keymap's text and find none. They aren't this ticket's and have no ticket yet.
