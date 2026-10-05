# The login on a tty left over from 72

Type: verification
Status: resolved
Milestone: M5
Blocked by: 72
Map: [Camino v0.1](../map.md)

## What to check

[72](72-session-file-and-first-login.md)'s first login ran in a winit window on Xvfb, in a machine with no tty, no GPU, no input device and no display manager. This ticket runs what that couldn't: `camino --session` as `caminotest` on a spare tty, on the installed packages, in the virtual machine first (Arch Linux ARM, as in 59) and then on the developer's machine. It's checking only: anything that fails gets its own ticket, and blocks [73](73-packaging-and-closing-m5.md) if M5's exit needs it.

The README's *The MATE session* section has the steps to make the user and start the session.

## Acceptance criteria

- [x] On x86_64, `makepkg -si` for `camino-git` pulls in the three Patched packages, and the Packaging job is green on a push with 72's `camino-git`, with all four built in order.
- [x] The display manager lists *MATE on Camino*.
- [ ] As `caminotest` on a tty, `camino --session`:
  - reaches the desktop with no mate-session phase timing out (30 s, 08), with the phases' order seen once with `camino --session="mate-session --debug"`;
  - exports to systemd, from a login this time: `systemctl --user show-environment` has `WAYLAND_DISPLAY`, `DISPLAY` and `MATE_WM_NAME=Camino`;
  - logs to the journal when its stderr isn't the terminal (`camino --session 2>~/session.log`);
  - shows caja's desktop icons above msd's wallpaper on the output caja covers, and the wallpaper alone on the other;
  - has the panel's menus open, and the window list's buttons and the workspace switcher answer a click ([63](63-layer-shell.md) left the menus to 72);
  - has `mate-screensaver` not running, and `org.mate.ScreenSaver` owned by Camino;
  - maximises a native and an X11 window to the work area on each output;
  - gives a GTK3 app a cursor theme set in MATE's settings, natively and on XWayland.
- [x] A `gtk-layer-shell` demo's keyboard modes work as 63 says (left to 72 by 63).
- [x] With `xorg-xrdb` installed, msd's `xrdb -merge` and Camino's own Xft resources (47) don't get in each other's way: `xrdb -query` after login and after a theme change ([71](71-mate-settings-daemon-camino.md) left it to 72).
- [ ] Logging out from the panel gives the tty back, and whether the variables stay in systemd is written down for [74](74-systemd-environment-after-logout.md) (`systemctl --user show-environment` from a second login).

## Notes

