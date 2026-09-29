# XSETTINGS

Type: implementation
Status: open
Milestone: M3
Blocked by: 34
Map: [Camino v0.1](../map.md)

## What to build

Camino as the XSETTINGS manager ([ADR 0006](../../../docs/adr/0006-camino-serves-xsettings.md), [11](11-xwayland-integration.md)): `xwayland::xsettings` ([15](15-crate-and-module-architecture.md)), so X11 GTK apps get MATE's theme, fonts and DPI. The X11 scale settings are [41](41-x11-scale.md)'s.

- **The table (ADR 0006):** a port of msd's translation (`msd-xsettings-manager.c:205-240` and `:541-597`, at the msd revision 11 read): theme, icons, fonts and font rendering, DPI (with scale 1 for now), cursor, double-click and drag threshold, event sounds, and decoration layout. It's a pure function from Camino's config to the XSETTINGS table, unit-tested with no X. The msd revision it follows is recorded in the module, since the two tables must be kept in step (ADR 0006).
- **Settings (09):** the settings thread gains the keys the table reads that M2 didn't, such as `org.mate.font-rendering dpi` and the `org.mate.sound` keys, with the vendored test schemas updated.
- **Applied** through `X11Wm::set_xsettings` after every `start_wm`, before `DISPLAY` is exported ([38](38-x-connection-and-root-properties.md)'s start-up order), and live on every relevant settings change.
- **msd:** its xsettings plugin must run mirror-only under Camino (ADR 0006). That patch ships with `mate-settings-daemon-camino` in M5; nested runs have no msd, so nothing contends for `_XSETTINGS_S0` in M3.

## Acceptance criteria

- [ ] Unit tests of the table for each setting msd translates, written before the code, with the expected values taken from msd's code.
- [ ] X11-tier tests: the x11rb client reads `_XSETTINGS_SETTINGS` and finds the theme, fonts and DPI; changing `gtk-theme` with `gsettings` updates it live.
- [ ] Nested, gtk3-demo with `GDK_BACKEND=x11` uses the MATE GTK theme and font, and follows a `gtk-theme` change without restarting.
