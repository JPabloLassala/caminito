# A right click on an output that isn't the primary one pops up no desktop menu

Epic: caminito-v0.1
Type: implementation
Status: open: the patch and the package are in, and the checklist item, which needs a Session with two monitors and `caja-caminito` at `pkgrel` 4 installed, is the developer's
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

In a Session with two monitors, a right click on the desktop of the output that isn't the primary one does nothing: no context menu. On the primary output it pops up caja's desktop menu. The developer saw it on 2026-10-12, with HDMI-A-1, the LG, as the primary output and DP-1, the AOC, to its right, on `caja-caminito 1.28.0-3`.

Under X11, MATE has the menu on every monitor, because caja's desktop is one window that spans them all.

## Acceptance criteria

- [x] The cause is written down in this ticket's notes, with a check that fails before the fix.
- [x] With two outputs, a right click on the one that isn't the primary output pops up the desktop's menu, at the pointer. (Seen with the real caja on the headless Caminito: see the notes.)
- [x] The wallpaper on that output looks as it did.
- [x] The menu follows the primary output and the monitors: it is on every output but the one with the icons, after the primary output changes and after either monitor is unplugged and plugged back.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item for it.
- [ ] In a Session with two monitors, the checklist item passes.

## Notes

- **The cause is the design of `caja-caminito`'s patch (81), not a regression.** caja's desktop on Wayland is one layer surface, and a layer surface is on one output. The patch puts it on the primary output. Every other output has msd's `mate-background` alone ([71](71-mate-settings-daemon-caminito.md)), which takes the click and has no menu. [76](76-tty-login-left-over.md) passed "the wallpaper alone on DP-1" by eye.
- **Seen in the developer's Session (2026-10-12)**, in each output's `layer_surfaces` in `DumpState`: HDMI-A-1 had `mate-background`, two `panel` and `desktop`, and DP-1 had `mate-background` only.
- **The developer's decision (2026-10-12): the menu only.** The icons stay on the primary output. A desktop with icons on every output would show the same folder on each, and is a much larger patch to carry.
- **The fix is caja's second carried patch**, `0002-desktop-window-Pop-up-the-desktop-s-menu-on-Caminito.patch`, gated on an owner of `org.mate.Caminito` as the first is (ADR 0011). Nothing changes in Caminito.
  - On each output but the primary one, caja keeps a layer surface in `bottom`, of the namespace `desktop-menu`, anchored to the four edges, with nothing drawn in it and no keyboard interactivity. A right click on it pops up the desktop view's background menu.
  - The menu is popped up with no event of its own. GTK then places it by the current event, which is the click on the `desktop-menu` surface, so the popup is that surface's. And caja records no position for what the menu makes, so a new folder or launcher goes where the desktop has room, on the primary output, and not at the other output's coordinates.
  - The surfaces are synced where the desktop follows the primary output: on `PrimaryOutput`'s change, on Caminito's name coming or going, and on `monitors-changed` (89).
  - The compositor closes the layer surface of an output that goes, and gtk-layer-shell then closes its window. caja keeps a reference to each window and to its monitor, so neither is freed under it.
- **Not built:** drops on the other outputs, and icons there.

## The check

[208-checks/repro208.rs](208-checks/repro208.rs) runs the real caja against Caminito on the headless backend, with two outputs and the first as the primary output. Its header says how to run it. It isn't in the suite, since it needs caja.

- **Before the fix**, with the installed `/usr/bin/caja` (`caja-caminito 1.28.0-3`): `headless-1` had `desktop`, `headless-2` had nothing, the right click asked for no popup, and the pixel under the pointer stayed the bare output's. Verdict `FAIL`.
- **After**, with `CAJA=` the caja built from the patched tree: `headless-2` had `desktop-menu`. Its pixel was the bare output's with caja running (`ff333333` both times), so the surface draws nothing. The right click at the output's middle asked for one popup of the layer surface, with the anchor at 1280,720, and the pixel beside the pointer became the menu's (`ffeaeaea`). Verdict `PASS`.
- **Following:** with the second output chosen, `desktop` was on `headless-2` and `desktop-menu` on `headless-1`. Each output unplugged in turn left `desktop` alone on the other, and plugged back had `desktop-menu` again, or `desktop` when it was the one chosen. caja was still running at the end, with no GLib warning in its log.
