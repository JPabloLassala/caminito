# The dialog captures a mouse button

Epic: system-shortcuts
Type: implementation
Status: open: the code is in, and `makepkg` and `namcap` on Arch, and the checklist item's Session and X11 runs, are the developer's
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

## Done (2026-10-12)

- **Tests first, at one seam:** `186-checks/buttons.c`, run by `186-checks/run.sh`, which clones mate-control-center's 1.29.1 tag, applies the whole series with `patch -Np1` as `prepare()` does, and checks what 0008 adds to `eggaccelerators.c`: the parser against the cases of Caminito's parser tests (184), the written names, the labels in English and Spanish, and which presses are a shortcut. It failed to build before the functions were in, and passes.
- **The cell renderer holds a button beside its keyval, keycode and mask,** as a `button` field and property that's 0 for a key, and a row's `KeyEntry` has one too. A captured button comes out by a new `button_edited` signal (path, button, modifiers), so `accel_edited` keeps its signature, and both end in one `binding_edited`, where the dialog's checks are. A row is still one string (ADR 0028).
- **The Spanish strings are carried in the patch,** behind `egg_caminito_text`, which picks the Spanish where the first of `g_get_language_names` is Spanish. That's against 183's answer, where the developer chose a desktop entry Caminito installs over strings in a patch and over a patch to `es.po`. An entry can carry a launcher's name but not a label or a message, so of the two left, strings in the patch keep upstream's catalogue untouched, as 183 did. Patching `es.po` instead is the developer's call. The labels are *Botón izquierdo*, *Botón central*, *Botón derecho*, *Botón atrás*, *Botón adelante* and *Botón N*. The dialogs' *OK* and *Cancel* are GTK's, so no third string was needed.
- **A button's modifiers are named as Caminito reads them,** so the value is `<Super>Button8` and the label *Super+Back Button*, as the ticket and ADR 0028 have them. A key's are named as stock names them, `<Mod4>k` and *Mod4+K*, since GTK3 on Wayland and X11 gives Super as Mod4. So a key row and a button row name Super differently. Making keys say Super too would change every row the dialog shows, and the ticket asked for neither.
- **The inhibitor needed a change to the grab.** GTK3's `gdk_wayland_seat_grab` holds a shortcuts inhibitor only for a keyboard-only grab, and the stock dialog grabs the whole seat. Under Caminito the keyboard is grabbed alone first and the rest of the seat after, and `gdk_seat_ungrab` lets both go and the inhibitor with them. A press goes to the window under the pointer (184), so the checklist item presses over the dialog.
- **GTK numbers the buttons as X does,** read from GTK 3.24's `pointer_handle_button`: BTN_SIDE is 8, BTN_EXTRA 9, up to 20 for 0x11f, which is Caminito's last (184). The parser and the capture take 1 to 3, 8 and 9 and up to 20, and not 4 to 7.
- **A bare left click ends the capture under Caminito, which isn't "as now" everywhere.** On Xvfb the stock grab took a left click on another row or outside the window and the capture went on until Escape, under Marco as under the gate. So the patch ends it on a bare left click, and the click then goes to the list. A bare right click goes to the list as before.
- **Under Marco a button value showed as *Mod4+0xffffff*, not disabled:** stock's parser takes `Button8` for an unknown keysym. So the dialog reads a button anywhere, and outside Caminito shows the row as *Disabled* without writing it. That's the one change outside the gate, and it's what the ticket asked for.
- **The `mouse-button-modifier` warning** reads `org.mate.Marco.general`, the key Caminito reads (162), and also warns for Shift with it on the left button, the snapping move that Caminito's `held_by` takes too.
- **The patch is in `packaging/patches/mate-control-center/`,** not beside the PKGBUILD, as ADR 0030 has every carried patch. `pkgrel` is 6, and 184's, 185's and the new checklist items ask for 1.29.1-6.
- **Run by hand, on Xvfb under X11 with `MATE_WM_NAME=Caminito`:** the capplet built from the patched tree against Ubuntu's libraries, with the mate-desktop and Marco version checks lowered in a scratch copy only. Through `xdotool`: Super with button 8 wrote `<Super>Button8` and showed *Super+Back Button*. Button 9 alone and button 10 on a custom shortcut gave *Forward Button* and *Button 10*. A bare middle button asked first, and *Cancel* and Escape left the row. Alt+middle warned and *OK* set `<Alt>Button2`. A button another row had asked to reassign, and *Reassign* moved it. A bare left click ended the capture, Backspace cleared the row, Escape cancelled and the wheel wasn't captured. A key was captured as before, as `<Mod4>k`. Values already in dconf were shown, in English and with `LANG=es_ES.UTF-8` in Spanish. With `MATE_WM_NAME=Marco` a button value showed *Disabled* and was kept, button 8 captured nothing, and Super+K was captured.
- **Not run: `makepkg` and `namcap`.** The environment's network policy refused Arch's mirrors to an Arch container, so the first criterion is the developer's. The series applies to the tag with `patch`, and the keybindings capplet builds without new warnings.
- **Not run: the checklist item in a Session, and its X11 step.** Xvfb is X11 with no compositor, so the inhibitor and a press reaching Caminito are only read from GTK's and Caminito's sources.
- **The suite, on this cloud machine (Ubuntu 24.04, not Arch):** nothing outside the patch, the PKGBUILD and the docs changed. `tools/check`'s rustfmt passed, and its clippy stopped in `caminito-theme`, untouched here, on Rust 1.97's `manual_range_contains`. So the rest was run by hand with `--no-fail-fast`: the workspace built, and 2610 tests passed with none failed, caminito's among them, on a private bus. `caminito-theme`'s `goldens` failed on pixels, as in *Yaru/unmaximize-pressed@1x.png: 20 pixels differ, by up to 16*, which is Ubuntu's Yaru and renderer against goldens made on Arch. Not run: the palette helper, the helpers' meson tests, caminito-locker and the extension.

