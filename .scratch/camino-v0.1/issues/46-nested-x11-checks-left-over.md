# Nested X11 checks left over from 35 and 36

Type: verification
Status: open
Milestone: M3
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to check

Some of [35](35-xwayland-windows.md)'s, [36](36-override-redirect-windows.md)'s, [37](37-clipboard-and-dnd.md)'s and [38](38-x-connection-and-root-properties.md)'s nested checks used stand-ins or ran only through XTest on Xvfb. This ticket runs them by hand in a real nested Camino, before [43](43-packaging-and-closing-m3.md) repeats them on the installed package. It's checking only: anything that fails gets its own ticket.

- **gtk3-demo's tooltips (36):** in the Application Class demo, hovering the toolbar's New button showed no tooltip, and the demo wasn't tried on bare X. A PyGObject GTK3 tooltip stood in. The button's tooltip is "Create a new file".
- **xterm and xeyes (35):** they weren't installed, so glxgears and pluma stood in. Install them with `pacman -S xterm xorg-xeyes`.
- **Clipboard and drag-and-drop (37):** PyGObject GTK3 apps stood in for xterm and foot, and for a file manager's drag. Use xterm, foot and caja this time.
- **wmctrl (38):** it wasn't installed, so a script read the desktop atoms as `wmctrl -d` does. Install it with `pacman -S wmctrl`.
- **Menus by hand (36):** the menus were only driven through XTest, with Camino on Xvfb. This time use the real pointer and keyboard.

Setup: run `tools/dev-nested --bin` and read the X display from the log line `Xwayland is ready on DISPLAY=:N` (43 makes `dev-nested` print it). Start each app in another terminal with `DISPLAY=:N GDK_BACKEND=x11`. For gtk3-demo, use `gtk3-demo --run=application_demo`.

## Acceptance criteria

- [ ] On the host desktop, without Camino, gtk3-demo's New button shows its tooltip. If it doesn't, find another gtk3-demo widget whose tooltip does show there, and use that one below.
- [ ] Nested, that tooltip shows above its window, and goes away when the pointer leaves the button.
- [ ] Nested, xterm and xeyes map with Menta frames, take focus on a click and move when their titlebar is dragged. xterm takes typing, and xeyes follows the pointer across Camino's output.
- [ ] Nested, gtk3-demo's Preferences menu opens on a click and shows above its window. Down, Right and Escape navigate it and its Color submenu. Clicking outside the menu closes it. Afterwards, typing goes to the window again.
- [ ] Nested, text copied in xterm pastes into foot, and back, through both the clipboard (Ctrl+Shift+C and V in foot) and the primary selection (select, then middle-click). A file dragged from caja with `GDK_BACKEND=x11` drops into a native caja, and the other way round.
- [ ] Nested, `wmctrl -d` lists four workspaces, `Workspace 1` to `Workspace 4`, with the first current, and `wmctrl -s 2` switches to the third.
- [ ] The results are recorded in this ticket's notes, with a new ticket for each failure.
