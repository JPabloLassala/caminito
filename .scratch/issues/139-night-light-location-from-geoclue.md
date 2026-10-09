# Night Light's location from geoclue

Epic: night-light
Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: 138
Map: [Night Light](../night-light/map.md)

## What to build

The first location source under "The location" in [the spec](../night-light/spec.md).

- **A geoclue client over zbus** (geoclue's crates are stale or new, and the D-Bus is a handful of calls):
  - `Manager.GetClient`;
  - `DesktopId` set to `caminito`;
  - `RequestedAccuracyLevel` set to city (4);
  - `Start`;
  - then each `LocationUpdated` read for `Latitude` and `Longitude`.

  It runs off the main loop, as the settings thread does.
- **Failure is never waited on.** That covers no geoclue, no agent (about 5 s), denied, or offline. The next source is in use from the start, and a location that comes later replaces it with a fade.
- **Each location** is written to `night-light-last-location`, and becomes the source `geoclue`.
- **It starts only while Night Light is enabled.** The sun schedule and a pause both need the location.
- **Packaging:** `caminito-git` lists `geoclue: Night Light's location` as an optional dependency.
- **Find out on Arch** whether `geoclue` ships the demo agent and its autostart file, and whether mate-session starts it in a Session. Write the answer here and in the spec's further notes. If it doesn't, decide with the developer whether `caminito-desktop` should start it.

## Acceptance criteria

- [ ] A headless test against a fake geoclue on the private bus: a location is used and stored, a later one replaces it, and the source reads `geoclue`.
- [ ] A headless test: a geoclue that never answers or refuses leaves the stored location or the time zone in use from the first frame, and is logged once.
- [ ] The package's `optdepends` has `geoclue`.
- [ ] In a Session, by the developer: with geoclue and its agent running, the source is `geoclue` and the location is near the developer's; with geoclue stopped, the stored location is used.
- [ ] The Arch finding about the demo agent is written down.
