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
- **The wizard,** [76-checks/tty-login.sh](76-checks/tty-login.sh), walks through every criterion from the display manager on, with 63's question and what 74 asks about. It's 59's kind: it shows what the session's own tools say (`systemctl --user show-environment`, the journal, `DumpState`, `busctl`, `xrdb -query`), asks for what needs eyes and hands, saves each result as it's given, and resumes a stopped run. A check can be recorded as not run. It runs as `caminotest` beside the session, and goes on after each of its two logouts when it's run again from the tty. [76-checks/prepare.sh](76-checks/prepare.sh) is the developer's part before it: `makepkg -si` for the four packages in order, and a copy of the checks in `/home/caminotest/76-checks`, since `caminotest` can't read the developer's home. `layer-demo.py` is the `gtk-layer-shell` demo, and `gtk-window.py` the window that asks to be maximised.
- **Not run yet (2026-10-05).** The wizard was written in an agent's session on the developer's machine (x86_64, sddm, two monitors, `caminotest` there already, `camino-git r116.2e03d31-1` and the stock MATE packages installed). That session wasn't allowed to install packages, and it can't sit at a tty, so no criterion that needs the session was checked, and every one of them is still open. Only the wizard's first stage and its `DumpState` printing were tried, against made-up state; the stages themselves have never met a session. The virtual machine of 59 is on another machine, and wasn't to be had.
- **`makepkg` without the Patched packages (criterion 1), on x86_64:** `makepkg --syncdeps` for `camino-git`, with none of the three installed, stops with `target not found` for each and `Could not resolve all dependencies`. So the stock packages don't satisfy it, which 72 couldn't show. The four in order with `makepkg -si` is `prepare.sh`'s, and not done.
- **The Packaging job (criterion 1):** green on the push of `6c21223`, which has 72's `camino-git` (run 37317288066, 13m37s, all four built and installed in order, with `camino-git`'s `check()`).
- **CI on the same push failed,** with one session test timed out. [77](77-session-test-times-out-in-ci.md) has it, and blocks 73.

