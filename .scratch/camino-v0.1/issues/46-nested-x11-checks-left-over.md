# Nested X11 checks left over from 35 and 36

Type: verification
Status: open
Milestone: M3
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to check

Some of [35](35-xwayland-windows.md)'s, [36](36-override-redirect-windows.md)'s, [37](37-clipboard-and-dnd.md)'s, [38](38-x-connection-and-root-properties.md)'s and [39](39-x-window-properties-and-requests.md)'s, [41](41-x11-scale.md)'s and [42](42-xwayland-restart.md)'s nested checks used stand-ins or ran only through XTest on Xvfb. This ticket runs them by hand in a real nested Camino, before [43](43-packaging-and-closing-m3.md) repeats them on the installed package. It's checking only: anything that fails gets its own ticket.

- **gtk3-demo's tooltips (36):** in the Application Class demo, hovering the toolbar's New button showed no tooltip, and the demo wasn't tried on bare X. A PyGObject GTK3 tooltip stood in. The button's tooltip is "Create a new file".
- **xterm and xeyes (35):** they weren't installed, so glxgears and pluma stood in. Install them with `pacman -S xterm xorg-xeyes`.
- **Clipboard and drag-and-drop (37):** PyGObject GTK3 apps stood in for xterm and foot, and for a file manager's drag. Use xterm, foot and caja this time.
- **wmctrl (38):** it wasn't installed, so a script read the desktop atoms as `wmctrl -d` does. Install it with `pacman -S wmctrl`.
- **xterm's frame extents and `wmctrl -t` (39):** pluma stood in for xterm, and a script sent the `_NET_WM_DESKTOP` message `wmctrl -r` sends.
- **xterm at scale 2 (41):** xterm wasn't installed, so gtk3-demo alone was checked at a winit scale of 2.
- **xterm after an Xwayland restart (42):** xterm wasn't installed, so gtk3-demo stood in.
- **Menus by hand (36):** the menus were only driven through XTest, with Camino on Xvfb. This time use the real pointer and keyboard.

Setup: run `tools/dev-nested --bin` and read the X display from the line `dev-nested: WAYLAND_DISPLAY=wayland-N DISPLAY=:M` it prints once Xwayland is ready (43). Start each app in another terminal with `DISPLAY=:N GDK_BACKEND=x11`. For gtk3-demo, use `gtk3-demo --run=application_demo`.

## Acceptance criteria

- [ ] On the host desktop, without Camino, gtk3-demo's New button shows its tooltip. If it doesn't, find another gtk3-demo widget whose tooltip does show there, and use that one below.
- [ ] Nested, that tooltip shows above its window, and goes away when the pointer leaves the button.
- [ ] Nested, xterm and xeyes map with Menta frames, take focus on a click and move when their titlebar is dragged. xterm takes typing, and xeyes follows the pointer across Camino's output.
- [ ] Nested, gtk3-demo's Preferences menu opens on a click and shows above its window. Down, Right and Escape navigate it and its Color submenu. Clicking outside the menu closes it. Afterwards, typing goes to the window again.
- [ ] Nested, text copied in xterm pastes into foot, and back, through both the clipboard (Ctrl+Shift+C and V in foot) and the primary selection (select, then middle-click). A file dragged from caja with `GDK_BACKEND=x11` drops into a native caja, and the other way round.
- [x] Nested, `xprop` on an xterm shows `_NET_FRAME_EXTENTS` matching the Menta frame, and `wmctrl -r xterm -t 1` moves it to the second workspace.
- [ ] Nested with `tools/dev-nested --bin --scale 2`, xterm renders sharp with a Menta frame, and `xprop` shows its `_NET_FRAME_EXTENTS` at twice the frame's logical size.
- [ ] Nested, killing the nested Xwayland by pid with xterm open removes the xterm, and a new xterm on the same `DISPLAY` gets a Menta frame.
- [x] Nested, `wmctrl -d` lists four workspaces, `Workspace 1` to `Workspace 4`, with the first current, and `wmctrl -s 2` switches to the third.
- [ ] The results are recorded in this ticket's notes, with a new ticket for each failure.

## Notes

- **Scripted, not by hand:** xterm, xeyes and wmctrl are installed. The checks that need no hands ran against `tools/dev-nested --bin`, driven by `wmctrl` and `xprop`. It was started with `DISPLAY` set to an Xvfb, but winit found the host's `wayland-0` in `XDG_RUNTIME_DIR` and opened on the host desktop instead. A screenshot of the Xvfb was black.
  - `wmctrl -d` listed `Workspace 1` to `Workspace 4` with the first current, and `wmctrl -s 2` made the third current (`_NET_CURRENT_DESKTOP` 2).
  - xterm and xeyes both mapped with `_NET_FRAME_EXTENTS` 1, 1, 31, 1, the Menta frame's, as 42's gtk3-demo had.
  - `wmctrl -r xterm -t 1` matches by title, and xterm's shell sets its title to the prompt, so it matched nothing and exited 1. `wmctrl -x -r xterm -t 1`, or an xterm started with `-T xterm -xrm 'XTerm*allowTitleOps: false'`, set `_NET_WM_DESKTOP` to 1. That's wmctrl's matching, not a Camino failure.
  - SIGKILL of the nested Xwayland by pid, with xterm and xeyes open: Camino logged the restart, Xwayland was ready on `:2` again 600 ms later, both clients had exited, and a new xterm on `:2` got `_NET_FRAME_EXTENTS` 1, 1, 31, 1. Nobody watched the output, so the criterion waits for the hand run.
  - With `--scale 2`, xterm's `_NET_FRAME_EXTENTS` were 2, 2, 62, 2, twice the frame's. Sharpness needs eyes, so the criterion waits for the hand run.
- **The hand checks** are in the wizard [46-hand-checks.sh](46-hand-checks.sh). Run it from a terminal on the host desktop with `tools/dev-nested --bin` in another. It launches each app on the right display, each GTK app on its own `dbus-run-session` so it can't hand off to a host instance, and walks through every criterion. The results go into these notes, and it ticks each criterion whose checks all passed.
