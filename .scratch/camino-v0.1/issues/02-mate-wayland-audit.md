# MATE components on Wayland audit

Type: research
Status: open
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

Which MATE components (1.28 and later, plus current git) already run on Wayland, and which Wayland protocols does each one expect from the compositor? Cover mate-panel and its applets (the window list, workspace switcher and notification area in particular), mate-session, mate-settings-daemon, caja's desktop and background, mate-notification-daemon, mate-screensaver, mate-display-properties, mate-control-center's window/keyboard/mouse capplets, and mate-polkit. Also: how does the existing Wayfire-based MATE session launch and wire things together? Output: a component × protocol table showing working, partial or X11-only, with a source link for each.
