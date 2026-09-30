# libinput on the tty

Type: implementation
Status: open
Milestone: M4
Blocked by: 53, 55
Map: [Camino v0.1](../map.md)

## What to build

Real input devices on [55](55-the-tty-backend.md)'s tty, with [53](53-input-settings-translation.md)'s settings applied ([09](09-gsettings-mapping.md), [15](15-crate-and-module-architecture.md)).

- **libinput** through the libseat session, suspended and resumed with it. Its events go through `input::process_input_event`, which is already generic over the backend.
- **Settings:** each device gets 53's settings for its kind when it's added, and again when a key changes.
- **Lid switch:** libinput's switch devices tell [50](50-default-output-scale.md)'s rule whether a lid switch exists, as KWin does (13).
- **Smithay feature:** `backend_libinput`.
- **Not in M4:** touch and tablet input, and mapping absolute pointer motion to anything but the first output (decided 2026-09-30).

## Acceptance criteria

- [ ] On the tty as `caminotest`: the mouse, the touchpad (if the machine has one) and the keyboard work.
- [ ] `gsettings set` on each of 53's mouse, touchpad and keyboard keys applies live, and a device plugged in afterwards gets them.
- [ ] With `kbd layouts` set, the keyboard types in the chosen layout on the tty.
