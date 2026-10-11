# The dialog captures a mouse button

Epic: system-shortcuts
Type: implementation
Status: ready-for-agent
Blocked by: 183, 184, 185
Spec: [System shortcuts](../system-shortcuts/spec.md)

## Why

A button System shortcut works once it's in a setting ([184](184-caminito-binds-mouse-buttons.md), [185](185-mouse-buttons-as-accelerators.md)), and only `dconf` can put it there. The developer wants *System Shortcuts* to detect a button press as it detects a key.

## What to build

A carried patch in `packaging/arch/mate-control-center-caminito`, gated on Caminito (ADR 0011). The capture is in `capplets/keybindings/eggcellrendererkeys.c` and the rest in `mate-keybinding-properties.c`.

### Capture

- **While a row is capturing, a button press becomes the shortcut,** with the modifiers held. The dialog holds its shortcuts inhibitor then, which under 184 brings it the presses Caminito would otherwise take.
- **Which buttons:** the middle button, back, forward and further side buttons, alone or with modifiers; the left and right buttons with a modifier only. Not the wheel.
- **A bare left click** outside the row cancels, as now. Escape cancels and Backspace clears.
- **Any row takes a button:** Caminito's window and workspace actions, the media keys and a custom shortcut.

### What's written and shown

- **The value written** is [ADR 0028](../../docs/adr/0028-a-mouse-button-is-written-into-marcos-keybinding-keys.md)'s: `<Super>Button8`.
- **The row shows** *Middle Button*, *Back Button* and *Forward Button* for 2, 8 and 9, *Left Button* and *Right Button* for 1 and 3, and *Button 10* and up by number, joined to the modifiers as a key is: *Super+Back Button*. They're the labels the Window menu shows (184).
- **Spanish** has the labels and the two messages below. Other locales show the English.
- **A value already in a setting** is read and shown the same way.

### Warnings

- **A bare middle button** asks for confirmation before it's set, as a bare letter does: it would end middle-click paste and the titlebar's middle-click action.
- **A combination that `mouse-button-modifier` uses** (its modifiers with the left, middle or right button) warns that over a window the click moves, resizes or opens the window's menu instead. The user can set it anyway.
- **A button already in use** by another row gets the dialog's *already used for* question, as a key does.

### Under Marco

- **The dialog captures keys only,** as stock, and a row whose value names a button shows as disabled. Its value is left as it is until the user sets the row.

## Acceptance criteria

- [ ] The package builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item, run in a Session with a mouse that has side buttons:
  - capturing Super+back on a window action shows *Super+Back Button*, `dconf` has `<Super>Button8`, and the action runs at once on that press over a window and over the panel;
  - capturing the forward button alone on a custom shortcut runs its command;
  - a bare middle button asks first, and cancelling leaves the row as it was;
  - Alt+middle warns of the modifier click;
  - a bare left click outside the row cancels the capture, and Backspace clears the row;
  - a button a row already has asks to reassign;
  - closing and opening the dialog shows the same labels.
- [ ] In the X11 session with Marco: a button press captures nothing, a row left with a button value shows as disabled, and a key is captured as before (the gate).
- [ ] 184's and 185's checklist items no longer set the shortcut with `dconf` alone: they say to capture it in *System Shortcuts*.

## For the implementing agent to settle

Recorded in the Done section:

- how the cell renderer holds a button, since it keeps a keyval, a mask and a keycode for a row;
- where the Spanish strings are carried, with [183](183-the-dialog-is-system-shortcuts.md)'s answer.
