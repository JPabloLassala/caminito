# mate-settings-daemon-camino

Type: implementation
Status: open
Milestone: M5
Blocked by: 69
Map: [Camino v0.1](../map.md)

## What to build

msd as a Patched package ([16](16-packaging.md)) at 1.29.1 or a later 1.29.x tag, the first with the wlrandr plugin (14), with the three Carried patches the session needs first ([17](17-milestone-slicing.md)). All three are active only when `MATE_WM_NAME=Camino` ([ADR 0011](../../../docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-camino.md)).

- **`packaging/arch/mate-settings-daemon-camino/PKGBUILD`,** from Arch's mate-settings-daemon PKGBUILD with the smallest diff: the 1.29.x tag as source, `provides` and `conflicts` on `mate-settings-daemon`, the patches, and whatever the tag newly depends on.
- **The patches:**
  - `0001` **xsettings, mirror-only** ([ADR 0005](../../../docs/adr/0005-msd-mirrors-mate-settings-into-gnome-schemas.md), [ADR 0006](../../../docs/adr/0006-camino-serves-xsettings.md), [09](09-gsettings-mapping.md)): under Camino the plugin doesn't take `_XSETTINGS_S0`, which Camino owns (40). It mirrors MATE's settings one way into the GNOME keys native GTK reads, live, with the plugin's existing translation table. What's mirrored is 09's list;
  - `0002` **background** (06): the plugin stays on under Camino and draws the wallpaper as a `mate-background` layer surface on every output, under caja's `desktop` ([63](63-layer-shell.md));
  - `0003` **typing break** (13): the plugin doesn't start under Camino.
- **Unpatched, and checked here:**
  - the wlrandr plugin applies `monitors-wayland.xml` over wlr-output-management (58) at login;
  - the clipboard plugin keeps the clipboard through data-control ([68](68-data-control-and-privileged-globals.md)). 11 inferred this; if the plugin is `X11Only` or absent on Wayland, say so in the notes and file a ticket, since M5's checklist item depends on it.
- **CI:** a line each in the Packaging job and the weekly patch-on-head job ([69](69-mate-session-manager-camino.md)).

## Acceptance criteria

- [ ] `makepkg -si` installs it in place of `mate-settings-daemon`; `namcap` reports nothing unexplained; both CI jobs are green.
- [ ] Nested with `MATE_WM_NAME=Camino`, msd started by hand:
  - Camino keeps `_XSETTINGS_S0`, and msd logs no XSETTINGS conflict;
  - `gsettings set org.mate.interface gtk-theme` changes `org.gnome.desktop.interface gtk-theme`, and a native GTK3 app restyles; the same for a font and the cursor theme;
  - the wallpaper shows, and changing `org.mate.background picture-filename` changes it;
  - the typing-break plugin isn't loaded.
- [ ] Nested, a saved `monitors-wayland.xml` with the output at scale 1.5 is applied when msd starts.
- [ ] Under X11 MATE, the patched msd owns `_XSETTINGS_S0`, draws the background as before, and writes nothing to the GNOME schemas.

## Notes

- **The rest of msd's series** (media-keys, keybindings, the OSD, touchpad) is M6 (17).
- **Which plugins are `X11Only`** at the tag decides how much `0002` has to change; read each `.mate-settings-plugin` file first.
- **The mirror's trade-off** (a user who also logs into GNOME gets MATE's choices there) is ADR 0005's, and isn't reopened here.
