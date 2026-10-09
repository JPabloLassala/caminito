# Map: Night Light

Status: closed on 2026-10-09. It reached its destination, [the spec](spec.md), in one grilling session with the developer.

## Destination

A spec, ready to cut into implementation tickets, for Night Light in Caminito: Plasma's, and a gradual schedule in place of macOS's True Tone, which needs a light sensor.

## Notes

- **Domain:** **Night Light**, **Schedule**, **Sun schedule**, **Constant schedule**, **Gamma client** and **Pause** are in [CONTEXT.md](../../CONTEXT.md). The ownership of the gamma tables is [ADR 0023](../../docs/adr/0023-caminito-owns-night-light-and-serves-wlr-gamma-control.md).
- **Not in a milestone.** It gates nothing of v0.1, and its tickets say `Milestone: none: after v0.1`.

## Decisions so far

From the grilling session of 2026-10-09:

- **One feature, not two.** The gradual change is Night Light's sun schedule, beside a constant schedule. Plasma's plateaus and custom times are left out.
- **The sun schedule follows the sun's elevation.** Night temperature below −6°, noon temperature at each day's highest, interpolated in mireds. Defaults: noon 5500 K, night 3400 K, constant 4000 K, each from 1000 to 6500 K.
- **Location:** geoclue through its demo agent, then the last location it gave, then the time zone's coordinates, then fixed sun times with a note. Caminito isn't geoclue's agent.
- **Caminito owns it and serves `wlr-gamma-control`** (ADR 0023). It sets the gamma tables, not a shader, so screenshots stay true and the nested run tints nothing. A gamma client wins its own output only, and Night Light fades back in when the client lets go. The protocol is privileged.
- **Same temperature on every output.**
- **Off and pause.**
  - `enabled` is flipped from the tab, D-Bus, and a user-bound shortcut, with none bound by default.
  - "Pause until sunrise" means the sun past −6° under either schedule, capped at 24 h. It's stored as an end time in a key, and is cancelled by flipping `enabled`. It's paused from the tab and D-Bus, not the shortcut.
  - There's no inhibit API.
- **Fades** of about 2 s on every change. Applied at once at session start. On the lock screen too. Applied again after a modeset, DPMS on, or a session resume.
- **UI:** a Night Light tab in caminito-displays, and a `caminito-night-light` menu entry that opens it.
- **The epic is `night-light`**, tickets 137 to 143.

## Not yet specified

Nothing. What still has to be seen is in the spec's further notes and ticket 143.
