# Night Light's constant schedule on the tty

Epic: night-light
Type: implementation
Status: open
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

- [ ] The schema has the seven keys with the spec's types, defaults and ranges, and `check_schemas` knows them.
- [ ] Unit tests: 6500 K gives an identity ramp; lower temperatures dim blue more than green, and green more than red; the ramp is monotonic for every size a CRTC reports.
- [ ] Unit tests of the fade: it reaches the target in about 2 s, a change mid-fade starts from where it is, and session start doesn't fade. No test sleeps.
- [ ] A headless test: setting the keys changes `DumpState`'s target temperature and the `NightLight` property, and `ToggleNightLight()` flips the key.
- [ ] On the tty, by the developer: with the constant schedule at 4000 K the screen warms with a fade, turning it off fades back, and a screenshot taken while it's on is true to the colours. The temperature is still there after a VT switch away and back, after DPMS off and on, and after a monitor is plugged in. Night Light isn't reset by starting Caminito after another compositor has left a table.

## Notes

- The gamma table is per CRTC, not per output. An output that moves to another CRTC is a new CRTC.
- Setting `GAMMA_LUT` by `set_property` can wait up to about a vblank on an atomic driver. Do it off the render path if it shows.
