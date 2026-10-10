# Night Light's sun schedule

Epic: night-light
Type: implementation
Status: open: the code is in, and the check on the tty is the developer's
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

- [x] Unit tests of the sun: elevations within 0.5° of NOAA's calculator for three places and dates (Montevideo at a solstice, Oslo at an equinox, Tromsø in December), and −6° crossings within 2 min.
- [x] Unit tests of the curve: night temperature below −6°, noon temperature at the day's highest in June and December, the midpoint in mireds halfway up, Tromsø's polar night all night temperature, and Svalbard's midnight sun never reaching it.
- [x] Unit tests of the time zone: `America/Montevideo` and `Europe/Amsterdam` give their `zone.tab` coordinates, `Asia/Calcutta` follows its link, `Etc/UTC` gives none, and the fixed times' curve is used then. They run on a test zoneinfo directory, not the host's.
- [x] A headless test with a fake clock: `DumpState` shows the temperature moving through a day and the source falling back as the inputs are taken away. No test sleeps.
- [ ] On the tty, by the developer: with the sun schedule, the temperature at the time of the run is the one the curve gives for the developer's place.

## Done (2026-10-10)

- **Tested first:** the sun (`night_light::sun`), the curve (`night_light::curve`) and the time zone (`night_light::zone`) as pure modules with the time as an argument, then the state's sun steps, then headless tests on a fake wall clock. The seams were picked here, with no one to confirm them: those three modules' functions and the dump and `NightLight` through `Fixture`.
- **The reference values aren't NOAA's calculator's.** gml.noaa.gov wasn't reachable from the container, so the expected elevations and −6° crossings are PyEphem 4.2's (VSOP87, no refraction), which agree with NOAA's to about a hundredth of a degree away from the horizon. The code is NOAA's spreadsheet equations, without the refraction correction, since civil twilight's −6° is geometric. The tests allow 0.5° and 2 min, as the ticket asks.
- **Tromsø in December does pass −6°.** Its sun climbs to about −3° at noon at the solstice, so under the spec ("every day reaches it, short winter ones too") its polar night is the night temperature but for a twilight noon at the noon temperature, and the test says so. The day the sun never passes −6°, night all day, is Longyearbyen's in December (about −11.5°). If the developer wants a polar night to stay at the night temperature even with civil twilight, that's a change to the spec.
- **The day's highest** is the elevation at that day's solar noon, the day running from one solar midnight to the next at the location's longitude. So just before solar midnight the highest is the day before's; it only matters to the curve while the sun is up.
- **`next-sunrise`** is the next time the sun climbs past −6°, found by walking ten minutes at a time for two days and bisecting to the second. It's 0 when there's none in two days (a long polar night, or the midnight sun), and the fixed times' next 07:00 local with no location. 140 caps the pause at 24 h on top of it.
- **The fixed times** take local time from the zone GLib reads at the `localtime` path (`glib::TimeZone::new`), and UTC when it can't, so a zone missing from `zone.tab` still gets its own 07:00 and 19:00. No time zone crate was added.
- **When it's worked out:** a 60 s loop timer, at start before any output is shown (so the first table is applied at once), on every Night Light key change (which fades), on logind's `PrepareForSleep(false)`, and whenever the wall clock has moved more than 60 s from Caminito's monotonic clock since the last work-out, which catches a sleep logind didn't report and a change of the time. A minute's step is taken at once; a resume or a jump fades over the usual 2 s.
- **The time zone** is read again at each work-out (one `readlink`), and each zone's `zone.tab` row is looked up once and cached. `zone.tab`, then a `L` line of `tzdata.zi`; `zone1970.tab` isn't read. Each change of source is logged once: `info!` for stored and time zone, `warn!` for none.
- **The tests' clocks:** the tests' wall clock starts at 2026-10-10 12:00 UTC and runs with Caminito's (and `advance_clock`), so the dump's snapshots stay the same; `Fixture::set_wall_clock` moves it, as a jump. The time-zone tests run on `tests/zoneinfo/`, excerpts of tzdata 2026c's `zone.tab` and `tzdata.zi`, with `localtime` links made in a temporary directory; a Fixture's default `localtime` doesn't exist, so no test reads the host's.
- **Fixed on the way:** 137's `the_property_is_signalled_when_the_target_changes` failed under the full suite's load: a `NightLight` signal reads the property when it's sent, so two changes back to back were both heard as the second. It now waits to hear each.
- **Not run:**
  - the check on the tty, the developer's: that the temperature at the time of the run is the curve's for their place;
  - `cargo deny`.
- **The suite here:** run on Ubuntu, not CI's Arch. Four keymap tests and `the_pointer_goes_through_an_override_redirect_windows_input_shaped_shadow` fail here on trunk too; every other test passes. Clippy is clean on `caminito`; Clippy 1.97's three `manual_range_contains` errors in `caminito-theme` are still there, untouched.
