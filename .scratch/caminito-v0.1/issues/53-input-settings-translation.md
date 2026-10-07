# Input settings translation

Type: implementation
Status: resolved
Milestone: M4
Blocked by:
Map: [Caminito v0.1](../map.md)

## What to build

The `org.mate.peripherals-*` keys turned into device and keyboard settings exactly as msd's mouse and keyboard plugins apply them on X11, and nothing more ([09](09-gsettings-mapping.md)). Pure translation and a seam for applying it; [56](56-libinput-on-the-tty.md) applies it to libinput devices.

- **`peripherals-mouse`**, non-touchpad pointers: `left-handed`; `motion-acceleration`, where -1 is the device default and otherwise `(m−1)×2/9−1` clamped to [−1, 1]; `accel-profile`; `middle-button-enabled` (middle emulation, non-touchpads only); `natural-scroll`.
- **`peripherals-touchpad`:** `touchpad-enabled` (send events); `left-handed` as `right`, `left` or `mouse`, which follows the mouse key; `motion-acceleration`, `accel-profile`, `natural-scroll`; `tap-to-click`; `disable-while-typing`; clickfinger when `two-finger-click` or `three-finger-click` is non-zero, else button areas; two-finger scrolling beats edge scrolling, and neither means none. The `horizontal-*-scrolling` keys drop that device's horizontal axis events in Caminito.
- **`peripherals-keyboard`:** `repeat`, `rate` and `delay` to the keyboard's repeat info, with rate 0 for `repeat=false` or a rate of 0 or less, and a delay of at least 1. `remember-numlock-state` and `numlock-state`: NumLock restored at start-up and written back on each change. These apply on every backend, nested too.
- **Never (09):** `motion-threshold`, `tap-button-*`, `bell-mode`, `click*`, `bell-pitch`, `bell-duration`. Unsupported, documented, not logged.
- **Not in M4:** `locate-pointer`, which Caminito draws itself as a ripple, goes to M8's parity audit (decided 2026-09-30).
- **Seam:** a `DeviceSettings` value per device kind, applied through a trait that libinput's `Device` implements and a test double records, so the translation and its re-application are tested without hardware.
- **Test schemas:** the touchpad, keyboard and keyboard-xkb schemas vendored beside the others.

## Acceptance criteria

- [x] Translation unit tests for every key above, written before the code, including the acceleration formula's ends and clamping, `left-handed=mouse`, and the scrolling precedence.
- [x] Headless tests: changing `rate` and `delay` live changes the repeat info a test client receives; NumLock's state is restored at start-up and written back.
- [x] Tests with the recording device double: a settings change reaches every device of its kind, and a device added later gets the current settings.
