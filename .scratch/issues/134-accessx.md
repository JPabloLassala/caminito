# AccessX: sticky, slow and bounce keys

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M7
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`input::accessx`, a pure filter in the key path after the lock's and before the Keyboard monitor's ([13](13-hidpi-and-accessibility-scope.md)'s *AccessX*, [15](15-crate-and-module-architecture.md)). msd's `a11y-keyboard` plugin is X11-only, so Caminito applies `org.mate.accessibility-keyboard` itself.

- **Sticky keys:** `stickykeys-enable`, `-latch-to-lock`, `-two-key-off` and `-modifier-beep`.
- **Slow keys:** `slowkeys-enable`, `-delay`, and its beeps on press, accept and reject.
- **Bounce keys:** `bouncekeys-enable`, `-delay` and `-beep-reject`.
- **Toggle-key beeps:** `togglekeys-enable`.
- **The gestures,** with `enable`: Shift five times toggles sticky keys, and Shift held for eight seconds toggles slow keys. Caminito writes the key and posts a notification through `org.freedesktop.Notifications`. msd's confirmation dialog isn't reproduced.
- **The beeps** go where [105](105-one-bell-event.md)'s bell event goes.
- **While locked** the filter still applies ([125](125-lock-state-and-ext-session-lock.md)).
- **Mouse keys** are later (13): `mousekeys-*` is read by nothing.

## Acceptance criteria

- [x] Unit tests of the filter with a driven clock, written before the code (14):
  - sticky: a modifier latches for the next key, locks on a second press with latch-to-lock, and two keys at once turn it off with two-key-off;
  - slow: a key held less than the delay is dropped, and one held longer is delivered once, at the delay;
  - bounce: a second press of the same key within the delay is dropped, and another key isn't;
  - each beep is asked for when its key says so;
  - the two gestures toggle, and only with `enable`.
- [x] Headless tests: a latched modifier reaches the focused client in its modifiers, a binding fires with a latched modifier, and the keys are read at start and followed live.
- [ ] The checklist has the item (*Sticky keys from the Keyboard dialog, in both kinds of window and at the lock screen*), tagged M7. It wasn't run in a Session, which is left to [136](136-packaging-and-closing-m7.md). The item: sticky keys set in the control center's Keyboard dialog work in a native and an X11 window and at the lock screen, and Shift five times toggles it with a notification.

## Notes

- **xkbcommon's own sticky keys** replace this one when Smithay exposes them (13). Nothing here depends on that.
- **The Keyboard dialog runs on XWayland** under Caminito ([102](102-mouse-and-keyboard-capplets.md)). Its Accessibility page writes the same keys.
- **A latched modifier is its key held down.** The filter never touches xkb's state: it holds back the modifier's release until the next key's press has gone on, and until the modifier's next press when it's locked. The bindings, the Window switcher and the client see the modifier held, as with the key itself. A client sees `depressed` where XKB's own sticky keys would give it `latched`.
- **A key's role is its unshifted keysym in the active layout**, so an option that moves Ctrl moves what sticky keys hold. Shift, Control, Alt and Meta, Super and Hyper, AltGr and the fifth level latch. CapsLock, NumLock and ScrollLock are the toggle keys, and take no latch.
- **The filter's order is XKB's:** bounce keys, slow keys, then sticky keys. The gestures count the keys as the device sends them.
- **A second press of a latched modifier without latch-to-lock** unlatches it, so there's a way to take a latch back.
- **Two keys at once** means a key pressed while a modifier's key is down. What was latched still goes with that press.
- **A pointer button takes no latch.** The filter sees keys only, so a latched Ctrl makes the next click a Ctrl+click and stays latched for the next key.
- **Settings read before a gesture's key was written don't undo it:** the filter keeps a feature it turned on or off itself until its key says the same.
- **Bounce keys** drop a press of the key last released, within the delay of that release, as XKB does. A dropped press's release counts, so a key that keeps bouncing keeps being dropped.
- **Shift five times** counts presses no more than 15 seconds apart with no other key between them, and toggles at the fifth release.
- **The filter turns a feature on or off at once** and then writes the key, so the keys typed before the key comes back are already filtered the new way. Two keys at once write `stickykeys-enable` and post a notification too.
- **The toggle keys beep at each press of a lock key.** One bell event can't tell on from off, so there's no second beep for off.
- **The notifications are in English.** Caminito has no translations yet.
- **Left out, and logged as Unsupported keys when turned on:** `timeout-enable` with `timeout`, `capslock-beep-enable` and `feature-state-change-beep`. [13](13-hidpi-and-accessibility-scope.md) has them in v0.1 and this ticket didn't list them. No M7 ticket carries them yet. `mousekeys-enable` is logged the same way.
- **While locked:** the filter is stage 2, after the lock's stage, which passes every key until [125](125-lock-state-and-ext-session-lock.md). Nothing in the filter asks whether the session is locked.
- **The checklist item wasn't run.** It needs a Session with the control center's Keyboard dialog and the locker, and this ticket was done without one. It is left to [136](136-packaging-and-closing-m7.md).
