# The session file and the first login

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M5
Blocked by: 61, 64, 67, 68, 70, 71
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

"MATE on Caminito" as something that can be chosen and logged into ([08](08-session-launch-model.md), [16](16-packaging.md)), and the first run of the real MATE components on Caminito as `caminitotest` ([14](14-development-and-testing-loop.md)). Every earlier M5 ticket was checked headless or nested; this is where they meet mate-session, mate-panel, caja and msd together, and where what they get wrong is found before the closing ticket.

- **`mate-caminito.desktop`** in `/usr/share/wayland-sessions/`, installed by the top-level `meson.build`: `Name=MATE on Caminito`, `Exec=caminito --session`, `DesktopNames=MATE`, `Type=Application`. No wrapper script and no systemd unit.
- **`caminito-git`'s dependencies (16, 17):** the three Patched packages by their real names (`mate-session-manager-caminito`, `mate-panel-caminito`, `mate-settings-daemon-caminito`), and the session's other components: `caja`, `mate-notification-daemon`, `mate-polkit`. `mate-screensaver`, `zenity` and the locker's libraries come with the milestones that use them.
- **README:** `--session` is supported; how to make the `caminitotest` user, log in on a spare tty and run `caminito --session` there; and how to read the session's journal.
- **The first login,** as `caminitotest` on a spare tty with `caminito --session`, on the installed packages. Fix here what's small; file a ticket for what isn't, and block [73](73-packaging-and-closing-m5.md) on it if M5's exit needs it.

## Acceptance criteria

- [ ] `makepkg -si` for `caminito-git` pulls in the three Patched packages, and the Packaging job is green with all four built in order.
- [ ] The display manager lists *MATE on Caminito*.
- [ ] As `caminitotest` on a tty, `caminito --session`:
  - reaches the desktop without any mate-session phase timing out (30 s, 08), with the phases' order in the journal;
  - exports to systemd: `systemctl --user show-environment` has `WAYLAND_DISPLAY`, `DISPLAY` and `MATE_WM_NAME=Caminito`, and after logout it has none of them;
  - logs to the journal (`journalctl --user -t caminito`), Caminito's helpers too;
  - shows mate-panel with the window list and the workspace switcher working, caja's desktop icons above msd's wallpaper on the output caja covers, and the wallpaper alone on the other;
  - has `mate-screensaver` not running, and `org.mate.ScreenSaver` owned by Caminito ([62](62-screensaver-names.md));
  - maximises native and X11 windows to the work area ([64](64-work-areas-from-struts.md));
  - gives a GTK3 app MATE's theme, fonts and cursor, natively and on XWayland ([71](71-mate-settings-daemon-caminito.md)).
- [x] The results are recorded in this ticket's notes, with a ticket for each failure not fixed here.

## Notes

