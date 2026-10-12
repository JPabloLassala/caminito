# Caminito binds mouse buttons

Epic: system-shortcuts
Type: implementation
Status: open: the code is in, the full suite wasn't run at the developer's word, and the checklist item needs a Session
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

## Done (2026-10-12)

- **Tests first for the parser, the table and the first presses.** The parser's and the table's unit tests were written and seen to fail before their code, and so were the first headless tests in `caminito/src/tests/button_bindings.rs`: a bound button runs its action, the client sees nothing, focus and stacking stay. The tests of the lock, a move, the menus, the popups, a modifier click and the inhibitor were written after the pointer path, so each guard was then broken in turn, and each break failed a test.
- **The highest button read is 20.** X's numbers from 8 up are the kernel's codes from `BTN_SIDE` (0x113) in order, as xf86-input-libinput and Xwayland number them: 8 is `BTN_SIDE`, 9 `BTN_EXTRA`, 10 `BTN_FORWARD`, 11 `BTN_BACK`, 12 `BTN_TASK`, and so on to 20, which is 0x11f, the last code before `BTN_JOYSTICK`. 1, 2 and 3 are `BTN_LEFT`, `BTN_MIDDLE` and `BTN_RIGHT`. `Button21` and up don't parse. `x_button` in `caminito/src/input/bindings/accelerators.rs` has the mapping.
- **A number is written as X writes it.** `Button08` and `Button+8` don't parse, since the syntax is in users' settings for good (ADR 0028) and a strict one can be loosened later.
- **A shortcuts inhibitor didn't keep modifier clicks from Caminito, and now does.** Before this ticket `on_pointer_button` asked `mouse-button-modifier` what a press did with no look at the inhibitor, so Alt and the left button moved a window whose client held one. Now an active inhibitor on the focused client turns off both the modifier clicks and the button bindings, and the press takes the ordinary path: it goes to whatever is under the pointer, and focuses and raises a window there as a plain click does.
- **What holds the pointer.** A binding gives way to the session lock, a Smithay pointer grab (a move, a resize, a drag-and-drop, an xdg popup, a button already down on a client), a keyboard move or resize, the Window menu, and an X11 client's menu. The last takes no pointer grab Caminito can see, so its keyboard grab through Xwayland stands for it; the code review found that one, and its test failed first. A drag-and-drop and an xdg popup have no test of their own.
- **The popups take a button before the inhibitor does,** as they take a key: an open Window switcher or Workspace popup is stepped by a bound button whether or not the focused client holds an inhibitor.
- **The button that opened the Window switcher steps it,** as the opening key does, with other modifiers held too. Any other button still picks a cell or cancels (108), and any button with no binding to the workspace beside still closes the Workspace popup (152).
- **A button with no modifier on a `switch-*` or `cycle-*` action binds nothing and is logged,** as a key with none is today, and one on a workspace switch switches and shows no popup.
- **`GrabAccelerators` refuses a button until 185.** The one parser now reads a button for both paths, and nothing signals a grabbed one yet, so `Grabs::grab` returns 0 with a log line. 185 takes that arm out.
- **The button's label is in `caminito/src/window_menu/labels.rs`,** with its unit test there, and `caminito/src/tests/window_menu.rs` checks the hint through the debug dump. The names are Caminito's own English and aren't translated, since Marco's catalogue has none.
- **The Window menu's *Maximize* item shows `maximize`'s binding, not `toggle-maximized`'s,** as before, so the checklist item binds `minimize` to see a hint.
- **Not run: the full suite and `tools/check`.** The developer asked for the relevant tests alone. Run, on a private bus with clippy and rustfmt clean: the `input`, `switcher`, `workspace_popup` and `window_menu` modules and the headless tests of bindings, button bindings, modifier clicks, the Window switcher, the Workspace popup, the Window menu, accelerators, grabs, frames, parity, popups, the lock, the keybinding manifests, the debug dump, override-redirect windows, keyboard grabs, drag icons and the focus modes. `frames::any_mouse_button_on_a_maximised_windows_button_unmaximises_it::x11` failed once in a parallel run and passed in ten reruns.
- **Not run: the checklist item,** *A mouse button as a System shortcut*, which needs a Session and a mouse with back and forward buttons.
- **Not done: line numbers for the two citations in `x_button`'s doc,** which name the function and file in xf86-input-libinput and Xwayland; neither source was at hand.
