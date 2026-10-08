# AccessX: sticky, slow and bounce keys

Epic: caminito-v0.1
Type: implementation
Status: open
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

- [ ] Unit tests of the filter with a driven clock, written before the code (14):
  - sticky: a modifier latches for the next key, locks on a second press with latch-to-lock, and two keys at once turn it off with two-key-off;
  - slow: a key held less than the delay is dropped, and one held longer is delivered once, at the delay;
  - bounce: a second press of the same key within the delay is dropped, and another key isn't;
  - each beep is asked for when its key says so;
  - the two gestures toggle, and only with `enable`.
- [ ] Headless tests: a latched modifier reaches the focused client in its modifiers, a binding fires with a latched modifier, and the keys are read at start and followed live.
- [ ] The checklist has an item, tagged M7, and it was run in a Session: sticky keys set in the control center's Keyboard dialog work in a native and an X11 window and at the lock screen, and Shift five times toggles it with a notification. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **xkbcommon's own sticky keys** replace this one when Smithay exposes them (13). Nothing here depends on that.
- **The Keyboard dialog runs on XWayland** under Caminito ([102](102-mouse-and-keyboard-capplets.md)). Its Accessibility page writes the same keys.
