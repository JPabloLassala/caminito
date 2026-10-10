# Modifier-drag and titlebar scrolling from settings

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
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

- [ ] Unit tests of the parsing, written before the code: Alt, Super, Mod4, Ctrl+Alt if Marco takes it, empty, `disabled`, and garbage with its log line.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: a move, a resize and the menu with each button setting; the modifier set to Super; `disabled` passing the press to the client; a scroll up and down on the titlebar doing each key's action; a key change applying at once.
- [ ] The four keys leave `UNSUPPORTED`; their rows in `docs/parity.md` say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: mate-control-center's *Windows* set to Super for the modifier, then a move, a resize and the menu on a native and an X11 window; a titlebar scroll set to shade. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section:

- **Whether a scroll action repeats per notch:** a fast scroll shading and unshading several times, or a single action per gesture; and how a high-resolution scroll wheel's steps count.
- **Which modifiers `mouse-button-modifier` takes** beyond one, and what Marco does with the left button when the key is `disabled`.

**For the developer:** none.
