# The session file and the first login

Type: implementation
Status: open
Milestone: M5
Blocked by: 61, 64, 67, 68, 70, 71
Map: [Camino v0.1](../map.md)

## What to build

"MATE on Camino" as something that can be chosen and logged into ([08](08-session-launch-model.md), [16](16-packaging.md)), and the first run of the real MATE components on Camino as `caminotest` ([14](14-development-and-testing-loop.md)). Every earlier M5 ticket was checked headless or nested; this is where they meet mate-session, mate-panel, caja and msd together, and where what they get wrong is found before the closing ticket.

- **`mate-camino.desktop`** in `/usr/share/wayland-sessions/`, installed by the top-level `meson.build`: `Name=MATE on Camino`, `Exec=camino --session`, `DesktopNames=MATE`, `Type=Application`. No wrapper script and no systemd unit.
- **`camino-git`'s dependencies (16, 17):** the three Patched packages by their real names (`mate-session-manager-camino`, `mate-panel-camino`, `mate-settings-daemon-camino`), and the session's other components: `caja`, `mate-notification-daemon`, `mate-polkit`. `mate-screensaver`, `zenity` and the locker's libraries come with the milestones that use them.
- **README:** `--session` is supported; how to make the `caminotest` user, log in on a spare tty and run `camino --session` there; and how to read the session's journal.
- **The first login,** as `caminotest` on a spare tty with `camino --session`, on the installed packages. Fix here what's small; file a ticket for what isn't, and block [73](73-packaging-and-closing-m5.md) on it if M5's exit needs it.

## Acceptance criteria

- [ ] `makepkg -si` for `camino-git` pulls in the three Patched packages, and the Packaging job is green with all four built in order.
- [ ] The display manager lists *MATE on Camino*.
- [ ] As `caminotest` on a tty, `camino --session`:
  - reaches the desktop without any mate-session phase timing out (30 s, 08), with the phases' order in the journal;
  - exports to systemd: `systemctl --user show-environment` has `WAYLAND_DISPLAY`, `DISPLAY` and `MATE_WM_NAME=Camino`, and after logout it has none of them;
  - logs to the journal (`journalctl --user -t camino`), Camino's helpers too;
  - shows mate-panel with the window list and the workspace switcher working, caja's desktop icons above msd's wallpaper on the output caja covers, and the wallpaper alone on the other;
  - has `mate-screensaver` not running, and `org.mate.ScreenSaver` owned by Camino ([62](62-screensaver-names.md));
  - maximises native and X11 windows to the work area ([64](64-work-areas-from-struts.md));
  - gives a GTK3 app MATE's theme, fonts and cursor, natively and on XWayland ([71](71-mate-settings-daemon-camino.md)).
- [ ] The results are recorded in this ticket's notes, with a ticket for each failure not fixed here.

## Notes

- **The virtual machine first** (Arch Linux ARM, as in 59), then the developer's machine. The Patched packages' `arch` may need `aarch64` there, as `camino-git`'s does (59).
- **Known gaps that aren't failures (06):** caja's desktop icons on one output only; a second modeset when msd applies the saved layout; mate-session's idle status. The window list showing every workspace's windows is the open question from [66](66-ext-workspace.md): decide here whether it needs a ticket in M5 or goes to M6.
- **An Xwayland crash logs the user out** (11). It's a documented limitation, not something to fix here.
- **Logging in from the display manager on the developer's own user** is the closing ticket's, since it shares the host's systemd manager and bus (14).
