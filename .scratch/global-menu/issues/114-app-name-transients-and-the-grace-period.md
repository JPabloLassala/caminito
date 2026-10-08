# The application's name, a dialog's parent menu, and the panel's restarts

Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [110](110-global-menu-for-gtk-application-apps.md)

## What to build

Three parts of ADR 0017 that 110 leaves out. Each can go into an earlier ticket where it falls naturally.

- **The application's name.** The applet shows the focused window's application name in bold before the menus. A preference of the applet turns it off, and it's on when the applet is first added. A window with no menu shows the name alone, or nothing when the name is off. Check what vala-panel-appmenu has already: it may need no patch.
- **A dialog shows its parent's menu.** When the focused window has no menu, Caminito publishes its transient parent's, and so on up, so the menus stay while a Find dialog is open.
- **The grace period.** When the applet's bus name loses its owner, Caminito keeps the registrar's name, the appmenu global and the capability for about ten seconds, and takes them away only if no owner has come back. Time comes in as an argument, as in policy.

## Acceptance criteria

- [ ] The preference shows and hides the name, on each applet by itself.
- [ ] With a menu-less dialog focused, the panel shows its parent's menu.
- [ ] Killing mate-panel in a Session doesn't put a menubar back in any GTK window while mate-session restarts it.
- [ ] Removing the applet puts the menubars back after the grace period.
- [ ] Headless tests for the parent's menu and for the grace period, with no sleep.

## Notes

- Later, and not this ticket: Alt+F and F10 opening the global menu.
