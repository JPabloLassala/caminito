# The Keyboard monitor for Orca

Epic: caminito-v0.1
Type: implementation
Status: resolved
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

- [x] Headless tests with a D-Bus client, written before the code (14):
  - the allowlist: a client without the name is refused, and one with it isn't;
  - a watch gets a `KeyEvent` for each key and the focused client still gets the key;
  - a grab of the keyboard, and a key grab, get the events and the client doesn't;
  - the lone-modifier rule, with a driven clock, both ways;
  - a grabbed key that is also a binding goes to the monitor;
  - the grabs are gone when the sender leaves the bus;
  - while locked, the events still come.
- [ ] The checklist has the item (*Orca with the Keyboard monitor and on the lock screen*), tagged M7 (17). It wasn't run in a Session, which is left to [136](136-packaging-and-closing-m7.md). The item: Orca starts from mate-session, reads the panel's menus and a native window, its modifier works with the focus on the desktop, and it reads `caminito-locker`'s dialog without echoing the password.

## Notes

- **Mouse review** (`PointerLocator`) and Caminito's own popups are later (13).
- **at-spi2-core needs 2.55 or later** to try the monitor. Check Arch's at 16's baseline.

## Done (2026-10-09)

- **Test first:** the eleven headless tests in `caminito/src/tests/keyboard_monitor.rs` were written before the code, and didn't build until the interface did. The lone-modifier rule was checked by breaking its delay: the test of the press alone failed, and only it.
- **Stage 3 runs before xkbcommon:** a key the monitor takes never reaches Smithay's keyboard, so it changes no state, as Mutter's interface asks of CapsLock and NumLock. Its keysym, character and modifiers are read from the state before it. Its code is xkb's, evdev's plus 8, as Mutter's is.
- **Who is told:** every caller that watches, and every caller that grabbed the key. The release of a key the monitor took goes to the callers that grabbed its press, and never on, even if the grab is gone by then.
- **A grabbed modifier** takes the keys pressed while it's held back, as Mutter's interface says and niri does. Pressed again within the repeat delay of its last press, it goes on as itself, and takes nothing while it's held then. The delay is `org.mate.peripherals-keyboard delay`, and the times are the key events', so the tests drive them.
- **Keystrokes match their mask exactly.** at-spi2-core sends each one with and without NumLock and CapsLock (`atspi-device-a11y-manager.c:151-154`).
- **The allowlist** is checked at each call, by asking the bus who owns `org.gnome.Orca.KeyboardMonitor`. A refused call gets `org.freedesktop.DBus.Error.AccessDenied` and leaves no entry, so a refused caller is told nothing.
- **The `KeyEvent`s** go into a channel that one task on zbus's executor empties, each to its caller as a unicast signal, so they arrive in the order of the keys.
- **The name** `org.freedesktop.a11y.Manager` is asked for with the other well-known names, without replacing an owner: in a nested run the host's compositor may have it, and Caminito logs a warning and carries on.
- **at-spi2-core's version on Arch** couldn't be checked from where this was done (archlinux.org refused the request). 13's source read was of 2.62, past the 2.55 the monitor needs; 136 checks the installed one when it runs the item.
- **The checklist item wasn't run.** It needs a Session with Orca and `caminito-locker`, and this ticket was done without one. It is left to [136](136-packaging-and-closing-m7.md).
