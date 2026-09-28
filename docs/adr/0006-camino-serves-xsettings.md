# 6. Camino serves XSETTINGS, and msd's xsettings plugin runs mirror-only

Date: 2026-09-28
Status: Accepted

## Context

On X11, mate-settings-daemon's xsettings plugin owns the `_XSETTINGS_S0` selection and publishes MATE's theme, fonts, DPI, cursor and scale to every GTK X11 client. Under Camino those clients run on XWayland: mate-session and its dialogs, the Mouse and Keyboard capplets, and third-party X11 apps.

Smithay's `X11Wm` claims `_XSETTINGS_S0` with an empty table on every `start_wm` (`xwm/settings.rs:105`) and ignores `SelectionClear`. msd's plugin sees an existing owner and aborts ("You can only run one xsettings manager at a time", `msd-xsettings-manager.c:921-926`). Since ADR 0005, the same plugin also carries the patch that mirrors MATE's settings into the GNOME keys native GTK reads, so the abort would take the mirror down too.

X11 apps can only be scaled by one global client scale, which has to agree with XSETTINGS `Xft/DPI` and `Gdk/WindowScalingFactor` (cosmic-comp does both in one place). msd's X connection also never reconnects after an Xwayland restart.

## Decision

- **Camino is the XSETTINGS manager**, through `X11Wm::set_xsettings`. It ports msd's translation table from `org.mate.*` (theme, icons, fonts and rendering, DPI, cursor, double-click and drag threshold, event sounds, decoration layout) and adds the X11 scale settings. It applies changes live and re-applies everything after every `start_wm`.
- **The carried msd xsettings patch from ADR 0005 is extended.** When `MATE_WM_NAME=Camino`, the plugin skips claiming `_XSETTINGS_S0` and starts only the GNOME-key mirror.

## Consequences

- X11 apps get MATE's appearance settings, and the X11 scale and its XSETTINGS values come from one place. XSETTINGS survives Xwayland restarts.
- MATE's settings are translated in two places: msd translates them to GNOME keys (ADR 0005), and Camino to XSETTINGS. ADR 0005 rejected Camino writing the GNOME keys because translation is msd's job; here the scale coupling and restart handling outweigh that. The two tables must be kept in step when msd's table changes.
- Under Marco, or with the patch missing, nothing changes: msd owns XSETTINGS as usual. A Camino session without the patch loses the GNOME mirror, which packaging must prevent.
- Rejected:
  - Camino's X connection stealing and releasing `_XSETTINGS_S0` right after `start_wm`, so msd can take it (msd doesn't reconnect after an Xwayland restart and doesn't know Camino's X11 scale);
  - patching Smithay to not claim XSETTINGS (03: pin master, don't fork);
  - leaving the table empty (X11 apps lose the MATE theme, fonts and DPI).
