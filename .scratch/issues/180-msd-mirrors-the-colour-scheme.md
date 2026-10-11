# msd mirrors the colour scheme from the GTK theme

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

With a dark GTK theme chosen in MATE's Appearance preferences, apps that follow the **colour scheme** stay light: Firefox and Chromium page content, VS Code and other Electron apps on "auto", libadwaita and GTK4 apps.

They read the colour scheme from the portal (`org.freedesktop.appearance color-scheme`). xdg-desktop-portal-gtk serves it from `org.gnome.desktop.interface color-scheme`, and nothing in a Caminito session writes that key. The msd mirror ([ADR 0005](../../docs/adr/0005-msd-mirrors-mate-settings-into-gnome-schemas.md)) covers the theme, icons, fonts, cursor, clicks, button layout and sounds, and `org.mate.interface` has no light/dark key to mirror from.

This was decided in a grilling session on 2026-10-11, and ADR 0005 was amended then.

## What to build

- **The mirror writes `org.gnome.desktop.interface color-scheme`,** in the carried patch `0001-xsettings-Mirror-to-the-GNOME-settings-only-under-Ca.patch` of `packaging/arch/mate-settings-daemon-caminito`, under the same `MATE_WM_NAME=Caminito` gate as the rest.
- **The value is derived from the GTK theme:** `prefer-dark` when the theme's window background is darker than its text colour, and `default` otherwise. Never `prefer-light`.
- **It follows the theme live.** A change of `org.mate.interface gtk-theme` updates it with no app restarted, and it's set once at start.
- **It's written only when it differs,** like the other mirrored keys. A value set by hand is overwritten on the next theme change.

Left alone:

- `gtk-application-prefer-dark-theme`. The theme chosen in MATE is already the dark one, and the flag would change which of the palette helper's variants ([28](28-palette-helper.md)) matches the windows.
- Qt and KDE apps. With no platform theme loaded, Qt6 apps take light or dark from the portal, so they're likely to follow as a side effect. The Done section says what Dolphin or Kate did; it isn't a criterion.

## Acceptance criteria

- [ ] With `org.mate.interface gtk-theme` set to `BlackMATE`, `org.gnome.desktop.interface color-scheme` is `prefer-dark` and the portal's `ReadOne org.freedesktop.appearance color-scheme` gives `1`.
- [ ] With it set to `Menta`, the key is `default` and the portal gives `0`.
- [ ] Both follow a theme change within a second, with msd left running.
- [ ] After `gsettings set org.gnome.desktop.interface color-scheme prefer-light` and a theme change, the key is the derived value again.
- [ ] Outside Caminito (`MATE_WM_NAME` not `Caminito`), msd doesn't write the key.
- [ ] `.scratch/issues/180-checks/` holds a script that runs the first four checks, and [checklist.md](../../docs/testing/checklist.md) has a by-eye entry: with a dark theme, Firefox's `prefers-color-scheme` is dark.
- [ ] The `pkgrel` of `mate-settings-daemon-caminito` is bumped.

## For the implementing agent to settle

Recorded in the Done section:

- how msd reads the theme's two colours (its own `GtkStyleContext` once the theme has loaded, or a `GtkCssProvider` for the named theme), and how it knows the new theme has loaded before it measures;
- what it writes for a theme that doesn't exist.

## Not Caminito's: what Plasma leaves in `~/.config/gtk-*`

The report that started this was "Breeze-Dark looks like Breeze", and that part has another cause.

A Plasma login writes `gtk.css`, `colors.css` and `window_decorations.css` into `~/.config/gtk-3.0` and `~/.config/gtk-4.0`, from KDE's own colours. GTK loads them over any theme, and they redefine the colour names Breeze-Dark is built on (`theme_bg_color_breeze` and the rest). With KDE on a light colour scheme, Breeze-Dark's window background reads `rgb(239,240,241)` with those files and `rgb(32,35,38)` without them. BlackMATE and the MATE themes don't use those names.

The same would happen under MATE on X11, so it's no parity gap, and Caminito doesn't work around it: they're the user's files, and overriding the GTK config directory for every app is a bigger decision. A user who also logs into Plasma moves the three files away, and Plasma writes them again at its next login.
