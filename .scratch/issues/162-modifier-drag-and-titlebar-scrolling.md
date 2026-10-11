# Modifier-drag and titlebar scrolling from settings

Epic: caminito-v0.1
Type: implementation
Status: resolved: the checklist item's run in a Session is 173's closing run
Milestone: M8
Blocked by: 161
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The pointer actions Marco reads from `org.mate.Marco.general` and Caminito hard-codes today ([05](05-v01-marco-feature-cut.md), [09](09-gsettings-mapping.md)):

- **`mouse-button-modifier`:** the modifier held for a click on a window's body to move it, resize it or open the window menu ([161](161-window-menu.md)). Parsed by 09's rules: a GTK accelerator, where `<Super>` and `<Mod4>` both mean the logo key, an empty string or `disabled` turns modifier-drag off, and anything unparseable falls back to `<Alt>` with a log line. Today Alt is hard-coded.
- **`resize-with-right-button`:** which of the middle and right buttons resizes and which opens the window menu. Today, with Alt, the left button moves and the middle one resizes; Marco's defaults (the key is on) resize with the right button and open the menu with the middle one, which Marco's source confirms.
- **`action-scroll-up-titlebar` and `action-scroll-down-titlebar`:** what a scroll on an SSD window's titlebar does, with the same actions as the click keys ([05](05-v01-marco-feature-cut.md)'s rule that the titlebar keys are SSD frames' only).
- **The modifier and the clients:** the press that starts a modifier-drag never reaches the client, as today, with whichever modifier is set.

## Acceptance criteria

- [x] Unit tests of the parsing, written before the code: Alt, Super, Mod4, Ctrl+Alt if Marco takes it, empty, `disabled`, and garbage with its log line.
- [x] Headless tests through the fixture, on xdg and XWayland windows: a move, a resize and the menu with each button setting; the modifier set to Super; `disabled` passing the press to the client; a scroll up and down on the titlebar doing each key's action; a key change applying at once.
- [x] The four keys leave `UNSUPPORTED`; their rows in `docs/parity.md` say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: mate-control-center's *Windows* set to Super for the modifier, then a move, a resize and the menu on a native and an X11 window; a titlebar scroll set to shade. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section:

- **Whether a scroll action repeats per notch:** a fast scroll shading and unshading several times, or a single action per gesture; and how a high-resolution scroll wheel's steps count.
- **Which modifiers `mouse-button-modifier` takes** beyond one, and what Marco does with the left button when the key is `disabled`.

**For the developer:** none.

## Done (2026-10-11)

- **Whether a scroll action repeats per notch: it does.** Marco's frame gets a scroll as a press of X's button 4 or 5, one for each click of the wheel, and runs the action at each (`meta_frames_button_press_event`, frames.c:1937-1948, read at `8522ed1`). So `toggle_shade` and a fast turn shade and unshade several times, and Caminito does the same. A wheel with finer steps, or a touchpad, acts once for each whole click its steps add up to (120 of a wheel's 120ths, or 15 of libinput's units), which is when the X server's driver presses the button. This part is my reading of the driver, not of Marco's source. What's left of a click is dropped when the pointer leaves the title and when the fingers leave the touchpad.
- **A scroll acts on the title alone,** not on the Frame's buttons or borders, and not while a grab has the pointer, as Marco's (frames.c:1768-1771, 1939-1947). The horizontal axis does nothing.
- **Which modifiers `mouse-button-modifier` takes: one or more,** with no key after them (`meta_ui_parse_modifier`, ui.c:905-950). Marco grabs buttons 1 to 3 with exactly those modifiers, the locks apart, and the left button with Shift too, for a move that snaps (`meta_display_grab_window_buttons`, display.c:4084-4129). So `<Control><Alt>` needs both and no other. On master, Alt with one more modifier still moved the window; now that press is the client's, but Shift with the left button.
- **`<Mod2>`, `<Mod3>`, `<Mod5>` and `<Release>` give `<Alt>` with the log line.** Marco takes the three `Mod`s; Caminito's accelerators match none of them, as for the keybindings (148).
- **The left button when the key is `disabled`:** Marco makes no grab with the modifier (display.c:4098), so every press is the client's and focuses the window as a click does. The same here, for the empty string too.
- **A value that doesn't parse gives `<Alt>`,** as 09 has it, where Marco keeps the value it had (`mouse_button_mods_handler`, prefs.c:1289-1313). The log line is the settings' usual one, `… mouse-button-modifier is 'garbage', which isn't valid; using '<Alt>'`, and doesn't say why.
- **The menu's click raises the window and doesn't focus it,** as Marco's (display.c:2009-2018), and opens the menu at the pointer. The resize asks for the window's resize function (display.c:1962).
- **A press with the modifiers held is never the client's,** whether it started something or not: Alt and the right button on a window of a fixed size does nothing, where master passed a middle press on.
- **Tested first:** the unit tests in `caminito/src/input/bindings/button_modifier.rs` and the new ones in `caminito/src/settings/translate/tests.rs` were written before the parser, and `caminito/src/tests/modifier_click.rs` and the scroll tests at the end of `caminito/src/tests/frames.rs` before the first build. I didn't run them against master to see them fail.
- **The existing Alt+middle tests in `grabs.rs` now use the right button,** since the schema's default for `resize-with-right-button` is on.
- **Found in review and fixed:** three items that needed no `pub`; the menu in the tests closed through Policy, not with Escape; a test of two clicks at once that no action at all would pass; no test of a touchpad's scroll or of a live change to the scroll-up key.
- **Known and left as they are.**
  - The resize with the modifier moves the corner nearest the pointer, as before this ticket. Marco's goes by thirds of the window: a corner, an edge, or nothing in the middle (display.c:1962-1990).
  - What a client sees of a press is tested with the xdg client alone: the fixture's X client records no button presses. The XWayland tests show what the press did to the window.
  - Marco's log line for a bad value and the reason from the parser aren't in Caminito's log.
- **Not run:** the full test suite, by the developer's rule of 2026-10-11. Run after the last edits: `cargo fmt --all --check`, clippy on the workspace, and `caminito`'s `input::`, `tests::modifier_click` and `tests::frames` (163); before the last test edits, also `settings::`, `tests::grabs`, `tests::window_menu`, `tests::parity` and `tests::input` (410 with the above); and before the review's fixes, 745 tests that the change touches (those, and focus modes, edge resistance, tiling, popups, the debug dump, bindings, the keyboard grab, placement, decorations, override-redirect, layer shell, the lock, the Window switcher, the Workspace popup, drag icons and selections). The rest of `tools/check` wasn't run. The checklist's *Modifier clicks and titlebar scrolling from the control center* needs a Session and is 173's.
