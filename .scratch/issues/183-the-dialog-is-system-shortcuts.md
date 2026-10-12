# The dialog is named System Shortcuts under Caminito

Epic: system-shortcuts
Type: implementation
Status: open: the code is in and the package builds, and the two Session criteria, the checklist's *Windows and System Shortcuts open under Caminito, and name it*, are the developer's
Blocked by: none
Spec: [System shortcuts](../system-shortcuts/spec.md)

## Why

What the control centre's *Keyboard Shortcuts* lists is, on Caminito, a System shortcut ([CONTEXT.md](../../CONTEXT.md)), and one will soon be a mouse button too ([184](184-caminito-binds-mouse-buttons.md)). The developer wants the dialog named for it.

## What to build

- **A carried patch** in `packaging/arch/mate-control-center-caminito`, the next in its series, gated on Caminito through patch 0001's helper in `capplets/common/wm-common.c` (ADR 0011).
- **Under Caminito the dialog's window is titled *System Shortcuts*.** The title is set in `capplets/keybindings/mate-keybinding-properties.ui`.
- **Under Caminito its launcher is named *System Shortcuts*** in the control centre's shell and in the menus. The name is `capplets/keybindings/mate-keybinding.desktop.in`'s `Name`, which a running program can't gate, so settle how below.
- **Spanish** has *Atajos del sistema* for both. Other locales show the English.
- **Under Marco on X11** both stay *Keyboard Shortcuts*, in every locale, as stock.
- **The docs** say *System Shortcuts* where they name the dialog under Caminito: `README.md`, `docs/parity.md` and `docs/testing/checklist.md`. A checklist step run in the X11 session keeps *Keyboard Shortcuts*. Resolved tickets aren't edited.
- **`pkgrel`** is bumped, and `caminito-git` asks for the new one if it names a version.

## Acceptance criteria

- [x] The package builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [ ] In a Session, the control centre lists *System Shortcuts*, and the dialog it opens has that title. Under `LANG=es_ES.UTF-8` both read *Atajos del sistema*.
- [ ] In the X11 session with Marco, both read *Keyboard Shortcuts*, and in Spanish what stock shows.
- [x] No doc names the dialog *Keyboard Shortcuts* for a step run under Caminito.
- [x] The checklist's *Windows and Keyboard Shortcuts open under Caminito, and name it* is renamed and checks the title and the launcher.

## For the implementing agent to settle

Recorded in the Done section:

- how the launcher's name is gated: a second entry in `share/caminito/xdg/applications`, the directory only a Session has on its data path, that shadows the stock one by its file name, or another way;
- where the Spanish strings are carried, since the package's `.po` files are upstream's.

## Done (2026-10-12)

- **The launcher's name is gated by a second entry,** `data/mate-keybinding.desktop`, which Meson installs to `share/caminito/xdg/applications` under the stock entry's id, as caminito-displays's is (84). Only a Session has that directory on its data path, so the control centre and the menus show one launcher there, *System Shortcuts*, and X11 MATE and the Wayfire session still find the stock one.
- **The Spanish is carried in that entry,** as `Name[es]=Atajos del sistema`, and nowhere else. Patch 0007, gated on patch 0001's `wm_common_is_caminito`, sets the dialog's title to its launcher's localised name with `GDesktopAppInfo`, so the title and the launcher can't differ and no `.po` of upstream's is touched. The `.ui` file keeps its stock title. The developer chose this over strings in the patch and over a patch to `es.po`.
- **The gate reads `MATE_WM_NAME` alone,** so it holds on XWayland too, as 0006's does. If the variable leaked into another session the title would be that session's launcher's name, which is the stock one, and in Spanish the stock launcher and the stock title are the same string, *Atajos de teclado*.
- **The entry keeps the stock comment and keywords in every language,** copied from mate-control-center 1.29.1's built entry. A shadowing entry replaces the whole file, so without them every language but Spanish would have lost its tooltip and search terms. Only `Name` has English and Spanish alone. A baseline move should copy them again.
- **The comment still says keys:** "Assign shortcut keys to commands", and its Spanish. The ticket names the title and the launcher's name, and 186 is where a mouse button can be captured.
- **Tested first, at two seams the developer agreed:** `caminito/src/tests/system_shortcuts_entry.rs` for the entry's names and what it launches, red on the missing file, and a step in `tools/check` that installs to a `DESTDIR` and fails unless the entry is in `share/caminito/xdg/applications` and nowhere else, red before the Meson rule. That step needs `desktop-file-utils`, added to `--deps`.
- **The patch has no test.** `makepkg` built 1.29.1-5 with 0007 in the series, and `namcap` gave what it gave before: the `$startdir` reference of the `file://` patches and three dependency warnings.
- **`caminito-git` names no version of `mate-control-center-caminito`,** so nothing changed there but a comment. A new `caminito-git` beside 1.29.1-4 shows the launcher as *System Shortcuts* and the title as *Keyboard Shortcuts*; the checklist's step 1 asks for 1.29.1-5.
- **`docs/parity.md` never named the dialog,** so it's unchanged. The README and eight Caminito steps of the checklist take the name, and the checklist's steps in the X11 and Wayfire sessions keep *Keyboard Shortcuts*. Code comments that name the stock dialog keep it, and so does open ticket 142's criterion.
- **Not run: anything in a Session.** The title and the launcher in English and Spanish, under Caminito and under Marco on X11, are the renamed checklist item. An attempt to read the title from the built dialog under Xvfb showed no window within 25 s and was dropped. This machine has no `es_ES.UTF-8` locale generated.
- **`tools/check` passed but for one step, which is this machine's:** *caminito-locker against caminito* fails without `pam_wrapper`, which isn't installed here. The step after it, the extension, was run by hand and passed. Four keymap tests and one tiling test failed on the branch's first base and passed once master's eff60ea was merged in.