- **The virtual machine first** (Arch Linux ARM, as in 59), then the developer's machine. The Patched packages' `arch` may need `aarch64` there, as `caminito-git`'s does (59).
- **Known gaps that aren't failures (06):** caja's desktop icons on one output only; a second modeset when msd applies the saved layout; mate-session's idle status. The window list showing every workspace's windows is the open question from [66](66-ext-workspace.md): decide here whether it needs a ticket in M5 or goes to M6.
- **An Xwayland crash logs the user out** (11). It's a documented limitation, not something to fix here.
- **Logging in from the display manager on the developer's own user** is the closing ticket's, since it shares the host's systemd manager and bus (14).
- **Resolved on 2026-10-05 with three criteria unticked.** The session file, the dependencies and the README are in. The machine this was done in (OrbStack, Arch Linux, aarch64) has no tty, no GPU (`/dev/dri`) and no display manager, and the virtual machine of 59 wasn't available. So the first login was `caminito --session` as `caminitotest` in a winit window on Xvfb, on `caminitotest`'s own systemd user manager and bus. That is neither a Session nor a nested run as CONTEXT.md has them: it has the systemd environment, and no logind session. [76](76-tty-login-left-over.md) carries the login on a tty, the display manager, `makepkg -si` on x86_64 and the Packaging job, which needs a push.
- **The packages (criterion 1):** [72-checks/build-packages.sh](72-checks/build-packages.sh) built and installed `mate-session-manager-caminito` 1.28.0.r23.g57a9178-1, `mate-panel-caminito` 1.28.7.r111.g58ac6c1-1 and `caminito-git` r148.fdb854a-1 with `makepkg --ignorearch --nocheck --syncdeps`, beside 71's `mate-settings-daemon-caminito` 1.29.1-1. `caminito-git`'s build pulled in `mate-notification-daemon` and `mate-polkit` from the repositories. The three Patched packages were installed before it, so that `makepkg` asks for them, and refuses without them, wasn't shown. `arch` is still `x86_64` in all four. `makepkg --install --noconfirm` answers no to taking out the stock package a Patched one conflicts with, so the script installs with `pacman -U`; CI's container has no stock package to take out.
- **The session file (criterion 2):** `pacman -Qo` gives `/usr/share/wayland-sessions/mate-caminito.desktop` to `caminito-git`. `desktop-file-validate` rejects `DesktopNames` as a key with no `X-`, as it does for every session file. A `Comment` line was added beyond what 08 lists. No display manager read it.
- **The run on Xvfb (2026-10-05), on those packages,** with [72-checks/xvfb-login.sh](72-checks/xvfb-login.sh). One 1280x800 output, no input device, no logind session. It ran eight times. The first three were with an earlier script that kept the user's manager between runs, so only the first of them started with a clean systemd environment; the last five each started a new manager. All eight gave the same results but for the logout.
  - Reaches the desktop: pass. The panel, caja, msd, the polkit agent and the notification daemon were running 25 s after the start. About 40 s after it, neither the journal nor what mate-session and its children wrote to stderr had a time-out. The phases' order isn't in the journal: mate-session logs it only with `--debug`, which wasn't tried.
  - Exports to systemd: pass. From a new manager, `systemctl --user show-environment` had none of the variables before, and `DISPLAY=:0`, `MATE_WM_NAME=Caminito`, `WAYLAND_DISPLAY=wayland-1`, `XCURSOR_SIZE=24`, `XDG_CURRENT_DESKTOP=MATE` and `XDG_SESSION_TYPE=wayland` in the session.
  - None of them after logout: FAIL in two of the five runs from a new manager, and in the three before them. When it passes, only `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` stay, as 61 decided. When it fails, all six stay, and Caminito logs `systemd didn't unset the environment within 5s`: mate-session restarts the user's bus as it exits, and Caminito's request races that. [74](74-systemd-environment-after-logout.md) has it, and blocks 73.
  - Logs to the journal: pass. `journalctl --user -t caminito`, as `caminitotest`, has Caminito's lines, and `-t Xwayland` has Xwayland's. `caminito-gtk-palette` wrote nothing. mate-session and what it starts write to Caminito's stderr, which was a file here, and not to the journal.
  - mate-panel: pass by a screenshot. Both panels are layer surfaces with exclusive zones 27 and 25; the window list had a button for each of two windows; the switcher showed four workspaces and followed a switch to the second (`wmctrl -s 1`). Nothing was clicked.
  - caja's desktop icons above msd's wallpaper: pass on the one output. Computer, Home and Trash are drawn on the gradient; `DumpState` has `mate-background` in `background` and `desktop` in `bottom`. Both are 1280x800 at y 27, below the top panel's zone, so their last 52 rows are under the bottom panel or off the output. It looks right. The second output is 76's.
  - `mate-screensaver` not running, `org.mate.ScreenSaver` owned by Caminito: pass. `org.freedesktop.ScreenSaver` too.
  - Maximising to the work area: pass. The work area was 0,27 1280x748. A native and an XWayland GTK3 window that asked to be maximised were both 1280x721 at 0,54 with a 27-row title bar.
  - MATE's theme, fonts and cursor: pass for the theme (`Menta`) and the font (`Sans 10`), the same in `gsettings` and in both windows. The cursor theme is MATE's default, which is empty, in all three, so it shows nothing; 76 sets one.
  - Logout: `mate-session-save --logout` ended mate-session with status 0, and Caminito exited with status 0, at once when systemd answered and 5 s later when it didn't.
- **Also seen, with no ticket:**
  - mate-session warns `Unable to find provider '' of required component 'dock'`, as it does on X11, and that `gnome-keyring-daemon` isn't installed (an optional dependency).
  - caja warns `Failed to register client: ... AlreadyRegistered`, and libxfce4windowing says `Window<->Workspace association is not available on your compositor` (75).
  - Caminito's warnings were winit's on Xvfb (`[EGL] BAD_SURFACE`, `error setting XSETTINGS`, the display's size) and two `Failed to handle X11 event`.
  - After a logout that left the variables in systemd, `xdg-desktop-portal-gtk` was started on the new bus and failed with `cannot open display: :2` (74).
  - The full test suite, in the same machine: `xwayland_crashing_again_and_again_as_it_starts_makes_caminito_give_up_and_run_without_x` failed in the first run. It passed alone three times, and a second full run passed. This ticket changes no Rust.
- **Left to 72 by other tickets, and not done here** for want of an input device, so 76 has them: that the panel's menus open and a `gtk-layer-shell` demo's keyboard modes (63), and whether msd's `xrdb -merge` and Caminito's own Xft resources get in each other's way (71; `xorg-xrdb` wasn't installed, and msd warned `Could not execute xrdb`). 63's question of caja's desktop menu under the windows is still open, and needs the developer.
- **The window list on every workspace (66):** seen. With two windows on workspace 1, the list on workspace 2 still had both buttons. [75](75-window-list-by-workspace.md) has it, proposed for M6: the session logs in and the list loses nothing, and M5's exit asks only that the list is shown. The developer hasn't confirmed the milestone.
- **The README** has a new *The MATE session* section. Its status paragraph and milestone table are 73's.
