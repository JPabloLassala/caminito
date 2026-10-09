# The Night Light tab in caminito-displays

Epic: night-light
Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: 137, 138, 140
Map: [Night Light](../night-light/map.md)

## What to build

Night Light's settings, under "caminito-displays" in [the spec](../night-light/spec.md).

- **A second tab, "Night Light,"** beside the displays page. Today's page becomes the first tab, "Displays", unchanged. The new tab has:
  - an on/off switch;
  - the schedule, as "Follow the sun" or "Constant";
  - the noon and night temperature sliders for the first, the constant one for the second, from warm to neutral and showing kelvin;
  - "Pause until sunrise" or "Resume", with "Paused until 06:42" from the key;
  - "Location unknown, using fixed times" when `NightLight`'s `location-source` is `none`.
- **It writes the keys through gio** as they change, with no Apply. There's no revert countdown here: nothing on this tab can leave the screen unusable.
- It calls `PauseNightLight()` and `ResumeNightLight()`, and follows the keys and the property when they change from elsewhere.
- **Its state is toolkit-free,** in its own module beside `page.rs`, with unit tests.
- **`--night-light`** opens the window on the tab, or switches a running one to it.
- **`--toggle-night-light`** calls `ToggleNightLight()` and exits with no window. It's for a custom shortcut (100).
- **`caminito-night-light.desktop`** goes in Preferences: "Night Light", with an icon from the theme, running `caminito-displays --night-light`. It's shown only in MATE.
- **`--demo`** shows the tab with a made-up state.
- **The README** says how to bind the toggle to a shortcut.

## Acceptance criteria

- [ ] Unit tests of the tab's state: the sliders shown follow the schedule, the pause button's label and text follow the key, and the note follows the source.
- [ ] A headless test with the nested Caminito: changing a control writes its key; `--toggle-night-light` flips `enabled` and opens no window; `--night-light` on a running window switches it to the tab.
- [ ] The package installs the desktop entry, and `desktop-file-validate` passes.
- [ ] In a Session, by the developer: the tab changes the screen with a fade; Night Light from the Preferences menu opens it; a shortcut bound in Keyboard Shortcuts to `caminito-displays --toggle-night-light` toggles it.
