# mate-settings-daemon-caminito

Type: implementation
Status: resolved
Milestone: M5
Blocked by: 69
Map: [Caminito v0.1](../map.md)

## What to build

msd as a Patched package ([16](16-packaging.md)) at 1.29.1 or a later 1.29.x tag, the first with the wlrandr plugin (14), with the three Carried patches the session needs first ([17](17-milestone-slicing.md)). All three are active only when `MATE_WM_NAME=Caminito` ([ADR 0011](../../../docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-caminito.md)).

- **`packaging/arch/mate-settings-daemon-caminito/PKGBUILD`,** from Arch's mate-settings-daemon PKGBUILD with the smallest diff: the 1.29.x tag as source, `provides` and `conflicts` on `mate-settings-daemon`, the patches, and whatever the tag newly depends on.
- **The patches:**
  - `0001` **xsettings, mirror-only** ([ADR 0005](../../../docs/adr/0005-msd-mirrors-mate-settings-into-gnome-schemas.md), [ADR 0006](../../../docs/adr/0006-caminito-serves-xsettings.md), [09](09-gsettings-mapping.md)): under Caminito the plugin doesn't take `_XSETTINGS_S0`, which Caminito owns (40). It mirrors MATE's settings one way into the GNOME keys native GTK reads, live, with the plugin's existing translation table. What's mirrored is 09's list;
  - `0002` **background** (06): the plugin stays on under Caminito and draws the wallpaper as a `mate-background` layer surface on every output, under caja's `desktop` ([63](63-layer-shell.md));
  - `0003` **typing break** (13): the plugin doesn't start under Caminito.
- **Unpatched, and checked here:**
  - the wlrandr plugin applies `monitors-wayland.xml` over wlr-output-management (58) at login;
  - the clipboard plugin keeps the clipboard through data-control ([68](68-data-control-and-privileged-globals.md)). 11 inferred this; if the plugin is `X11Only` or absent on Wayland, say so in the notes and file a ticket, since M5's checklist item depends on it.
- **CI:** a line each in the Packaging job and the weekly patch-on-head job ([69](69-mate-session-manager-caminito.md)).

## Acceptance criteria

- [ ] `makepkg -si` installs it in place of `mate-settings-daemon`; `namcap` reports nothing unexplained; both CI jobs are green.
- [x] Nested with `MATE_WM_NAME=Caminito`, msd started by hand:
  - Caminito keeps `_XSETTINGS_S0`, and msd logs no XSETTINGS conflict;
  - `gsettings set org.mate.interface gtk-theme` changes `org.gnome.desktop.interface gtk-theme`, and a native GTK3 app restyles; the same for a font and the cursor theme;
  - the wallpaper shows, and changing `org.mate.background picture-filename` changes it;
  - the typing-break plugin isn't loaded.
- [x] Nested, a saved `monitors-wayland.xml` with the output at scale 1.5 is applied when msd starts.
- [x] Under X11 MATE, the patched msd owns `_XSETTINGS_S0`, draws the background as before, and writes nothing to the GNOME schemas.

## Notes

- **The rest of msd's series** (media-keys, keybindings, the OSD, touchpad) is M6 (17).
- **Which plugins are `X11Only`** at the tag decides how much `0002` has to change; read each `.mate-settings-plugin` file first.
- **The mirror's trade-off** (a user who also logs into GNOME gets MATE's choices there) is ADR 0005's, and isn't reopened here.
- **Resolved on 2026-10-05 with the first criterion unticked:** the package, the patches, the two CI lines and the README are done, and the package was built and installed in place of the stock one. **The CI part, and namcap on an x86_64 package,** need a push, as in [69](69-mate-session-manager-caminito.md) and [70](70-mate-panel-caminito.md). The other three criteria were seen with no desktop and no login, as below; [72](72-session-file-and-first-login.md) runs them again in a real session.
- **Baseline:** the `v1.29.1` tag. Upstream's head is two commits after it, and the series applies there (`git am`).
- **msd 1.29.1 doesn't build on Arch as it is.** It asks for mate-desktop 1.29.0, and Arch has 1.28.2. One upstream commit needs it (`13b2430`, "xrandr: Handle hotplug_mode_update for VM guest resizing", which calls `mate_rr_output_get_hotplug_mode_update`), in the `X11Only` xrandr plugin.
  - **`0004` takes that commit out,** but for its change to upstream's CI file, so the xrandr plugin is as in 1.28.0, the stock package. It isn't a carried patch and has no gate. ADR 0011 now says what such a build patch is and when it's allowed, and the README says the package has one.
  - **The other way** was a fourth patched package, `mate-desktop-caminito` at 1.29.0. That replaces a library every MATE component links, for one function Caminito doesn't use, so it wasn't done. If [73](73-packaging-and-closing-m5.md) wants msd unchanged on X11, that's the way.
  - `0004` goes when Arch's mate-desktop is 1.29. A revert is also the patch most likely to stop applying on upstream's head; the weekly job says when.
