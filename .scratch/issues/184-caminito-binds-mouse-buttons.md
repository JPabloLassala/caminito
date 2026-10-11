# Caminito binds mouse buttons

Epic: system-shortcuts
Type: implementation
Status: ready-for-agent
Blocked by: none
Spec: [System shortcuts](../system-shortcuts/spec.md)

## Why

A System shortcut can be a mouse button ([CONTEXT.md](../../CONTEXT.md)). This is Caminito's own half: a binding read from `org.mate.Marco.global-keybindings` or `window-keybindings` that names a button runs its action when the button is pressed.

## What to build

### The syntax

[ADR 0028](../../docs/adr/0028-a-mouse-button-is-written-into-marcos-keybinding-keys.md) has it. The parser in `caminito/src/input/bindings/accelerators.rs` reads it, for this ticket and for [185](185-mouse-buttons-as-accelerators.md).

- **A button is `Button` and X's number,** where a keysym's name would be, after GTK's modifiers: `<Super>Button8`, `Button2`. 1 to 3 are left, middle and right, 8 is back and 9 is forward, and the further side buttons follow.
- **`Button1` and `Button3` need a modifier.** Alone they don't parse.
- **`Button4` to `Button7` don't parse:** they're the wheel's.
- **A value that doesn't parse** binds nothing and is logged, as an unparseable key is today.

### The pointer path

- **The bindings table matches a button press** with exactly the binding's modifiers held, the locks aside, as it matches a key.
- **The action runs on the press,** wherever the pointer is: a window, a Frame, the panel, the desktop.
- **No client sees the press or the release.** A press that started under one table ends under it, as for keys (148).
- **It doesn't fire** while the session is locked, or during a move, a resize, a drag-and-drop or an open menu or popup: the button goes to whatever holds the pointer.
- **Focus and stacking don't change.** The action applies to the focused window, not the one under the pointer.

### The popups

- **With modifiers,** a button drives the Window switcher and the Workspace popup as a key does: each press steps, and letting go of the modifiers picks.
- **With none,** it acts as a key with no modifier does today.
- **The Workspace popup** closes on a press of a pointer button (152). A press of the button whose binding drives it steps it instead.

### Precedence

- **A `mouse-button-modifier` click wins over a window** (162): with the same combination bound, the move, the resize or the Window menu happens there, and the binding runs when the pointer is over anything else.
- **A shortcuts inhibitor covers the pointer.** While the focused client holds an active one, a button binding and a modifier click don't happen and the press is that client's, as a key binding's is.

### Labels

- **The Window menu's hints** show a button binding: *Middle Button*, *Back Button* and *Forward Button* for 2, 8 and 9, *Left Button* and *Right Button* for 1 and 3, and *Button 10* and up by number, joined to the modifiers as a key is (*Super+Back Button*).
- **The debug dump** lists button bindings with the rest.

## Acceptance criteria

- [ ] Unit tests of the parser, written before the code: a button with and without modifiers, in any order and case; `Button1` and `Button3` alone; `Button4` to `Button7`; `Button0` and a button with no number; and that every key accelerator the tests have today parses as before.
- [ ] Unit tests of the table: a button binding beside a key binding, two bindings on one button, a changed key, `disabled`.
- [ ] Headless tests through the fixture, on an xdg and an XWayland window:
  - a bound button runs its action, and the client under the pointer gets neither the press nor the release;
  - an unbound button reaches the client as before;
  - the action applies to the focused window when the pointer is over another, and neither focus nor stacking changes;
  - it doesn't run while the session is locked, during a move or a resize, or with the Window menu open;
  - `<Super>Button9` on `switch-windows` steps the Window switcher on each press and picks on Super's release;
  - a binding on a workspace switch shows the Workspace popup, and further presses of the button step it;
  - `<Alt>Button2` bound to an action: over a window the modifier click happens, and over the desktop the action runs;
  - with an active shortcuts inhibitor on the focused surface, the press reaches the client, and so does an Alt+left click.
- [ ] The Window menu shows a button binding's label, with a test in `caminito/src/window_menu/`.
- [ ] `docs/parity.md` says, at `global-keybindings` and `window-keybindings`, that a button is read where Marco reads none, with ADR 0028 and the tests above.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item, set with `dconf` since the dialog can't capture a button until [186](186-the-dialog-captures-a-mouse-button.md): the back button with Super bound to an action runs it over a window and over the panel, and a browser no longer goes back on it.

## For the implementing agent to settle

Recorded in the Done section:

- the highest button number read, and how X's numbers above 9 map to the kernel's button codes;
- whether a shortcuts inhibitor already kept modifier clicks from Caminito, and if so what changed.
