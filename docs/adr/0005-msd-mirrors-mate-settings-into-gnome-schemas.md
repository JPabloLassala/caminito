# 5. msd mirrors MATE's appearance settings into the GNOME schemas GTK reads

Date: 2026-09-28
Status: Accepted

## Context

On X11, native GTK apps get the theme, fonts, cursor, double-click time and CSD button layout through XSETTINGS, which mate-settings-daemon's xsettings plugin publishes from `org.mate.*`. On its Wayland backend, GTK3 doesn't read XSETTINGS at all. It reads the GNOME schemas: `org.gnome.desktop.interface`, `.peripherals.mouse`, `.wm.preferences` and `.sound` (`gdkscreen-wayland.c:525-565`), or the xdg-desktop-portal Settings interface, but only when sandboxed or with `GTK_USE_PORTAL=1`. It never reads an `org.mate.*` key. msd's xsettings plugin still runs under Wayland, but only for XWayland.

Nothing in MATE bridges this gap. The Wayfire-based MATE session ships a static `:MATE` gschema override for a few defaults and tells users to set both sets of keys by hand. So under Camino, native GTK apps would ignore every appearance setting the user makes in mate-control-center.

## Decision

- **A carried msd patch** in the xsettings plugin, active only when `MATE_WM_NAME=Camino`, mirrors MATE's settings **one way, live, into the GNOME keys** GTK reads, reusing the plugin's existing translation table.
- **What's covered:**
  - theme, icons, fonts and font rendering;
  - cursor;
  - double-click and drag threshold;
  - the CSD button layout and titlebar click actions;
  - event sounds.
- **Writes** happen only when a value differs. MATE is the source of truth, and edits made on the GNOME side are overwritten.

## Consequences

- Native GTK3 and GTK4 apps follow mate-control-center, as X11 apps do.
- The mirror writes the user's real `org.gnome.*` keys, not defaults. A user who also logs into GNOME finds MATE's theme, fonts and button layout there. We accept this for v0.1.
- Rejected:
  - a static `:MATE` gschema override, like mate-wayland-session's (it only sets defaults, so user changes aren't followed);
  - Camino writing the keys itself (settings translation is msd's job, and msd already owns the translation table);
  - setting `GTK_USE_PORTAL=1` session-wide with a Camino portal Settings backend that translates `org.mate.*` (it also reroutes GTK's file chooser and other portals, changes behaviour for every GTK app, and is much more work);
  - doing nothing, as mate-wayland-session does.
