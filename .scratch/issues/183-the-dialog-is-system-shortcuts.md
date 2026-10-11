# The dialog is named System Shortcuts under Caminito

Epic: system-shortcuts
Type: implementation
Status: ready-for-agent
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

- [ ] The package builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [ ] In a Session, the control centre lists *System Shortcuts*, and the dialog it opens has that title. Under `LANG=es_ES.UTF-8` both read *Atajos del sistema*.
- [ ] In the X11 session with Marco, both read *Keyboard Shortcuts*, and in Spanish what stock shows.
- [ ] No doc names the dialog *Keyboard Shortcuts* for a step run under Caminito.
- [ ] The checklist's *Windows and Keyboard Shortcuts open under Caminito, and name it* is renamed and checks the title and the launcher.

## For the implementing agent to settle

Recorded in the Done section:

- how the launcher's name is gated: a second entry in `share/caminito/xdg/applications`, the directory only a Session has on its data path, that shadows the stock one by its file name, or another way;
- where the Spanish strings are carried, since the package's `.po` files are upstream's.
