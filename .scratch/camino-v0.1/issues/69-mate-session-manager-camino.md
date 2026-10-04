# mate-session-manager-camino

Type: implementation
Status: open
Milestone: M5
Blocked by:
Map: [Camino v0.1](../map.md)

## What to build

The first Patched package ([16](16-packaging.md), [ADR 0011](../../../docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-camino.md)), with mate-session's two Carried patches ([08](08-session-launch-model.md), [ADR 0004](../../../docs/adr/0004-mate-session-as-camino-child-on-xwayland.md)), and the CI that every patched package then uses.

- **`packaging/arch/mate-session-manager-camino/PKGBUILD`,** from Arch's own PKGBUILD for mate-session-manager with the smallest diff (16):
  - the source at a pinned master commit (`#commit=`), starting from `57a9178`, which 08 read, or a newer one if it builds;
  - `pkgver` in `git describe`'s form;
  - `provides=(mate-session-manager=$pkgver)` and `conflicts=(mate-session-manager)`, with no `replaces`;
  - the patch files with their `sha256sums`, applied in order in `prepare()`.
- **The patches,** as `git format-patch` files beside the PKGBUILD, both active only when `MATE_WM_NAME=Camino`:
  - `0001`: `g_unsetenv("GDK_BACKEND")` right after `gtk_init`, so it reaches no child and neither exported environment (08, `main.c:637`, `:651-654`);
  - `0002`: `append_required_apps` skips the `windowmanager` required component, so Marco isn't started and the WindowManager phase ends at once (`main.c:237-297`).
- **CI on push (16):** `packaging.yml` builds each package under `packaging/arch/` in dependency order, installing each into the container as it's built, with `namcap` on the PKGBUILD and on the built package. The patched packages come before `camino-git`.
- **CI weekly (14, 16):** a job in `weekly.yml` applies each package's patch series to its component's upstream git head and builds it. A failure opens or updates an issue, as the Smithay canary does. It's written here for one package, in a form [70](70-mate-panel-camino.md) and [71](71-mate-settings-daemon-camino.md) add a line to.
- **README:** how to rebase a series (16's three steps), once, for all the patched packages.

## Acceptance criteria

- [ ] `makepkg -si` installs the package in place of `mate-session-manager`, and `pacman -Q mate-session-manager` is satisfied through `provides`.
- [ ] `namcap` reports nothing unexplained on the PKGBUILD or the package.
- [ ] With `MATE_WM_NAME=Camino`, nested under [61](61-session-start-up-and-exit.md)'s `camino --session`: Marco isn't started, the session reaches the Panel phase without waiting 30 s, and a child of mate-session (`/proc/<pid>/environ` of mate-panel) has no `GDK_BACKEND`.
- [ ] Without `MATE_WM_NAME`, an X11 MATE login starts Marco as before.
- [ ] The Packaging job and the weekly patch-on-head job are green.

## Notes

- **Not `-git`:** a pinned commit needs no suffix (16).
- **If master has moved to Meson only,** the PKGBUILD follows it; 16 expected Arch's autotools build to keep working, and the diff against Arch's PKGBUILD says what changed.
- **The nested criterion needs 61,** but the package and both CI jobs don't, so this ticket isn't blocked by it. That criterion is ticked when both are in.
