# Night Light's constant schedule on the tty

Epic: night-light
Type: implementation
Status: open: the code is in, and the check on the tty is the developer's
Milestone: none: after v0.1
Blocked by:
Map: [Night Light](../night-light/map.md)

## What to build

The first slice of [the spec](../night-light/spec.md): Night Light on, at the constant temperature, on every output of the tty backend. It writes the gamma tables of [ADR 0023](../../docs/adr/0023-caminito-owns-night-light-and-serves-wlr-gamma-control.md). The spec's "Settings", "Caminito: the gamma tables" and "What the user sees, and when" sections have it in full. niri's `src/backend/tty.rs` (`GammaProps`, `set_gamma`, `restore_gamma`) is the reference.

- **The keys.** All seven of the spec's keys go into `org.mate.Caminito`, with their descriptions. The settings thread translates them into typed settings. Only `night-light-enabled`, `night-light-schedule` and `night-light-constant-temperature` act in this slice. The sun schedule stays neutral until 138.
- **A temperature becomes a gamma ramp** through a blackbody white point computed from a formula, as wlsunset (MIT) does. Nothing comes from redshift's or gammastep's tables, which are GPL-3.0.
- **The tty backend sets each CRTC's table:**
  - by `GAMMA_LUT` as a property blob, set outside the frame commits through `DrmOutputManager::device()`, with the old blob destroyed;
  - by the legacy call, with an explicit linear ramp to reset, where there's no `GAMMA_LUT`;
  - by neither, logged once per output, where the CRTC has no gamma.
- **When the table is reset or applied again:**
  - It's reset when the device is taken.
  - It's applied again after a session resume, a new CRTC, a modeset and DPMS on.
  - It's never set while the session is paused: the change is kept for the resume.
- **Fades.** About 2 s from the output's current table to its target, at every change, in steps on a timer. A change during a fade starts from where the fade is. At session start the table is applied at once.
- **The nested run and headless** keep the state and set no table.
- **On `org.mate.Caminito`:**
  - the `NightLight` property, with `temperature`, plus `location-source` and `next-sunrise` as `none` and 0 until 138;
  - `ToggleNightLight()`, which flips `night-light-enabled`.
- **`DumpState`** gets Night Light's section.

## Acceptance criteria

- [x] The schema has the seven keys with the spec's types, defaults and ranges, and `check_schemas` knows them.
- [x] Unit tests: 6500 K gives an identity ramp; lower temperatures dim blue more than green, and green more than red; the ramp is monotonic for every size a CRTC reports.
- [x] Unit tests of the fade: it reaches the target in about 2 s, a change mid-fade starts from where it is, and session start doesn't fade. No test sleeps.
- [x] A headless test: setting the keys changes `DumpState`'s target temperature and the `NightLight` property, and `ToggleNightLight()` flips the key.
- [ ] On the tty, by the developer: with the constant schedule at 4000 K the screen warms with a fade, turning it off fades back, and a screenshot taken while it's on is true to the colours. The temperature is still there after a VT switch away and back, after DPMS off and on, and after a monitor is plugged in. Night Light isn't reset by starting Caminito after another compositor has left a table.

## Notes

- The gamma table is per CRTC, not per output. An output that moves to another CRTC is a new CRTC.
- Setting `GAMMA_LUT` by `set_property` can wait up to about a vblank on an atomic driver. Do it off the render path if it shows.

## Done (2026-10-09)

- **Tested first:** the ramp and the fade as pure modules (`night_light::ramp`, `night_light::state`, with `now` as an argument), then the keys' translation, then headless tests of the dump, `NightLight`, its signal and `ToggleNightLight()`. The fade's headless test moves Caminito's clock with `advance_clock`, so nothing sleeps.
- **The schema's keys:** a headless test reads the compiled `org.mate.Caminito` and checks each key's type, default and range. `check_schemas` itself checks schemas, not keys; `org.mate.Caminito` was already in `SCHEMAS`, so its new keys are read with it. `Value` gained `Uint`, `Int64` and `MaybePair` for `u`, `x` and `m(dd)`.
- **The white point:** wlsunset's formulas, taken relative to 6500 K's white point, so 6500 K is exactly the identity ramp and temperatures near it don't jump to it. The sRGB transfer is the standard's. Kim et al.'s Planckian fit holds only from 1667 K, so 1000 to 1667 K all get 1667 K's white point. Whether that's warm enough at the bottom of the slider is for 142 or the developer.
- **The fade:** 2 s, in mireds, per output, from where the output is at the moment of the change. A 50 ms loop timer runs only while a fade does. The refresh pass hands each output's temperature to the backend, and the tty writes a table only when the temperature changed. An output that is shown, at the Session's start or plugged in later, gets the target at once. That includes one plugged in during a fade.
- **The tty:** every CRTC is read and reset to neutral when the GPU is opened. `GAMMA_LUT` goes through a blob made with `drm-ffi`, because drm's `create_property_blob` can't take a slice; neutral sets blob 0. The legacy call writes a linear ramp. A CRTC with neither is warned about once. A table is written again after a resume, and after the first frame shown following a modeset or lighting a CRTC, since the mode is committed with that frame. Nothing is written while the session is paused. A failed write warns once, and isn't retried until the temperature changes.
- **DPMS on:** Caminito has no DPMS yet; idle draws a black blank, and powering outputs off is 129's. Re-applying the table after DPMS on belongs with that work.
- **The dump's per-output temperature** is Night Light's, not read back from the CRTC: it doesn't say whether a table was written. `gamma_client` per output is 141's to add.
- **Not run:**
  - The by-hand check on the tty: the screen warming at 4000 K, the fades, the screenshot's colours, a VT switch, DPMS, a hotplug, and a table left by another compositor. That's the developer's.
  - `cargo deny`.
  - The writes run on the refresh pass. If `GAMMA_LUT` stalls for a vblank during a fade on the tty, move them off the render path, as the ticket's note says.
- **The suite here:** run on Ubuntu, not CI's Arch. 236 X11, Xwayland and keymap tests fail on the parent commit just as they fail with this change, so they come from this container. Every other test passes. Clippy is clean on `caminito` and `caminito-test-client`. Clippy 1.97 finds three `manual_range_contains` errors in `caminito-theme`, which this change doesn't touch.
- **Citations:** niri's and wlsunset's functions are cited by name, not by line: their sources weren't at hand to check line numbers against.
