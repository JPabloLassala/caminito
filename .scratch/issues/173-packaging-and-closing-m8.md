# Packaging and closing M8

Epic: caminito-v0.1
Type: implementation
Status: open
Milestone: M8
Blocked by: 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

M8's exit criterion holds on the developer's machine, and the closing commit says so ([17](17-milestone-slicing.md)): every `org.mate.Marco.*` key and all 103 actions are honoured, never implemented, or on the roadmap.

- **Packaging ([16](16-packaging.md)):** `caminito-git` depends on zenity ([167](167-zenity-dialogs.md)) and on whatever plays sound ([170](170-audible-bell-and-switch-sounds.md)). Any Carried patch M8's tickets wrote (mate-panel's, for urgency or above) is in its Patched package. `pkgver` and every `pkgrel` are bumped, and all the packages are installed on the developer's machine from the built files.
- **The keybinding manifests:** the `50-caminito-*.xml` files list every action Caminito now binds, checked by their test against every action, so that mate-control-center's *Keyboard Shortcuts* shows them all.
- **The README** says what Marco parity means for v0.1 and points at `docs/parity.md`.
- **The parity table:** no row is *to do*, and the *unsupported* rows are exactly 05's *later* list and 09's roadmap, with any other moved there only with a reason beside it.
- **The closing run:** every item tagged M8, and every earlier one still open, on the installed packages, in a Session on the developer's machine, with a wizard script beside this ticket as [73](73-packaging-and-closing-m5.md) had. It includes [108](108-window-switcher.md)'s and [144](144-drag-to-unmaximize.md)'s Session checks, [101](101-mate-control-center-caminito.md)'s item with a changed shortcut now applying at once, and the pass over the parity table's rows that no test covers.
- **The closing commit,** `Close M8 Marco parity`, with the results in its message. It moves the tag of any item that's deferred, and updates the README's status and the map.

## Acceptance criteria

- [ ] `tools/check` passes on the developer's machine.
- [ ] `docs/parity.md` has no row left to do, and its unsupported rows are exactly 05's and 09's roadmap; its test passes.
- [ ] The manifests' test checks every action Caminito binds.
- [ ] 17's exit, run by hand in a Session: the parity table's rows that no automated test covers, each seen.
- [ ] X11 MATE and MATE on Wayfire still work with all the Patched packages installed (ADR 0011).
- [ ] Every checklist item tagged M8, and every earlier one still open, was run, and the results are in the closing commit's message.

## Notes

- **M8's tickets don't wait on [136](136-packaging-and-closing-m7.md),** M7's closing run, as 108 and 144 didn't. M7's items still open when this runs are part of it.

## Open questions

**For the developer:**

- **Patches mate-panel may need,** for urgency in the window list ([166](166-focus-stealing-prevention-for-xwayland-windows.md)) or for *Always on Top* in its menu ([155](155-above-below-and-sticky.md)): whether they're carried in `mate-panel-caminito`, or left as known gaps in the parity table.
