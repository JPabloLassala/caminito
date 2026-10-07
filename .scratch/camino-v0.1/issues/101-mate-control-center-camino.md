# mate-control-center-camino: Windows and Keyboard Shortcuts

Type: implementation
Status: open
Milestone: M6
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

The control center's Windows and Keyboard Shortcuts dialogs open under Camino, name it, and change its settings ([06](06-v01-protocol-and-integration-set.md)'s *Camino's identity* row, [05](05-v01-marco-feature-cut.md), [16](16-packaging.md), [17](17-milestone-slicing.md)).

- **The package:** `mate-control-center-camino`, a Patched package as 16 describes them: Arch's PKGBUILD at 16's baseline, `provides` and `conflicts` on the stock one, its `format-patch` series beside it, its `makepkg` and `namcap` CI job, and its place in the weekly patch-on-head job.
- **The carried patches,** gated as ADR 0011 says:
  - `wm-common` reads `MATE_WM_NAME` and `MATE_WM_KEYBINDINGS` on a display that isn't X11. Camino exports both already ([61](61-session-start-up-and-exit.md));
  - the Windows capplet uses `wm-common`, accepts "Camino", and connects its X11-only signal only on X11;
  - Keyboard Shortcuts no longer dereferences a NULL keybinding list on Wayland.
- **The manifests:** Camino's `50-camino-*.xml` keybinding manifests with `wm_name="Camino"`, pointing at the `org.mate.Marco.*` keys, installed by the top-level Meson project into the control center's keybindings directory. Start from Marco's own manifests, and leave out the actions 05 put in *never*.
- **`camino-git`** depends on the package by its real name, as it does on the other Patched packages.
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [ ] The package builds, and its `makepkg` and `namcap` job passes in CI.
- [ ] The manifests are installed by Meson, and `camino-git` builds and passes `namcap` with them.
- [ ] The checklist has the item, and it was run in a Session: Windows opens and a change there (the titlebar's double-click action, say) applies live; Keyboard Shortcuts opens without a crash and lists Camino's window management actions. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).
- [ ] On X11 MATE both dialogs work as before, with Marco named (the gate).

## Notes

- **A shortcut changed in the dialog changes no binding until M8,** when Camino reads `org.mate.Marco.*` keybindings in place of its hard-coded set (17). The dialog writes the key; say so in the checklist item.
- **No 5% scale steps patch.** 17 had one for the Display capplet. camino-displays replaced that capplet ([84](84-camino-displays.md), ADR 0014), so the patch has nothing to change.
- **The Mouse and Keyboard capplets** are [102](102-mouse-and-keyboard-capplets.md)'s.
