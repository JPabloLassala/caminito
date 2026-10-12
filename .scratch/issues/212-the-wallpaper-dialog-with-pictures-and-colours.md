# The Wallpaper dialog, with pictures and colours

Epic: wallpaper
Type: implementation
Status: resolved
Blocked by: 210
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

`caminito-wallpaper`, the dialog, under "The dialog" and "The Control Center" in [the spec](../wallpaper/spec.md). GTK4 without libadwaita, written afresh: the prototype on the branch `prototype/wallpaper` (`tools/prototype-wallpaper.py`, variant B) is the picture of the layout and nothing more.

- **The sidebar:** a small picture of each output showing its wallpaper, in the layout's left-to-right order, all the same width, with the monitor's name; "Same on all outputs"; the five kinds as a list with icons. With one output, its picture is a plain preview and the switch is hidden.
- **The kinds this ticket builds** are Picture and Colour. The other three are in the list and show a line saying they aren't there yet, until their tickets fill them.
- **Picture:** one scrolling grid of thumbnails with a heading per source, in this order: the current picture, the pictures and folders the user added, MATE's (`/usr/share/backgrounds`), Plasma's (`/usr/share/wallpapers`, one picture per package, the largest landscape one). "Add picture…", "Add folder…" and Positioning under it. Thumbnails are decoded off the main loop.
- **Colour:** a few swatches and a colour chooser.
- **Everything applies on click,** by writing the store. No Apply button.
- **`caminito-wallpaper.desktop`:** "Wallpaper", in the Control Center category Appearance is in, installed where the other Caminito entries are.
- Escape closes it, and starting it again raises the open window, as caminito-about does.
- `--demo` shows it with made-up outputs and no store, as caminito-displays has.
- `caminito-git`'s PKGBUILD installs it.

## Acceptance criteria

- [x] The page state is toolkit-free, as caminito-displays's `page.rs` is, with unit tests: selecting an output, the switch on and off (turning it on copies the selected output's wallpaper to all), the grid's groups and order, and a KDE package's chosen picture.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item, in a Session with two monitors: a click on a picture is on the desktop at once; a different wallpaper per output; "Add folder…" adds a heading; the entry is in the Control Center beside Appearance; the dialog on one monitor only.

## Done (2026-10-12)

- **The tests came first, at three seams.** `page/tests.rs` drives the page against the store in memory, which moved from the settings' tests to `settings::Memory` so `--demo` uses it too: the left-to-right order, selecting an output, a click with the switch on and off, the switch turned on (the selected output's wallpaper copied to `all/`) and off, and the switch hidden with one output. `grid.rs` tests the groups and their order on a temporary disk, and a KDE package's picture. `caminito/src/tests/caminito_wallpaper.rs` runs the dialog's client of output management and its page against Caminito headless, through a hot-unplug and a plug.
- **The page is in the library, the window in the binary.** `page.rs`, `grid.rs` and `monitors.rs` (output management, as the drawer reads it, with the place, the mode's size and the transform) name no GTK. The binary is `src/bin/caminito-wallpaper/`; the crate depends on GTK for it alone, and `ldd caminito-wallpaperd` still shows none, since a binary links only the crates it uses. `Store` gained the switch's setter and the lists of added pictures and folders.
- **Turning the switch off writes `all/` to every connected output's own set,** so nothing on the desktop changes until the next click. The ticket names only turning it on.
- **A kind clicked in the list applies at once** when there's something to show: Colour always, Picture once a picture is set. The three later kinds only show their line. This is "everything applies on click" read literally; browsing to Colour does change the desktop.
- **Opening the dialog gives each output its settings as the drawer does** (`settings_for`), so a monitor the drawer hasn't seen yet shows what it will. That writes the store on open, as the drawer would.
- **The grid.** MATE's directory is read three levels deep, and an added folder at its own level alone, so adding `~/Pictures` doesn't pull in every folder under it. A Plasma package shows its largest landscape picture by its file name's size, or its largest, or its first. Pictures loose in `/usr/share/wallpapers` are offered too. MATE's and Plasma's are scanned once as the window opens; the added folders are read again at each render, which is one directory each.
- **Thumbnails and the sidebar's pictures are decoded on GLib's pool,** four at a time, EXIF-turned, and handed back as bytes, since a `Pixbuf` isn't `Send`. The sidebar draws each output's wallpaper with the drawer's `place`, so the positioning shows there too. Every output's picture is 200 pixels wide; one on its side is taller.
- **The entry's categories are Appearance's,** `GTK;Settings;DesktopSettings;`, from mate-control-center's `mate-appearance-properties.desktop`, which `matecc.menu` files under *Look and Feel*. Meson installs it to `share/caminito/xdg/applications`, and `desktop-file-validate` passes.
- **Run in an Ubuntu 24.04 cloud container** (GTK 4.14), with the build's dependencies and Xwayland from apt. The workspace's tests pass but `caminito-theme`'s `goldens`, which fails the same way on the base here (the Yaru buttons a few pixels off, from this container's libraries); caminito's 1834 pass, and so do the ignored drawer tests. The dialog was seen working in `--demo` and against a keyfile GSettings under Xvfb, with clicks by `xdotool`: a picture, a colour, Tile and the switch, each written to the store. It wasn't run on Wayland, nor in a Session with two monitors, nor with *Add picture…* and *Add folder…*'s file choosers: that's the checklist item and 220's. `tools/check`'s clippy step fails on the base's three `manual_range_contains` in `caminito-theme`, as 210 found; the crates this ticket touches are clippy-clean. No PKGBUILD was run.
