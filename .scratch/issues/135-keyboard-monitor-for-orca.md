# The Keyboard monitor for Orca

Epic: caminito-v0.1
Type: implementation
Status: open
Milestone: M7
Blocked by: 125, 134
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`dbus::a11y`: `org.freedesktop.a11y.KeyboardMonitor` at `/org/freedesktop/a11y/Manager`, under the name `org.freedesktop.a11y.Manager`, with Mutter's interface ([13](13-hidpi-and-accessibility-scope.md)'s *Orca key events*). With it Orca's modifier and its global commands work wherever the focus is.

- **The interface:** `GrabKeyboard` and `UngrabKeyboard`, `WatchKeyboard` and `UnwatchKeyboard`, `SetKeyGrabs(au modifiers, a(uu) keystrokes)`, and the `KeyEvent` signal, as in Mutter's `org.freedesktop.a11y.xml`.
- **The lone modifier:** a grabbed modifier pressed alone is swallowed unless it's pressed again within the repeat delay, so Orca's modifier still works as itself on a double press.
- **Who may call it:** only a client that owns `org.gnome.Orca.KeyboardMonitor`. Everyone else gets `AccessDenied`.
- **Per sender,** and dropped when the sender leaves the bus.
- **The order:** the lock's filter, AccessX, this, Caminito's bindings, the focused client (15). An Orca grab outranks a binding, and Orca sees keys after sticky and slow keys.
- **While locked** it keeps working, so Orca can read `caminito-locker`'s dialog.
- **The key path never waits on the bus** (15): signals are sent from the loop without blocking.

## Acceptance criteria

- [ ] Headless tests with a D-Bus client, written before the code (14):
  - the allowlist: a client without the name is refused, and one with it isn't;
  - a watch gets a `KeyEvent` for each key and the focused client still gets the key;
  - a grab of the keyboard, and a key grab, get the events and the client doesn't;
  - the lone-modifier rule, with a driven clock, both ways;
  - a grabbed key that is also a binding goes to the monitor;
  - the grabs are gone when the sender leaves the bus;
  - while locked, the events still come.
- [ ] The checklist has *Orca with the Keyboard monitor and on the lock screen*, tagged M7 (17), and it was run in a Session: Orca starts from mate-session, reads the panel's menus and a native window, its modifier works with the focus on the desktop, and it reads `caminito-locker`'s dialog without echoing the password. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **Mouse review** (`PointerLocator`) and Caminito's own popups are later (13).
- **at-spi2-core needs 2.55 or later** to try the monitor. Check Arch's at 16's baseline.
