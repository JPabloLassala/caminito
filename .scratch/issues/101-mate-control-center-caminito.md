# mate-control-center-caminito: Windows and Keyboard Shortcuts

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M6
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The control center's Windows and Keyboard Shortcuts dialogs open under Caminito, name it, and change its settings ([06](06-v01-protocol-and-integration-set.md)'s *Caminito's identity* row, [05](05-v01-marco-feature-cut.md), [16](16-packaging.md), [17](17-milestone-slicing.md)).

- **The package:** `mate-control-center-caminito`, a Patched package as 16 describes them: Arch's PKGBUILD at 16's baseline, `provides` and `conflicts` on the stock one, its `format-patch` series beside it, its `makepkg` and `namcap` CI job, and its place in the weekly patch-on-head job.
- **The carried patches,** gated as ADR 0011 says:
  - `wm-common` reads `MATE_WM_NAME` and `MATE_WM_KEYBINDINGS` on a display that isn't X11. Caminito exports both already ([61](61-session-start-up-and-exit.md));
  - the Windows capplet uses `wm-common`, accepts "Caminito", and connects its X11-only signal only on X11;
  - Keyboard Shortcuts no longer dereferences a NULL keybinding list on Wayland.
- **The manifests:** Caminito's `50-caminito-*.xml` keybinding manifests with `wm_name="Caminito"`, pointing at the `org.mate.Marco.*` keys, installed by the top-level Meson project into the control center's keybindings directory. Start from Marco's own manifests, and leave out the actions 05 put in *never*.
- **`caminito-git`** depends on the package by its real name, as it does on the other Patched packages.
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [x] The package builds, and its `makepkg` and `namcap` job passes in CI. (It builds with `makepkg` on the developer's machine, and `namcap`'s report on the PKGBUILD and on the package is in the resolution. The CI job has the package, and hasn't run: its first run is the push's.) (Ticked on 2026-10-08: the Packaging job passed on `master` on 2026-10-08 (run 37823877022, at `88710e2`), where `mate-control-center-caminito` 1.29.1-3 was built and `namcap` had no error.)
- [x] The manifests are installed by Meson, and `caminito-git` builds and passes `namcap` with them. (A staged `meson install` puts the three in `share/mate-control-center/keybindings`, and `namcap` has nothing to say of the PKGBUILD. `caminito-git` itself wasn't rebuilt here: it builds the committed `HEAD`, so its build is CI's, and 106's.) (Ticked on 2026-10-08: the Packaging job passed on `master` on 2026-10-08 (run 37823877022, at `88710e2`), where `caminito-git` r346.88710e2-1 was built and `namcap` had no error.)
- [ ] The checklist has the item (*Windows and Keyboard Shortcuts open under Caminito, and name it*). It wasn't run in a Session, which is left to [106](106-packaging-and-closing-m6.md); what was run instead is in the resolution. The item: Windows opens and a change there (the titlebar's double-click action, say) applies live; Keyboard Shortcuts opens without a crash and lists Caminito's window management actions. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).
- [x] On X11 MATE both dialogs work as before, with Marco named (the gate). (Seen on Xvfb with Marco, with Caminito's two variables set to show they aren't read there. The X11 session itself is in the checklist item, and the developer saw both dialogs work in it on 2026-10-08.)

## Notes

- **A shortcut changed in the dialog changes no binding until M8,** when Caminito reads `org.mate.Marco.*` keybindings in place of its hard-coded set (17). The dialog writes the key; say so in the checklist item.
- **No 5% scale steps patch.** 17 had one for the Display capplet. caminito-displays replaced that capplet ([84](84-caminito-displays.md), ADR 0014), so the patch has nothing to change.
- **The Mouse and Keyboard capplets** are [102](102-mouse-and-keyboard-capplets.md)'s.

## Resolution