- **What the run on Xvfb already showed** is in 72's notes: the systemd export, the journal, the screensaver names, both panels, one output's desktop, the work area, the theme and the font.
- **mate-session doesn't log its phases** unless it's run with `--debug` (72).
- **63's open question,** caja's desktop menu drawn under the windows that overlap it, can be looked at in the same sitting. It needs the developer's decision.
- **The wizard,** [76-checks/tty-login.sh](76-checks/tty-login.sh), walks through every criterion from the display manager on, with 63's question and what 74 asks about. It's 59's kind: it shows what the session's own tools say (`systemctl --user show-environment`, the journal, `DumpState`, `busctl`, `xrdb -query`), asks for what needs eyes and hands, saves each result as it's given, and resumes a stopped run. A check can be recorded as not run. It runs as `caminotest` beside the session, and goes on after each of its two logouts when it's run again from the tty. [76-checks/prepare.sh](76-checks/prepare.sh) is the developer's part before it: `makepkg -si` for the four packages in order, and a copy of the checks in `/home/caminotest/76-checks`, since `caminotest` can't read the developer's home. `layer-demo.py` is the `gtk-layer-shell` demo, and `gtk-window.py` the window that asks to be maximised.
- **Not run yet (2026-10-05).** The wizard was written in an agent's session on the developer's machine (x86_64, sddm, two monitors, `caminotest` there already, `camino-git r116.2e03d31-1` and the stock MATE packages installed). That session wasn't allowed to install packages, and it can't sit at a tty, so no criterion that needs the session was checked, and every one of them is still open. Only the wizard's first stage and its `DumpState` printing were tried, against made-up state; the stages themselves have never met a session. The virtual machine of 59 is on another machine, and wasn't to be had.
- **`makepkg` without the Patched packages (criterion 1), on x86_64:** `makepkg --syncdeps` for `camino-git`, with none of the three installed, stops with `target not found` for each and `Could not resolve all dependencies`. So the stock packages don't satisfy it, which 72 couldn't show. The four in order with `makepkg -si` is `prepare.sh`'s, and not done.
- **The Packaging job (criterion 1):** green on the push of `6c21223`, which has 72's `camino-git` (run 37317288066, 13m37s, all four built and installed in order, with `camino-git`'s `check()`).
- **CI on the same push failed,** with one session test timed out. [77](77-session-test-times-out-in-ci.md) has it, and blocks 73.
- **The first login on the tty (2026-10-05), on the developer's machine** (x86_64, NVIDIA RTX 3060, two monitors), on `camino-git r163.6e542b3-1`, `mate-session-manager-camino 1.28.0.r23.g57a9178-1`, `mate-panel-camino 1.28.7.r111.g58ac6c1-2` and `mate-settings-daemon-camino 1.29.1-1`, installed with `prepare.sh`. As `caminotest` on tty4: `camino --session="mate-session --debug" 2>~/session.log`, from 11:31:31 to a logout at 11:35:13. The wizard wasn't run in it, so what follows is read from `session.log` and the journal afterwards, and nothing that needs eyes was recorded. No criterion is ticked on it.
  - The phases: `INITIALIZATION`, `WINDOW_MANAGER`, `PANEL`, `DESKTOP`, `APPLICATION`, `RUNNING`, in that order, each ended by its component registering, all inside one second. At logout: `QUERY_END_SESSION`, `END_SESSION`, `EXIT`. Neither the file nor the journal has a time-out. mate-session logged `skipping component 'windowmanager' under Camino`.
  - Running 90 s in: mate-session, msd, mate-panel, caja, the notification daemon and the polkit agent. Camino lit HDMI-A at 3840x2160 and DisplayPort at 1920x1080, and Xwayland was `:2`.
  - The journal: 76 lines under `camino`, 22 under `Xwayland`, and mate-session's own 964, since its `--debug` writes to syslog as well as to stderr. The file has what the components wrote.
  - `mate-screensaver` wasn't running, but because it crashed, as `mate-power-manager` and `mate-maximus` did: [78](78-x11-only-autostarts-crash.md).
  - After the logout: `systemctl --user show-environment` had only `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE`, and Camino logged no `systemd didn't unset the environment`. For [74](74-systemd-environment-after-logout.md): mate-session did restart the user's bus on the tty (`dbus.service` active since 11:35:13), and the bus went first, with zbus's `Failed to parse NameLost signal` and `stopping the session command` before `the session command exited: exit status: 0`. One sample, and not from a clean user manager: it was 50 minutes old, with 16 `background` logind sessions and stray processes left by earlier runs of `72-checks/xvfb-login.sh`.
  - Also seen, with no ticket: at exit, `Failed to restore previous state. Error: Invalid argument (os error 22)` from the DRM device; `Failed to destroy old mode property blob` at each modeset; two `Failed to handle X11 event`; msd's `Failed to call "GrabMediaPlayerKeys"` and a `gtk_widget_get_scale_factor` critical. After the logout `xdg-desktop-portal-gtk` failed on the new bus with `cannot open display:`, an empty one this time. The machine's KDE autostart entries that aren't limited to KDE started too (kclockd, kgpg, kdeconnectd, kalendarac, kunifiedpush-distributor), and `kunifiedpush-distributor` outlived the session.
