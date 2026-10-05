# A cursor theme set in MATE's settings changes no pointer

Type: implementation
Status: open
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

- [ ] The cause is written down in this ticket's notes, with a headless test written before the fix where the cause is Camino's.
- [ ] In the session, `gsettings set org.mate.peripherals-mouse cursor-theme Oxygen_Yellow` turns the pointer yellow over a frame, the desktop, the panel, a native GTK3 window and an XWayland one, with no restart of any of them.
- [ ] What can't follow a live change (clients that read `XCURSOR_THEME` from their environment) is listed in the README's *Known limitations*, or `XCURSOR_THEME` is no longer exported.

## Notes

- M5's exit gives a GTK3 app MATE's cursor ([72](72-session-file-and-first-login.md)'s criterion), so this blocks [73](73-packaging-and-closing-m5.md).
