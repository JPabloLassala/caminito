# A cursor theme set in MATE's settings changes no pointer

Type: implementation
Status: resolved
Milestone: M5
Blocked by: 71
Map: [Camino v0.1](../map.md)

## What to build

In the session, changing `org.mate.peripherals-mouse cursor-theme` changes the pointer nowhere. [76](76-tty-login-left-over.md) saw it on 2026-10-05, in a login from sddm on the developer's machine (two outputs at scale 2, NVIDIA), with the theme changed from `mate` to `Oxygen_Yellow`, whose arrow is yellow.

- **The name got everywhere it's sent.** `org.gnome.desktop.interface cursor-theme` (msd's mirror, [71](71-mate-settings-daemon-camino.md)) and Camino's `Xcursor.theme` resource (47) were `Oxygen_Yellow` within 3 s. A GTK3 window started after the change reported `gtk-cursor-theme-name` as `Oxygen_Yellow`, both on Wayland and on XWayland.
- **The pointer didn't follow.** The arrow stayed black over those two windows, over frames, over the panel, over the desktop, and over a `mate-calc` started from the panel after the change. In foot and VS Code the text and hand cursors were drawn in a blue-ish Oxygen theme, which is neither `mate` nor `Oxygen_Yellow`; whether they were that before the change wasn't looked at.
- **systemd's environment kept `XCURSOR_THEME=mate`,** which Camino exported at start-up (61). So does the environment of everything mate-session started. Clients that take the theme from the environment, and Xcursor, which reads `XCURSOR_THEME` before the resource, can't follow a change.
- **Not known:** why Camino's own cursors, and the shapes it draws for `wp_cursor_shape_v1` clients, didn't change. `State::settings_changed` does call `Cursors::settings_changed`, which loads the new theme and drops the loaded images. Whether the theme change reached it, whether the cursor on screen is drawn again without a change of shape, and where the blue-ish theme comes from (the theme's `Inherits`, or `~/.icons/default`, which inherits Adwaita on this machine) are the first things to look at. 72's run on Xvfb had an empty default theme and no pointer, so it showed nothing of this.

## Acceptance criteria

- [x] The cause is written down in this ticket's notes, with a headless test written before the fix where the cause is Camino's.
- [ ] In the session, `gsettings set org.mate.peripherals-mouse cursor-theme Oxygen_Yellow` turns the pointer yellow over a frame, the desktop, the panel, a native GTK3 window and an XWayland one, with no restart of any of them. Not run: it needs a login on a build with the fix, and the session this was written in runs the installed one. It's the developer's, with [73](73-packaging-and-closing-m5.md)'s checks.
- [x] What can't follow a live change (clients that read `XCURSOR_THEME` from their environment) is listed in the README's *Known limitations*, or `XCURSOR_THEME` is no longer exported.

## Notes

- M5's exit gives a GTK3 app MATE's cursor ([72](72-session-file-and-first-login.md)'s criterion), so this blocks [73](73-packaging-and-closing-m5.md).
- **The cause was Camino's, twice over (2026-10-05).** The change did reach `Cursors::settings_changed`, and the cursor on screen was drawn again. What it drew was wrong:
  - **The arrow came from Adwaita.** `Oxygen_Yellow` names its arrow `left_ptr` only, and has no `default`. Camino asked the `xcursor` crate for each of the icon's names in turn, and the crate follows a theme's inheritance for one name before Camino tried the next. A theme with no `Inherits` inherits `default`, which is `/usr/share/icons/default` on the developer's machine and inherits Adwaita, which has a `default` cursor, and it's black. (`~/.icons` is empty there; the ticket had the wrong directory.) So the arrow went from `mate`'s black one to Adwaita's black one. `cursor::files` now takes a file in the theme itself, under any of the icon's names, before one in a theme it inherits, with the crate's `load_icon_with_depth`. The lookup is shared by the cursors Camino draws for `wp_cursor_shape_v1` clients, which GTK 3.24.52 is, and by XWayland's root cursor, which is why the arrow was black everywhere.
  - **The blue-ish theme was `Oxygen_Yellow` with red and blue swapped.** The crate's `pixels_rgba` is the file's order, ARGB32 words, and Camino gave it to the renderer as `Abgr8888`. Grey themes look the same either way, so `mate` and Adwaita never showed it. It's `Argb8888` now. foot and VS Code ask for `text` and `pointer` by name, which `Oxygen_Yellow` has, so those were the theme's own, in cyan. XWayland's root cursor always had the right order.
- **The test** is `a_theme_without_a_default_cursor_shows_its_own_arrow_not_one_it_inherits`, in `tests/cursors.rs`, written first: it failed on black, then on cyan. It writes two themes in a temporary directory, names one by its absolute path in `cursor-theme` (the crate joins a theme's name to each directory it searches, so that finds it whatever is installed), and reads the pixel under a pointer that hasn't moved. A throwaway run of the same lookup on the installed `Oxygen_Yellow` gave yellow for the arrow, `text`, `pointer`, `move` and `w-resize`.
- **`XCURSOR_THEME` is still exported,** since it's the only way a client that loads its own cursors gets MATE's theme at all. What that leaves is in the README's *Known limitations*.
- **Not Camino's, and not checked in a session:** an X11 GTK3 widget that asks for `default` by name goes through libXcursor, which looks in the theme, then in what it inherits, then in `default`, so with `Oxygen_Yellow` it'd get Adwaita's arrow under any window manager. An X window that sets no cursor shows the root's, which is Camino's.
