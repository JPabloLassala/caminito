# The Wallpaper dialog, with pictures and colours

Epic: wallpaper
Type: implementation
Status: ready-for-agent
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

- [ ] The page state is toolkit-free, as caminito-displays's `page.rs` is, with unit tests: selecting an output, the switch on and off (turning it on copies the selected output's wallpaper to all), the grid's groups and order, and a KDE package's chosen picture.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item, in a Session with two monitors: a click on a picture is on the desktop at once; a different wallpaper per output; "Add folder…" adds a heading; the entry is in the Control Center beside Appearance; the dialog on one monitor only.
