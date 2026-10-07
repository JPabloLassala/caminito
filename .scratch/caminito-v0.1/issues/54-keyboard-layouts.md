# Keyboard layouts

Type: implementation
Status: resolved
Milestone: M4
Blocked by:
Map: [Caminito v0.1](../map.md)

## What to build

`org.mate.peripherals-keyboard-xkb.kbd` as the keymap, live ([09](09-gsettings-mapping.md)).

- **RMLVO:** `layouts`, `options` and `model` are libmatekbd strings (`layout\tvariant`, `group\toption`). Caminito converts them to rules `evdev`, the model, comma-joined layouts and variants, and options.
- **Live:** a change rebuilds the keymap and sets it on the seat's keyboard, replacing today's default `XkbConfig`.
- **Fallback:** empty lists fall back to `XKB_DEFAULT_*`, then to localed's X11 keymap.
- **Groups:** `grp:*` options switch groups through xkbcommon. `general group-per-window` and `default-group` are unsupported: v0.1 has one global group. Documented, not logged.
- **Xwayland:** X clients follow the new keymap through Xwayland's seat, as they do nested.

## Acceptance criteria

- [x] Conversion unit tests, written before the code: one and several layouts with and without variants, options, a model, empty lists and each fallback.
- [x] Headless tests: setting `layouts` live sends a test client a new keymap with the layouts, and a `grp:` option's key switches the group.
- [x] Nested, `gsettings set org.mate.peripherals-keyboard-xkb.kbd layouts "['us', 'es']"` with `options "['grp\tgrp:alt_shift_toggle']"` switches between them in a GTK3 app and in xterm.

## Notes

- **Implemented** in 6fc330e. The conversion unit tests (`settings/translate/tests.rs`) and the headless keymap tests (`tests/keymap.rs`) pass.
- **The nested check** passed on 2026-10-04, in the UTM virtual machine (Arch Linux ARM, GTK 3.24.52, xterm 411, Xwayland 24.1.13), at 6bb2a72. Nobody pressed the keys. Other work had the machine's ttys, so Caminito ran nested on a private `Xvfb` display, and XTEST pressed the keys on that display. `54-hand-checks.sh` does the same check with a person at the keyboard on a host desktop.
  - `layouts` and `options` were set on the nested bus. Xwayland's `xkb_symbols` changed from `pc_us_inet(evdev)` to `pc_us_es_2_inet(evdev)_group(alt_shift_toggle)`.
  - GTK3 app (a native Wayland `Gtk.Entry`): `;` typed `;`, then `ñ` after Alt+Shift, then `;` after Alt+Shift again.
  - xterm (on the nested Xwayland): `;` typed `;`, then `ñ` after Alt+Shift.
  - One global group: with xterm on the Spanish layout, a click on the GTK3 field and `;` typed `ñ` there with no other Alt+Shift. Alt+Shift there, then a click on xterm, typed `;` in both.
  - Not checked: `gtk3-demo` or `mate-terminal` as the GTK3 app, and a host desktop that takes Alt+Shift for itself.
