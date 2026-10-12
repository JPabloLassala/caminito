# Dynamic wallpapers from KDE's pairs and timed XML

Epic: wallpaper
Type: implementation
Status: ready-for-agent
Blocked by: 210, 212, 215
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

The Dynamic kind, under "The kinds" and "The sun" in [the spec](../wallpaper/spec.md), for two of its three formats.

- **KDE's packages with `images` and `images_dark`:** the light picture by day and the dark by night, cross-fading through twilight by the sun's elevation Caminito publishes ([215](215-caminito-publishes-the-suns-position.md)).
- **GNOME's timed XML:** its pictures at its clock times, with its transitions.
- **The dialog:** a grid of the dynamic wallpapers found in `/usr/share/wallpapers` and `/usr/share/backgrounds` and those the user added, each thumbnail showing day and night halves; "Add…" for an `.xml`; a line saying whether it follows the sun or fixed times; and Positioning.
- Caminito ships no dynamic wallpaper of its own.

## Acceptance criteria

- [ ] Unit tests: the pair's blend for elevations through a day; a timed XML's picture and blend for times through its cycle, with the installed `cosmos` file as a fixture; a malformed XML is refused without a crash.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item: a Plasma pair turns dark as Night Light warms, and a timed XML is on its right picture for the hour.
