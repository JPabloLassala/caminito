# Appearance hides its Background tab under Caminito

Epic: wallpaper
Type: implementation
Status: open: the patch is in and carried by the PKGBUILD; the Arch package build and the checklist item are the developer's, and the Debian build is 199's
Blocked by: 212
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

"The Control Center" in [the spec](../wallpaper/spec.md): with mate-settings-daemon's background plugin off, the Background tab of MATE's Appearance capplet has no effect in a Session.

A carried patch to mate-control-center, kept in `packaging/patches/mate-control-center` (ADR 0030) and gated on Caminito as the others are, that leaves the Background tab out. Anything that opens Appearance at that tab opens the Wallpaper dialog instead, if a way to do that is found in the capplet; otherwise it opens Appearance's first tab.

## Acceptance criteria

- [ ] The patched packages build with the patch in their series, on Arch and on Debian.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item: in a Session Appearance has no Background tab, and caja's desktop menu "Change Desktop Background" opens something sensible; in the X11 session with Marco the tab is there and works (the gate).

## Done (2026-10-15)

- **0009, `appearance: Leave out the Background tab under Caminito`**, is gated on `MATE_WM_NAME` alone, as 0006 to 0008 are (ADR 0011). Under Caminito it hides `background_vbox` after `gtk_widget_show_all`, rather than removing the page: a GTK 3 notebook draws no tab for a hidden page and its wheel scrolling skips it, the page numbers Help's switch reads stay the same, and `desktop_init`/`desktop_shutdown` keep the widgets they expect.
- **The Wallpaper dialog was found to be openable from the capplet**, so the ticket's first choice is what's built: `--show-page=background`, which caja's "Change Desktop Background" passes (`fm-desktop-icon-view.c`), and a picture on the command line with no page, which stock opens at Background, launch `caminito-wallpaper.desktop` through `GDesktopAppInfo`, and Appearance exits. Only a Session has that launcher on its data path, so without it Appearance opens at its first tab, with the tab still hidden.
- **A picture on the command line is dropped under Caminito**: the dialog's `Exec=caminito-wallpaper` takes no file, and passing it to `desktop_init` would select it in the hidden list and write `org.mate.background` out of sight (found in the spec review).
- **What was run:** the series 0001 to 0009 applied with `patch -Np1` to v1.29.1, as `prepare()` does, and the patched `appearance-main.c` compiled with `-Wall -Wextra` against Ubuntu 24.04's GTK 3 in a configure tree whose version floors were lowered for the check alone, with an empty stand-in for `mate-image-menu-item.h` (mate-desktop 1.27's). No package was built: this container has no Arch, no podman and no Session. `tools/check`'s rustfmt passed and its clippy stopped on 24.04's missing graphene/GTK 4; the diff touches no file the suite covers.
- **Left:** the first criterion's Arch build (`makepkg` of `mate-control-center-caminito` 1.29.1-7) and a run of the new checklist item are the developer's. The Debian half waits on 199, which packages mate-control-center and copies the series from `packaging/patches/` (ADR 0030).
- **Not changed:** the Theme tab's "Apply Background" for a theme that names one still writes `org.mate.background`, which caminito-wallpaperd follows onto every output as it does caja's "Set as wallpaper".
