# Caminito publishes the sun's position

Epic: wallpaper
Type: implementation
Status: resolved
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

"The sun" in [the spec](../wallpaper/spec.md): a dynamic wallpaper follows the same sun as Night Light's sun schedule, and caminito-wallpaper does no sun sums of its own.

- **Caminito keeps the location current whether or not Night Light is on,** from the same sources in the same order ([the Night Light spec](../night-light/spec.md)). It stays the only geoclue client.
- **It publishes the sun's elevation and azimuth,** and the location's source, on its D-Bus interface, with a change signal often enough for a wallpaper to follow dusk smoothly.
- **With no location** it publishes the sun of Night Light's fixed times, and says so in the source.
- `DumpState` shows them.

## Acceptance criteria

- [x] Unit tests of the azimuth against known values for a place and time, beside the elevation's.
- [x] A headless test reads the properties with Night Light off, and sees them change when the test clock moves.
- [x] Night Light's own tests pass unchanged.

## Done (2026-10-12)

- **The azimuth was tested first,** in `night_light/sun.rs` beside the elevation's tests, at the same eight places and times, against PyEphem 4.2.1 with no refraction, to half a degree. It's NOAA's hour angle and declination, as the elevation is, through `atan2`, clockwise from north, 0 to 360.
- **The property is `Sun` on `org.mate.Caminito`,** an `a{sv}` of `elevation` (`d`), `azimuth` (`d`) and `location-source` (`s`, as `NightLight`'s). It's worked out in Night Light's own minute's working-out of the sun, which already ran whether or not Night Light is on, and is signalled when it changes: each minute, when the sun has moved a quarter of a degree at most. Night Light's `NightLight` property and its dump are as they were.
- **With no location the sun is the fixed times',** `curve::fixed_times_position`: an elevation that is a sine about −6°, crossing it at 07:00 and 19:00 and peaking at 45° at 13:00, so that the sun schedule's curve of it is exactly the fixed times' temperature (a unit test checks every half hour), and an azimuth going round at 15° an hour, east at 07:00 and south at 13:00, as north of the tropics. The 45°, the night's −57° and the northern sky are chosen, not given by the spec: with no location a southern user's fixed sun is in the wrong half of the sky, which is the developer's to change if it matters.
- **`DumpState` has a top-level `sun`** with the elevation, azimuth and location source. The debug snapshot shows the two numbers as `[moves]`, since the test wall clock moves them by however long the start took, and its helper is renamed `without_what_varies`.
- **Geoclue isn't here yet** (139), so "the same sources in the same order" is stored, time zone, none. 139's bullet that geoclue starts only while Night Light is enabled is changed to run whether or not it is, for this ticket.
- **The full suites passed** under `dbus-run-session`: caminito's 1840 and the workspace's but `caminito-theme`'s goldens, whose Yaru renders differ by a few pixels under this Ubuntu container's cairo and pango, from a crate this ticket doesn't touch. The Xwayland tier needed `/tmp/.X11-unix` made first.
- **Not run:** `tools/check`'s meson, locker and extension steps, which touch nothing changed here, and clippy without `-A clippy::manual_range_contains`, which this container's clippy 1.97 raises on three lines of `caminito-theme` that were there before.

