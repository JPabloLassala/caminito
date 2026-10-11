# 28. A mouse button is written into Marco's keybinding keys

Date: 2026-10-11
Status: Accepted

## Context

A System shortcut can be a mouse button, with or without modifiers, as well as a key. Caminito's own bindings are stored in `org.mate.Marco.global-keybindings` and `org.mate.Marco.window-keybindings`, which it reads as Marco reads them (148), and a command's shortcut is stored in mate-settings-daemon's own keys (100). Every one of those values is a string in GTK's accelerator syntax, which names modifiers and a keysym and has no way to name a button.

The control centre's dialog has one string for each row: it reads the row's key, shows it, and writes the captured combination back to it.

Two ways were open:

- a button in the same string, in a syntax of Caminito's own;
- new keys beside the old ones, in a schema of Caminito's, holding only the button.

## Decision

- **A button is written where a keysym's name would be,** as `Button` and X's number for it: `<Super>Button8`, `Button2`. The modifiers before it are GTK's, as for a key. The developer chose this on 2026-10-11.
- **The numbers are X's,** which GTK, `xev` and Marco's `mouse-button-modifier` clicks already use: 1 to 3 are left, middle and right, 8 is back and 9 is forward. The wheel's 4 to 7 aren't buttons here and don't parse.
- **The left and right buttons need a modifier.** `Button1` and `Button3` alone don't parse.
- **One parser reads it** for both paths, Caminito's bindings and the accelerators another program asks for, so `GrabAccelerators` keeps its signature and takes a button as one more string.
- **Only the dialog under Caminito writes it.** Under Marco the patched dialog captures keys alone (ADR 0011).

## Consequences

- The dialog needs no second column and no second key for a row: a row is still one string, and a button replaces a key as another key would.
- Marco and stock mate-settings-daemon can't parse a button. A user who goes back to X11 MATE finds that shortcut unbound, with a line in the log, and the dialog there shows it as disabled. The value is kept until it's changed.
- `org.mate.Marco.*` holds values that aren't Marco's, in keys that `docs/parity.md` otherwise says are read as Marco reads them. The parity table says so.
- The syntax is in users' settings from the first release that has it, so it can't be changed without reading the old one too.
- A keysym named `Button` and a number would be shadowed. xkbcommon has none.
- Rejected: separate keys in a schema of Caminito's (the dialog would read and write two keys for a row and decide which wins, Marco's key would have to be disabled whenever a button is set, and every action would need a second key).
