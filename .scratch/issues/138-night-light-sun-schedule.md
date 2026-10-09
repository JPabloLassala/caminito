# Night Light's sun schedule

Epic: night-light
Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: 137
Map: [Night Light](../night-light/map.md)

## What to build

The **sun schedule** ("Follow the sun"), under "The temperature" and "The location" in [the spec](../night-light/spec.md), with every location source but geoclue (139).

- **The sun's elevation** comes from the time and a latitude and longitude, computed in Caminito: NOAA's solar position equations or an equivalent, with no service and no crate beyond arithmetic. From it come:
  - each day's highest elevation;
  - the next time it climbs past −6°, for 140 and for `next-sunrise`.
- **The curve:**
  - Night temperature below −6°.
  - Noon temperature at the day's highest elevation.
  - Linear in elevation in between, interpolated in mireds.
  - At high latitudes: the night temperature all day if the sun never passes −6°; bottoming out with the sun under the midnight sun.
- **It's worked out again about once a minute,** and at once on a key change, a resume from sleep, or a jump of the clock. A small step is applied without a fade; a jump after a sleep fades.
- **Locations:** `night-light-last-location` when it's set; otherwise the time zone's coordinates (`/etc/localtime`'s target, its row in `zone.tab`, with backward links through `tzdata.zi`); otherwise fixed times. The fixed times put night before 07:00 and from 19:00, with a half-sine peaking at 13:00. Each fallback is logged once.
- **The `NightLight` property** gets `location-source` and `next-sunrise`. `DumpState` gets the location, its source, the sun's elevation, and the day's highest.

## Acceptance criteria

- [ ] Unit tests of the sun: elevations within 0.5° of NOAA's calculator for three places and dates (Montevideo at a solstice, Oslo at an equinox, Tromsø in December), and −6° crossings within 2 min.
- [ ] Unit tests of the curve: night temperature below −6°, noon temperature at the day's highest in June and December, the midpoint in mireds halfway up, Tromsø's polar night all night temperature, and Svalbard's midnight sun never reaching it.
- [ ] Unit tests of the time zone: `America/Montevideo` and `Europe/Amsterdam` give their `zone.tab` coordinates, `Asia/Calcutta` follows its link, `Etc/UTC` gives none, and the fixed times' curve is used then. They run on a test zoneinfo directory, not the host's.
- [ ] A headless test with a fake clock: `DumpState` shows the temperature moving through a day and the source falling back as the inputs are taken away. No test sleeps.
- [ ] On the tty, by the developer: with the sun schedule, the temperature at the time of the run is the one the curve gives for the developer's place.
