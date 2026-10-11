# Spec: System shortcuts

Written on 2026-10-11 from a grilling session with the developer. It holds the decisions; the four tickets say what to build.

Nothing here is built.

## Problem

The control centre's *Keyboard Shortcuts* lists what Caminito acts on before any client sees it, and only a key can be one. A mouse with side buttons can't have one bound to a window action or a command, and the dialog's name would be wrong if it could.

## Solution

- The thing is a **System shortcut** ([CONTEXT.md](../../CONTEXT.md)), and the dialog is *System Shortcuts* under Caminito.
- A System shortcut can be a mouse button, with or without modifiers. Caminito's own bindings and the accelerators mate-settings-daemon asks for both take one.
- The dialog captures a button as it captures a key.

## Decisions

### The name

- A System shortcut is a combination of modifiers with a key or a mouse button that Caminito acts on before any client sees it, whoever owns the action: Caminito's own bindings, a command's shortcut, or a media key.
- "Binding" stays the word for one of Caminito's own, and "accelerator" for one another program asks for. "Keyboard shortcut" and "keybinding" are avoided.
- The dialog's title and its launcher say *System Shortcuts* under Caminito, with the Spanish strings carried. Other locales show the English.
- Under Marco on X11 the dialog is *Keyboard Shortcuts*, as stock, and captures keys only (ADR 0011).
- `README.md`, `docs/parity.md` and the checklist take the new name. Resolved tickets keep the old one.

### Which buttons

- The middle button, back, forward and any further side buttons, alone or with modifiers.
- The left and right buttons, with a modifier only.
- Not the wheel, up, down or tilted: it isn't a press, and the titlebar has its own scroll actions.
- Anything that arrives as a pointer button counts, so a touchpad's three-finger tap is the middle button. A tablet's buttons and touch are out.
- A bare middle button ends middle-click paste and the titlebar's middle-click action, so the dialog asks first, as it does for a bare letter. Bare back and forward need no confirmation.

### How a button is stored

- In the same string as a key, where the keysym's name would be: `<Super>Button8`, `Button2`. The numbers are X's: 2 is middle, 8 back, 9 forward ([ADR 0028](../../docs/adr/0028-a-mouse-button-is-written-into-marcos-keybinding-keys.md)).
- Marco and stock mate-settings-daemon can't parse it, so on X11 MATE that shortcut is unbound and the dialog shows it as disabled.

### How a button is shown

- *Middle Button*, *Back Button* and *Forward Button* for 2, 8 and 9, *Left Button* and *Right Button* for 1 and 3, and *Button 10* and up by number.
- Joined to modifiers as a key is: *Super+Back Button*.
- The dialog and the Window menu's hints both show it so.

### What a press does

- The action runs on the press, wherever the pointer is: a window, the panel, the desktop. No client sees the press or the release.
- It doesn't fire while the session is locked, or during a move, a resize, a drag-and-drop or an open menu: the button goes to whatever holds the pointer.
- It changes neither focus nor stacking. The action applies to the focused window, not the one under the pointer, as for a key.
- With modifiers, a button drives the Window switcher and the Workspace popup as a key does: each press steps, and letting go of the modifiers picks. With none, it acts as a key with no modifier does. A press of the button that drives a popup doesn't close it.

### Precedence

- Over a window, a `mouse-button-modifier` click (move, resize, the Window menu) wins over a System shortcut on the same combination. Elsewhere the System shortcut runs. The dialog warns when one of those three is captured.
- Caminito's binding wins over another program's accelerator on the same button, and a second program's grab of a taken one is refused, as for keys. `GrabAccelerators` keeps its signature.
- While a client that has the focus holds a shortcuts inhibitor, button System shortcuts and modifier clicks go to that client, as key ones do. That's how the dialog sees the press it captures, and a virtual machine or remote desktop window gets its clicks by it too. No new protocol or D-Bus call.

### Capture in the dialog

- While a row is capturing, a press of a button that can be a System shortcut becomes it, with the modifiers held.
- A bare left click outside the row cancels, Escape cancels and Backspace clears, as now.

## Tickets

- [183](../issues/183-the-dialog-is-system-shortcuts.md): the dialog is named System Shortcuts under Caminito.
- [184](../issues/184-caminito-binds-mouse-buttons.md): Caminito binds mouse buttons.
- [185](../issues/185-mouse-buttons-as-accelerators.md): mouse buttons as accelerators. Blocked by 184.
- [186](../issues/186-the-dialog-captures-a-mouse-button.md): the dialog captures a mouse button. Blocked by 183, 184 and 185.

183 is independent of 184 and 185.
