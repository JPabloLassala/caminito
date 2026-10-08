# mate-session-manager-caminito

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M5
Blocked by:
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The first Patched package ([16](16-packaging.md), [ADR 0011](../../docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-caminito.md)), with mate-session's two Carried patches ([08](08-session-launch-model.md), [ADR 0004](../../docs/adr/0004-mate-session-as-caminito-child-on-xwayland.md)), and the CI that every patched package then uses.

- **`packaging/arch/mate-session-manager-caminito/PKGBUILD`,** from Arch's own PKGBUILD for mate-session-manager with the smallest diff (16):
  - the source at a pinned master commit (`#commit=`), starting from `57a9178`, which 08 read, or a newer one if it builds;
  - `pkgver` in `git describe`'s form;
  - `provides=(mate-session-manager=$pkgver)` and `conflicts=(mate-session-manager)`, with no `replaces`;
  - the patch files with their `sha256sums`, applied in order in `prepare()`.
- **The patches,** as `git format-patch` files beside the PKGBUILD, both active only when `MATE_WM_NAME=Caminito`:
  - `0001`: `g_unsetenv("GDK_BACKEND")` right after `gtk_init`, so it reaches no child and neither exported environment (08, `main.c:637`, `:651-654`);
  - `0002`: `append_required_apps` skips the `windowmanager` required component, so Marco isn't started and the WindowManager phase ends at once (`main.c:237-297`).
- **CI on push (16):** `packaging.yml` builds each package under `packaging/arch/` in dependency order, installing each into the container as it's built, with `namcap` on the PKGBUILD and on the built package. The patched packages come before `caminito-git`.
- **CI weekly (14, 16):** a job in `weekly.yml` applies each package's patch series to its component's upstream git head and builds it. A failure opens or updates an issue, as the Smithay canary does. It's written here for one package, in a form [70](70-mate-panel-caminito.md) and [71](71-mate-settings-daemon-caminito.md) add a line to.
- **README:** how to rebase a series (16's three steps), once, for all the patched packages.

## Acceptance criteria

- [x] `makepkg -si` installs the package in place of `mate-session-manager`, and `pacman -Q mate-session-manager` is satisfied through `provides`.
- [ ] `namcap` reports nothing unexplained on the PKGBUILD or the package.
- [ ] With `MATE_WM_NAME=Caminito`, nested under [61](61-session-start-up-and-exit.md)'s `caminito --session`: Marco isn't started, the session reaches the Panel phase without waiting 30 s, and a child of mate-session (`/proc/<pid>/environ` of mate-panel) has no `GDK_BACKEND`.
- [x] Without `MATE_WM_NAME`, an X11 MATE login starts Marco as before.
- [x] The Packaging job and the weekly patch-on-head job are green. (Ticked on 2026-10-08: the Packaging job passed on `master` on 2026-10-08 (run 37823877022, at `88710e2`), and the weekly job's `patches-on-head` passed for this package on 2026-10-05, run 37316303593.)

## Notes

- **Not `-git`:** a pinned commit needs no suffix (16).
- **If master has moved to Meson only,** the PKGBUILD follows it; 16 expected Arch's autotools build to keep working, and the diff against Arch's PKGBUILD says what changed.
- **The nested criterion needs 61,** but the package and both CI jobs don't, so this ticket isn't blocked by it. That criterion is ticked when both are in.
- **Resolved on 2026-10-04 with three criteria unticked:** the package, the patches, both CI jobs and the README are on `master`. What is left needs things this ticket doesn't have:
  - **The nested criterion** needs [61](61-session-start-up-and-exit.md)'s `caminito --session`, which isn't in. [72](72-session-file-and-first-login.md) runs it in a real session.
  - **The CI criterion, and namcap on the built package,** need a push: the commit wasn't pushed when this was written. The first run of the Packaging job on x86_64 answers both. The weekly job can be started by hand (`workflow_dispatch`).
- **Baseline:** `57a9178` is still master's head (`v1.28.0-23-g57a9178`), so `pkgver` is `1.28.0.r23.g57a9178`. Master still builds with autotools; there is no `meson.build`.
- **`b2sums`, not `sha256sums`:** Arch's PKGBUILD uses `b2sums`, and the smallest diff keeps them. The git source has a checksum too, as Arch's has; `mate-submodules` stays `SKIP`, as in Arch's.
- **`pkgver()`** is there so the weekly job's build of master's head names what it built. With the pinned commit it gives the same value as the line above it.
- **The weekly job** takes `#commit=` (or `#tag=`) out of the PKGBUILD's source and runs `makepkg --skipchecksums`. 70 and 71 add a line to its matrix and to `PACKAGES` in `packaging.yml`, and their clone directories to `packaging/arch/.gitignore`.
- **Checked in the virtual machine (aarch64, `makepkg -A`):**
  - The package builds and both patches apply with no offset. `pacman -U` replaces `mate-session-manager` 1.28.0-2, and `pacman -Q mate-session-manager` answers `mate-session-manager-caminito 1.28.0.r23.g57a9178-1`. It stays installed there.
  - `namcap PKGBUILD` prints nothing. On the built package, namcap calls every linked library, glibc included, an uninstalled dependency; that is namcap on Arch Linux ARM, not the package. One line is real there only: the guest has libcanberra, so configure links it. A clean container doesn't.
  - The weekly job's steps, run by hand, build master's head.
  - **The patches, under Xvfb on a private bus** as `caminitotest`, with `GDK_BACKEND=x11` and `mate-session --disable-acceleration-check --debug`. With `MATE_WM_NAME=Caminito`: the log has "skipping component 'windowmanager' under Caminito", no marco process starts, the phases go from WINDOW_MANAGER to RUNNING in less than 25 s, and mate-panel's `/proc/<pid>/environ` has no `GDK_BACKEND`. Without `MATE_WM_NAME`: marco starts, and mate-panel has `GDK_BACKEND=x11`, as stock. That second run is what the X11 criterion is ticked on; it wasn't a login through a display manager.
- **`caminito-git` doesn't depend on this package yet.** 16 has it depend on the patched packages by their real names; that waits for [73](73-packaging-and-closing-m5.md), when all three of M5's are in.
- **Review left as is:** the weekly job's report job and the build-user step are copies of the ones beside them.
