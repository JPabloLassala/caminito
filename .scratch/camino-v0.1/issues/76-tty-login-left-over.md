# The login on a tty left over from 72

Type: verification
Status: open
Milestone: M5
Blocked by: 72
Map: [Camino v0.1](../map.md)

## What to check

[72](72-session-file-and-first-login.md)'s first login ran in a winit window on Xvfb, in a machine with no tty, no GPU, no input device and no display manager. This ticket runs what that couldn't: `camino --session` as `caminotest` on a spare tty, on the installed packages, in the virtual machine first (Arch Linux ARM, as in 59) and then on the developer's machine. It's checking only: anything that fails gets its own ticket, and blocks [73](73-packaging-and-closing-m5.md) if M5's exit needs it.

The README's *The MATE session* section has the steps to make the user and start the session.

## Acceptance criteria

- [ ] On x86_64, `makepkg -si` for `camino-git` pulls in the three Patched packages, and the Packaging job is green on a push with 72's `camino-git`, with all four built in order.
- [ ] The display manager lists *MATE on Camino*.
- [ ] As `caminotest` on a tty, `camino --session`:
  - reaches the desktop with no mate-session phase timing out (30 s, 08), with the phases' order seen once with `camino --session="mate-session --debug"`;
  - exports to systemd, from a login this time: `systemctl --user show-environment` has `WAYLAND_DISPLAY`, `DISPLAY` and `MATE_WM_NAME=Camino`;
  - logs to the journal when its stderr isn't the terminal (`camino --session 2>~/session.log`);
  - shows caja's desktop icons above msd's wallpaper on the output caja covers, and the wallpaper alone on the other;
  - has the panel's menus open, and the window list's buttons and the workspace switcher answer a click ([63](63-layer-shell.md) left the menus to 72);
  - has `mate-screensaver` not running, and `org.mate.ScreenSaver` owned by Camino;
  - maximises a native and an X11 window to the work area on each output;
  - gives a GTK3 app a cursor theme set in MATE's settings, natively and on XWayland.
- [ ] A `gtk-layer-shell` demo's keyboard modes work as 63 says (left to 72 by 63).
- [ ] With `xorg-xrdb` installed, msd's `xrdb -merge` and Camino's own Xft resources (47) don't get in each other's way: `xrdb -query` after login and after a theme change ([71](71-mate-settings-daemon-camino.md) left it to 72).
- [ ] Logging out from the panel gives the tty back, and whether the variables stay in systemd is written down for [74](74-systemd-environment-after-logout.md) (`systemctl --user show-environment` from a second login).

## Notes

- **What the run on Xvfb already showed** is in 72's notes: the systemd export, the journal, the screensaver names, both panels, one output's desktop, the work area, the theme and the font.
- **mate-session doesn't log its phases** unless it's run with `--debug` (72).
- **63's open question,** caja's desktop menu drawn under the windows that overlap it, can be looked at in the same sitting. It needs the developer's decision.
