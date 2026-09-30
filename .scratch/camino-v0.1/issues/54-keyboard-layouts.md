# Keyboard layouts

Type: implementation
Status: open
Milestone: M4
Blocked by:
Map: [Camino v0.1](../map.md)

## What to build

`org.mate.peripherals-keyboard-xkb.kbd` as the keymap, live ([09](09-gsettings-mapping.md)).

- **RMLVO:** `layouts`, `options` and `model` are libmatekbd strings (`layout\tvariant`, `group\toption`). Camino converts them to rules `evdev`, the model, comma-joined layouts and variants, and options.
- **Live:** a change rebuilds the keymap and sets it on the seat's keyboard, replacing today's default `XkbConfig`.
- **Fallback:** empty lists fall back to `XKB_DEFAULT_*`, then to localed's X11 keymap.
- **Groups:** `grp:*` options switch groups through xkbcommon. `general group-per-window` and `default-group` are unsupported: v0.1 has one global group. Documented, not logged.
- **Xwayland:** X clients follow the new keymap through Xwayland's seat, as they do nested.

## Acceptance criteria

- [ ] Conversion unit tests, written before the code: one and several layouts with and without variants, options, a model, empty lists and each fallback.
- [ ] Headless tests: setting `layouts` live sends a test client a new keymap with the layouts, and a `grp:` option's key switches the group.
- [ ] Nested, `gsettings set org.mate.peripherals-keyboard-xkb.kbd layouts "['us', 'es']"` with `options "['grp\tgrp:alt_shift_toggle']"` switches between them in a GTK3 app and in xterm.
