# Caminito publishes the sun's position

Epic: wallpaper
Type: implementation
Status: ready-for-agent
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

"The sun" in [the spec](../wallpaper/spec.md): a dynamic wallpaper follows the same sun as Night Light's sun schedule, and caminito-wallpaper does no sun sums of its own.

- **Caminito keeps the location current whether or not Night Light is on,** from the same sources in the same order ([the Night Light spec](../night-light/spec.md)). It stays the only geoclue client.
- **It publishes the sun's elevation and azimuth,** and the location's source, on its D-Bus interface, with a change signal often enough for a wallpaper to follow dusk smoothly.
- **With no location** it publishes the sun of Night Light's fixed times, and says so in the source.
- `DumpState` shows them.

## Acceptance criteria

- [ ] Unit tests of the azimuth against known values for a place and time, beside the elevation's.
- [ ] A headless test reads the properties with Night Light off, and sees them change when the test clock moves.
- [ ] Night Light's own tests pass unchanged.
