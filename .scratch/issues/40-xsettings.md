# XSETTINGS

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M3
Blocked by: 34
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Caminito as the XSETTINGS manager ([ADR 0006](../../docs/adr/0006-caminito-serves-xsettings.md), [11](11-xwayland-integration.md)): `xwayland::xsettings` ([15](15-crate-and-module-architecture.md)), so X11 GTK apps get MATE's theme, fonts and DPI. The X11 scale settings are [41](41-x11-scale.md)'s.

- **The table (ADR 0006):** a port of msd's translation (`msd-xsettings-manager.c:205-240` and `:541-597`, at the msd revision 11 read): theme, icons, fonts and font rendering, DPI (with scale 1 for now), cursor, double-click and drag threshold, event sounds, and decoration layout. It's a pure function from Caminito's config to the XSETTINGS table, unit-tested with no X. The msd revision it follows is recorded in the module, since the two tables must be kept in step (ADR 0006).
- **Settings (09):** the settings thread gains the keys the table reads that M2 didn't, such as `org.mate.font-rendering dpi` and the `org.mate.sound` keys, with the vendored test schemas updated.
- **Applied** through `X11Wm::set_xsettings` after every `start_wm`, before `DISPLAY` is exported ([38](38-x-connection-and-root-properties.md)'s start-up order), and live on every relevant settings change.
- **msd:** its xsettings plugin must run mirror-only under Caminito (ADR 0006). That patch ships with `mate-settings-daemon-caminito` in M5; nested runs have no msd, so nothing contends for `_XSETTINGS_S0` in M3.

## Acceptance criteria

- [x] Unit tests of the table for each setting msd translates, written before the code, with the expected values taken from msd's code.
- [x] X11-tier tests: the x11rb client reads `_XSETTINGS_SETTINGS` and finds the theme, fonts and DPI; changing `gtk-theme` with `gsettings` updates it live.
- [x] Nested, gtk3-demo with `GDK_BACKEND=x11` uses the MATE GTK theme and font, and follows a `gtk-theme` change without restarting.

## Notes

- **The table** is all of msd's `translations[]` plus its Xft settings, not only the ticket's summary of them: the cited lines are the whole table. `xwayland::xsettings::table` is a pure function of `Settings`, with the msd revision and file lines in its module doc. `Net/FallbackIconTheme` is `mate`, as msd sets it at start-up and on every change. Left out: msd's `overrides` key (its own plugin schema), `Fontconfig/Timestamp` (its fontconfig monitor), the Xft resources it writes to `RESOURCE_MANAGER`, and the Qt variables it exports (13: never).
- **Where it differs from msd:** values come from Caminito's settings, so Caminito's rules for a key (09) apply first: an empty theme, icon theme or font takes its default, as do a double-click time or cursor size below 1 and a negative drag threshold. A `dpi` of 0 or less is 96 (13), where msd measures the screen for 0.
- **Settings:** `Interface` gained `GtkOptions`, the 22 `org.mate.interface` keys Caminito only passes on; `FontRendering` gained `dpi`; `org.mate.sound` is a new optional schema, `Sound`, which M6's bell will read too. The schema is vendored from mate-desktop 1.28.2.
- **Publishing** follows the refresh-pass rule: `RunningXwayland` keeps the table `X11Wm` publishes, and each pass sends only the settings that changed. `xwayland_ready` publishes it after the root properties, before `DISPLAY`, and clears what's kept when a new `X11Wm` attaches, since each `start_wm` claims XSETTINGS with an empty table ([42](42-xwayland-restart.md)'s restart relies on this).
- **Smithay doesn't flush** after `set_xsettings` (`xwm/mod.rs:1372-1386`), so the write waited in its buffer until X next sent it an event. 38's `DISPLAY` test, extended to read XSETTINGS, and the live `gtk-theme` test both caught it. Caminito follows it with an empty restack, `update_stacking_order_upwards` of no windows, which flushes and changes nothing (`:1110-1165`). A Smithay PR that flushes in `set_xsettings` would remove it.
- **The nested check** ran winit on Xvfb in a private session bus and `XDG_CONFIG_HOME`, with `font-name` set to `DejaVu Serif 12` and `dpi` to 120. A PyGObject probe with `GDK_BACKEND=x11` read `gtk-theme-name` Menta, that font, `gtk-xft-dpi` 122880 and icon theme menta from Caminito, and saw `notify::gtk-theme-name` fire with BlackMATE when `gsettings set org.mate.interface gtk-theme BlackMATE` ran. gtk3-demo's application demo drew in Menta with the serif font, and in BlackMATE after the change, without a restart.