- **What the tag newly depends on,** with `--enable-wayland`: `gtk-layer-shell`, `wayland`, `libayatana-appindicator` (the wlrandr plugin's tray icon; the tag's configure turns Wayland off without it) and, to build, `wayland-protocols`. Without `--enable-wayland`, a missing one turns the wlrandr plugin and the Wayland background and clipboard off and configure says nothing.
- **Which plugins are `X11Only` at the tag:** a11y-keyboard, keybindings, keyboard, media-keys, mouse and xrandr. Background, clipboard, typing-break, xsettings and wlrandr aren't, and the tag has Wayland code for the background (gtk-layer-shell, namespace `mate-background`) and the clipboard already. So `0002` is 8 lines.
- **`0001`, the mirror,** is a new file, `plugins/xsettings/msd-gnome-mirror.c`, started from `mate_xsettings_manager_start` when `MATE_WM_NAME=Caminito`, before anything else of the manager. So under Caminito the plugin opens no X display, writes no X resources and doesn't export the Qt variables (13).
  - **It has its own table, not the plugin's `translations[]`.** That table names XSETTINGS, not GNOME keys, and has 16 of 09's 26 keys: the document and monospace fonts, `gtk-overlay-scrolling`, font rendering and the titlebar actions aren't in it. A second column there would touch every row of upstream's table, and each rebase would pay for it. The mirror's table is 09's list, row for row. Caminito's XSETTINGS table (40) and this one both follow 09.
  - **A change to one mirrored MATE key copies all of them,** so a change made on the GNOME side is overwritten at the next MATE change, as 09 says. Each key is written only when it differs.
  - **Where it's more careful than 09's list:** `text-scaling-factor` is kept in the GNOME key's range, 0.5 to 3.0. A schema or key that isn't installed is skipped with one warning, since `g_settings_new` aborts on one, and Marco's schema comes from another package.
- **`0002`, the background:** on Wayland, the tag makes no layer surface while caja's `show-desktop-icons` is on. Under Caminito that test is skipped, so every output has the wallpaper, and caja's `desktop` is drawn above it ([63](63-layer-shell.md)).
- **`0003`, typing break,** returns from the plugin's `activate` under Caminito. msd still opens the plugin's library; the manager, and `mate-typing-monitor`, never start. That is what "isn't loaded" is ticked on. Keeping the library closed needs a change to msd's plugin loader for one plugin.
- **The clipboard plugin loads on Wayland and keeps the clipboard,** as 11 inferred. It binds `ext_data_control_manager_v1` version 1 only, not the wlr one, so [68](68-data-control-and-privileged-globals.md) must offer the ext protocol. Under labwc, with `wl-copy` killed, `wl-paste` gives the text back: msd takes the selection 2 s after its owner goes. In the nested run it logs "The compositor does not support the ext-data-control protocol", because 68 isn't in. No ticket was filed.
- **The wlrandr plugin fails the whole layout when one part is refused.** Nested, the winit output's transform is `flipped-180` (58). A saved file with `normal` got "Could not apply the display configuration" and the scale stayed 1. With the transform msd itself would save, the layout is applied. On a tty the transform is `normal`.
- **For [72](72-session-file-and-first-login.md):** the xrdb plugin isn't `X11Only` and runs `xrdb -merge` at start and at each theme change. Whether that and Caminito's own Xft resources (47) get in each other's way wasn't looked at. The check machine has no `xrdb`.
- **Checked in the OrbStack machine** (Arch Linux ARM, aarch64, `makepkg -A`; it has no desktop and no login), with the scripts in [71-msd-checks](71-msd-checks/). Each uses a scratch dconf database and its own bus:
  - The package builds with the checksums on, and the patches apply with no offset. `pacman -U` replaces `mate-settings-daemon` 1.28.0-2, and `pacman -Q mate-settings-daemon` answers `mate-settings-daemon-caminito 1.29.1-1`. It stays installed there.
  - `namcap PKGBUILD` prints nothing. On the built package: an empty `etc/mate-settings-daemon/xrandr`, as upstream installs it, and namcap on Arch Linux ARM calling every linked library uninstalled, as in 69.
  - **Nested, Caminito on Xvfb** ([caminito.sh](71-msd-checks/caminito.sh)): `_XSETTINGS_S0` has the same owner before and after msd starts, and msd logs "Under Caminito: mirroring to the GNOME settings only" and no conflict. A native GTK3 window's `GtkSettings` go from BlackMATE to Menta, to `Serif 13` and to the `mate-black` cursor theme as the MATE keys change. A pixel of Caminito's window is `FF0000` with that colour as the background, and `0000FF` after `picture-filename` names a blue picture. msd logs "Not activating typing_break plugin under Caminito" with `org.mate.typing-break enabled` on. `wlr-randr` shows scale 1 before msd and 1.5 after.
  - **Under a headless labwc** ([wayland.sh](71-msd-checks/wayland.sh)), with `MATE_WM_NAME=Caminito`: the mirror at start-up and live, the titlebar actions (`toggle_maximize_horizontally` to `toggle-maximize-horizontally`), `dpi` 120 to 1.25, one `mate-background` layer surface with `show-desktop-icons` on. Without the variable, as on Wayfire: the GNOME keys stay at their defaults, no layer surface, and the typing-break manager starts.
  - **On X11, under Xvfb** ([x11.sh](71-msd-checks/x11.sh)), with no `MATE_WM_NAME`: `_XSETTINGS_S0` has no owner before msd and one after; the root window is `FF0000`; the typing-break manager starts; and the dconf database has no `org.gnome` key after MATE's theme and font change. That is what the X11 criterion is ticked on; it wasn't a login.
- **msd's debug lines** need both `org.mate.debug mate-settings-daemon` and `G_MESSAGES_DEBUG=all`. With `--debug` and the variable but not the key, it printed none.
