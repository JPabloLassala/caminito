# libinput on the tty

Type: implementation
Status: resolved
Milestone: M4
Blocked by:
Map: [Caminito v0.1](../map.md)

## What to build

Real input devices on [55](55-the-tty-backend.md)'s tty, with [53](53-input-settings-translation.md)'s settings applied ([09](09-gsettings-mapping.md), [15](15-crate-and-module-architecture.md)).

- **libinput** through the libseat session, suspended and resumed with it. Its events go through `input::process_input_event`, which is already generic over the backend.
- **Settings:** each device gets 53's settings for its kind when it's added, and again when a key changes.
- **Lid switch:** libinput's switch devices tell [50](50-default-output-scale.md)'s rule whether a lid switch exists, as KWin does (13).
- **Smithay feature:** `backend_libinput`.
- **Not in M4:** touch and tablet input, and mapping absolute pointer motion to anything but the first output (decided 2026-09-30).
- **Devices queued at start-up (found 2026-10-04):** libinput queues the seat's devices as plugged in when the seat is assigned and when the session resumes, without becoming readable. So they waited for the first key or motion, and until then no lid switch reached 50's rule and no keyboard showed the LEDs. `backend::tty` now handles the queued devices at once, as wlroots does, with the key and button releases that suspending queued before them.

## Acceptance criteria

- [x] On the tty as `caminitotest`: the mouse, the touchpad (if the machine has one) and the keyboard work.
- [x] `gsettings set` on each of 53's mouse, touchpad and keyboard keys applies live, and a device plugged in afterwards gets them.
- [x] With `kbd layouts` set, the keyboard types in the chosen layout on the tty.

## Checks (2026-10-04)

Run in the Arch Linux ARM virtual machine, as `caminitotest` in a logind session on tty4, with uinput devices and `wev`. The scripts are in [56-tty-checks](56-tty-checks/README.md).

- **Devices:** a mouse, a clickpad, a touchpad with buttons and a keyboard move the pointer, click, scroll and type. Each is handled when it's plugged in, at start-up and after a VT switch back. A mouse button held across a VT switch is released for the client.
- **Mouse keys, live:** `left-handed`, `motion-acceleration`, `accel-profile` and `middle-button-enabled` each change what a client receives. A mouse plugged in afterwards is left-handed too.
- **Touchpad keys, live:** `touchpad-enabled`, `left-handed` (`left`, and `mouse` following the mouse key), `motion-acceleration`, `accel-profile`, `natural-scroll`, `tap-to-click`, clickfinger through `two-finger-click`, `vertical-two-finger-scrolling` and `horizontal-two-finger-scrolling`. A touchpad plugged in afterwards taps too.
- **Keyboard keys, live:** `rate`, `delay` and `repeat` reach the client as repeat info.
- **Layouts:** with `kbd layouts` at `['fr']`, the Q key types `a`, also on a keyboard plugged in afterwards and after Caminito restarts. With `['us\tdvorak']` it types `'`.
- **Not checked:** real hardware; `disable-while-typing`, which libinput took but uinput devices can't show; edge scrolling; the mouse's `natural-scroll`, which the installed schema lacks; a lid switch on a built-in panel, which the headless tests cover. On a clickpad, libinput leaves a click outside the button areas as the left button when left-handed.
