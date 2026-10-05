# The login on a tty left over from 72

Type: verification
Status: open
Milestone: M5
Blocked by: 72
Map: [Camino v0.1](../map.md)

## What to check

[72](72-session-file-and-first-login.md)'s first login was run nested on Xvfb, in a machine with no tty, no GPU and no display manager. This ticket runs what that couldn't: `camino --session` as `caminotest` on a spare tty, on the installed packages, in the virtual machine or on the developer's machine. It's checking only: anything that fails gets its own ticket, and blocks [73](73-packaging-and-closing-m5.md) if M5's exit needs it.

The README's *The MATE session* section has the steps to make the user and start the session.

## Acceptance criteria

- [ ] The Packaging job is green on a push with 72's `camino-git`, with all four packages built in order.
- [ ] The display manager lists *MATE on Camino*.
- [ ] As `caminotest` on a tty, `camino --session`:
  - reaches the desktop with no mate-session phase timing out;
  - logs to the journal when its stderr isn't the terminal (`camino --session 2>/dev/null`);
  - shows caja's desktop icons above msd's wallpaper on the output caja covers, and the wallpaper alone on the other;
  - has the window list's buttons and the workspace switcher answer a click;
  - maximises a native and an X11 window to the work area on each output;
  - gives a GTK3 app a cursor theme set in MATE's settings, natively and on XWayland.
- [ ] Logging out from the panel gives the tty back.

## Notes

- **What the nested run already showed** is in 72's notes: the systemd export, the journal, the screensaver names, both panels, one output's desktop, the work area, the theme and the font.
- **[74](74-systemd-environment-after-logout.md)** is known: the variables stay in systemd after logout. Whether mate-session restarts the bus after a login on a tty too is worth a look here (`systemctl --user show-environment` from a second login after the logout).
- **mate-session doesn't log its phases** unless it's run with `--debug`, so their order isn't in the journal (72). `camino --session="mate-session --debug"` shows them.