The package is `packaging/arch/mate-control-center-caminito`, on the 1.29.1 tag, with three patches. Arch's own PKGBUILD is at 1.28.2 now and cherry-picks one upstream commit, which the tag has, so the cherry-pick is the one thing of Arch's that's left out. The manifests are `data/keybindings/50-caminito-*.xml`, and `caminito/src/tests/keybinding_manifests.rs` checks them against the vendored Marco schema.

Decided here, on 2026-10-06:

- **`wm-common` reads the variables only where `MATE_WM_NAME` is `Caminito`** (`0001`), which is ADR 0011's gate to the letter. There it names Caminito, and the keybindings in `MATE_WM_KEYBINDINGS`, or Caminito's name without one. Under any other Wayland compositor, whatever the variables say, it answers what it did: an unknown window manager and no keybindings. On X11 they're never read, so a variable left over in an X11 session changes nothing.
- **The Windows capplet has a third X11-only thing** besides the name and the signal (`0002`): the modifiers it offers for moving a window came from the X server's modifier map. Off X11 it offers Alt, and Super when the keymap has the key. Meta and Hyper aren't offered there, since a Wayland keymap has them on Alt's and Super's modifiers.
- **The Keyboard Shortcuts crash wasn't only the NULL list** (`0003`). 06 found the list; the capplet also parsed and showed every binding through the X server's keymap and modifier map, on a display that has neither, and that crashed first in the run where it was looked at (in libX11's `_XkbReloadDpy`). Off X11 the keycode comes from GDK's keymap, the virtual modifiers are a fixed table (Mod1 Alt and Meta, Mod2 Num Lock, Mod4 Super and Hyper, Mod5 Mode_switch, which is what xkeyboard-config's keymaps have), and GDK's event says whether a key is a modifier.
- **`0003` has no gate, and is an upstream fix in ADR 0011's sense.** It's right on any compositor and changes nothing on X11. It does change the Wayfire session: the stock capplet crashes there, and this one opens and lists the shortcuts that aren't a window manager's. A gate would only keep the crash. Like `mate-settings-daemon-caminito`'s `0006`, this is the developer's to confirm.
- **The manifests leave nothing out.** They are marco 1.29.0's three with `wm_name="Caminito"`. None of 05's *never* actions (`run-command-13` to `32`, `set-spew-mark`) is in Marco's manifests, or in its schema. They keep `package="marco"`, so the names are translated from marco's catalogue, which `caminito-git` depends on.
- **Under Caminito each action is listed once:** Marco's own manifests are skipped there, as Caminito's are under Marco.

How it was tested, with mate-control-center 1.29.1 built with the patches, on a private bus and a scratch dconf, and a GTK module that prints what each dialog shows:

- **On a headless Wayfire, with `MATE_WM_NAME=Caminito` and `MATE_WM_KEYBINDINGS=Caminito`:** Windows opens, with Alt and Super. Keyboard Shortcuts opens and lists 93 actions in four sections, the same rows as on X11 with Marco, with Caminito's from the staged manifests.
- **There without the variables (the gate):** Windows says the window manager is unsupported, and with `MATE_WM_NAME=Metacity` too. Keyboard Shortcuts opens and lists 28 actions, none of a window manager.
- **On Xvfb with Marco, with the variables set:** both dialogs are as the stock 1.28.1 ones are there, with Marco's actions listed. With no window manager, Windows says so.

The package was built with `makepkg` on the developer's machine, with the final series. `namcap` has nothing to say of the PKGBUILD, and of the package only three warnings, each of a dependency that "may not be needed" and that Arch's own PKGBUILD has: `gnome-themes-extra`, `mate-panel` and `mate-settings-daemon`. The one dependency changed from Arch's is `libayatana-appindicator` for `libappindicator`: configure takes it first where both are installed, and the patched msd brings it. The full test suite passes.

Not tested: a shortcut edited in the dialog on Wayland, which needs a key press, and a setting applying live in Caminito. Both are in the checklist item, for the Session.