- **Resolved on 2026-10-05 with two criteria unticked.** The checks were made on the developer's machine in two logins: the one on tty4 as `caminotest` above, and one from sddm as the developer, below. The third criterion is unticked for its cursor theme, which failed and is [79](79-cursor-theme-change-not-shown.md), and because all but its first and third lines were seen in the developer's login and not in `caminotest`'s. The last is unticked because nobody wrote down what the tty looked like after the logout; 73's checklist item on logging out has it. The virtual machine wasn't used. The wizard was never run: the agent that wrote it ran its commands from inside the developer's session, and the developer looked.
- **`makepkg -si` (criterion 1):** `prepare.sh` built and installed the four in order on x86_64, with `camino-git`'s `check()`. With the refusal and the Packaging job above, that's the criterion.
- **The login from sddm (2026-10-05, 11:59), as the developer,** on the same packages. sddm listed *MATE on Camino* and ran `camino --session` on tty1, with Camino's stderr in `~/.local/share/sddm/wayland-session.log`. Logging in as oneself was to wait for 73 (14); it was done here, and it's what the rest was checked in. HDMI-A-1 (3840x2160) and DP-1 (1920x1080) were both at scale 2, from the developer's `window-scaling-factor` of 2.
  - Reaches the desktop: pass. msd, the panel, caja, the notification daemon and the polkit agent ran, and neither the journal nor sddm's log had a time-out. No `--debug` here; the phases are the tty login's.
  - Exports to systemd, from a login: pass. `DISPLAY=:1`, `MATE_WM_NAME=Camino`, `WAYLAND_DISPLAY=wayland-1`, `XCURSOR_SIZE=24`, `XCURSOR_THEME=mate`, `XDG_CURRENT_DESKTOP=MATE` and `XDG_SESSION_TYPE=wayland`, with a logind session of class `user` on tty1.
  - Logs to the journal: pass. 43 lines under `camino` and 22 under `Xwayland`, with Camino's stderr a file. `camino-gtk-palette` wrote nothing.
  - The desktop on each output: pass, by the developer's eyes. Icons over the wallpaper on HDMI-A-1, the wallpaper alone on DP-1. `DumpState` has `mate-background` in `background` on both and `desktop` in `bottom` on HDMI-A-1. As in 72, both of HDMI-A-1's are 1920x1080 at y 26, so their last rows are under the bottom panel or off the output.
  - The panel: pass. The three menus and their submenus open and close, the panel's own menu opens, the window list's buttons focus, minimise and restore foot and xterm, and the switcher goes to the second workspace and back.
  - The screensaver names: pass. `org.mate.ScreenSaver` and `org.freedesktop.ScreenSaver` are Camino's. `mate-screensaver` wasn't running, having crashed again with the other two (78).
  - Maximising: pass on both outputs. On HDMI-A-1, work area 0,26 1920x1029, a native and an X11 GTK3 window were both 1920x998 at 0,57 under a 31-row title bar. On DP-1, which has no panel, work area 1920,0 960x540, both were 960x509 at 1920,31.
  - The cursor theme: FAIL. With `cursor-theme` set to `Oxygen_Yellow`, both windows reported it and no pointer on screen changed. [79](79-cursor-theme-change-not-shown.md).
  - The layer-shell keyboard modes (63): pass, with [76-checks/layer-demo.py](76-checks/layer-demo.py) on the overlay layer. `exclusive` had the keyboard without a click, kept it through a click on a window, and gave it back to the focused window on Escape. `on_demand` had it from a click on it until a click on a window. `none` never had it.
  - `xrdb` (71, 47): pass. `xrdb -query` had 193 lines after login: each of Camino's eight once (`Xft.dpi: 192`, `Xcursor.size: 48`), and msd's 185 colour lines. After the theme went from BlackMATE to Menta, and after it went back, the output was the same to the byte. So neither undoes the other. But msd's colours didn't follow the theme: its `xrdb -merge` didn't run again, or merged the same colours, in the 9 s waited. Whether it does on X11 wasn't looked at, and it has no ticket.
  - 63's question: seen. caja's desktop menu is cut off where a window overlaps it. The developer decided it goes above the windows: [80](80-layer-popups-above-windows.md).
  - Also seen: Camino logged `surface missing from known popups` three times, and `Got X11 Error` in runs.
- **The demo had a bug,** found in this run and fixed: it imported `Gdk` without asking for 3.0, and PyGObject loaded 4.0.

