# msd mirrors the colour scheme from the GTK theme

Epic: caminito-v0.1
Type: implementation
Status: resolved
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

- [x] With `org.mate.interface gtk-theme` set to `BlackMATE`, `org.gnome.desktop.interface color-scheme` is `prefer-dark` and the portal's `ReadOne org.freedesktop.appearance color-scheme` gives `1`.
- [x] With it set to `Menta`, the key is `default` and the portal gives `0`.
- [x] Both follow a theme change within a second, with msd left running.
- [x] After `gsettings set org.gnome.desktop.interface color-scheme prefer-light` and a theme change, the key is the derived value again.
- [x] Outside Caminito (`MATE_WM_NAME` not `Caminito`), msd doesn't write the key.
- [x] `.scratch/issues/180-checks/` holds a script that runs the first four checks, and [checklist.md](../../docs/testing/checklist.md) has a by-eye entry: with a dark theme, Firefox's `prefers-color-scheme` is dark.
- [x] The `pkgrel` of `mate-settings-daemon-caminito` is bumped.

## For the implementing agent to settle

Recorded in the Done section:

- how msd reads the theme's two colours (its own `GtkStyleContext` once the theme has loaded, or a `GtkCssProvider` for the named theme), and how it knows the new theme has loaded before it measures;
- what it writes for a theme that doesn't exist.

## Not Caminito's: what Plasma leaves in `~/.config/gtk-*`

The report that started this was "Breeze-Dark looks like Breeze", and that part has another cause.

A Plasma login writes `gtk.css`, `colors.css` and `window_decorations.css` into `~/.config/gtk-3.0` and `~/.config/gtk-4.0`, from KDE's own colours. GTK loads them over any theme, and they redefine the colour names Breeze-Dark is built on (`theme_bg_color_breeze` and the rest). With KDE on a light colour scheme, Breeze-Dark's window background reads `rgb(239,240,241)` with those files and `rgb(32,35,38)` without them. BlackMATE and the MATE themes don't use those names.

The same would happen under MATE on X11, so it's no parity gap, and Caminito doesn't work around it: they're the user's files, and overriding the GTK config directory for every app is a bigger decision. A user who also logs into Plasma moves the three files away, and Plasma writes them again at its next login.

## Done (2026-10-11)

- **One row in the mirror's table,** from `org.mate.interface gtk-theme` to `org.gnome.desktop.interface color-scheme`, with `translate_color_scheme` as its translation. It reuses everything the other rows have: the `MATE_WM_NAME=Caminito` gate, the write only when the value differs, the run at start-up, and the run at every change of a mirrored MATE key. `-lm` is added to the plugin's `LIBADD`, for `pow`, as the mouse and media-keys plugins have it.
- **How msd reads the two colours: a `GtkCssProvider` for the named theme, not msd's own style context.** `gtk_css_provider_get_named` loads the theme's CSS, and `theme_is_dark` measures it on a `GtkStyleContext` of its own, for the path `window.background`. It compares the `background-color` with the `color` by WCAG relative luminance. The provider is added at `APPLICATION` priority, above the theme the screen has, which can still be the old one. So there's no waiting for msd to load the new theme: the answer is there before the change handler returns. The user's `gtk.css` (`USER` priority) stays above it, as in every app. So with Plasma's files in `~/.config/gtk-3.0` the measure is the colours the apps draw, which are wrong for the reason the ticket gives.
- **A theme that doesn't exist gives `default`.** GTK draws it as Adwaita, and `gtk_css_provider_get_named` falls back to Adwaita the same way, so it's measured as Adwaita, which is light. A theme with no `gtk-3.0` directory, such as HighContrastInverse, is the same case.
- **The hand-set value goes at any mirrored change, not only a theme change.** The row is one of the table's, and a change to any mirrored MATE key copies all of them. ADR 0005 says this of every mirrored key.
- **Not handled: a theme whose window background is transparent,** with its colour in a `background-image`. It would measure as black and give `prefer-dark`. None of the themes tried is like that.
- **Tested first:** a test program of the mirror alone (not in the repo), with GSettings' memory backend under Xvfb. It failed at its first check (`default` with BlackMATE) before the code was written. After the code, it passed for BlackMATE, Menta, Adwaita, TraditionalOk, a hand-set `prefer-light` followed by Menta, and `No-Such-Theme`. It also passed with `GTK_THEME=BlackMATE` and with `GTK_THEME=Menta` for the process, which shows that the named theme wins over the process's own.
- **Where it ran: Ubuntu 24.04 in a cloud container, not the Arch machine.** The series was applied with `patch` to the `v1.29.1` tag, as `prepare()` does. Only the daemon, the xsettings plugin and the schemas were built and installed, because the keyboard plugin needs mate-desktop 1.27 and Ubuntu has 1.26.2. [colour-scheme.sh](180-checks/colour-scheme.sh) ran against that msd, under a headless labwc with xdg-desktop-portal-gtk 1.15.1 and `XDG_CURRENT_DESKTOP=MATE`. With the variable set, it gave the key and the portal's `1` and `0` for BlackMATE and Menta. Each change landed in 17 to 73 ms, and a hand-set `prefer-light` went at the next theme change. With `stock`, the key kept the `prefer-light` it had before msd started. Each line says `ok` or `FAIL`, and the script exits 1 after a `FAIL`.
- **Not run:**
  - `makepkg` and `namcap` on the Arch machine. The patch's checksum was checked against the PKGBUILD.
  - The checklist's Firefox item, which needs a Session.
  - Dolphin and Kate, so what Qt apps do is still unknown.
  - `tools/check`, which needs the Arch packages `--deps` installs. Its Rust, meson and extension code didn't change.
