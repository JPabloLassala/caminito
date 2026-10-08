# The global menu of Qt apps and Firefox shows in the panel

Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [110](110-global-menu-for-gtk-application-apps.md)

## What to build

Qt 6 and Firefox give a surface a dbusmenu address through `org_kde_kwin_appmenu` (ADR 0017). The protocol's XML is in `/usr/share/qt6/wayland/protocols/appmenu/appmenu.xml`.

- **Caminito implements `org_kde_kwin_appmenu_manager`** and keeps each window's bus name and object path.
- **The global, the registrar's name and the GTK capability come and go together,** while an applet is present. A Qt app that sees the registrar's name without the global has no menu anywhere (read from `qmenubar.cpp`, not run).
- **The published property carries the dbusmenu kind,** and the applet's Caminito backend hands it to vala-panel-appmenu's dbusmenu importer.
- **Document** Firefox's two prefs, `widget.gtk.global-menu.enabled` and `widget.gtk.global-menu.wayland.enabled`, both off by default, and that it needs `libdbusmenu-glib` and `libdbusmenu-gtk3`. And that a Qt app or Firefox follows the applet only from its next start.

## Acceptance criteria

- [ ] In a Session with the applet on the panel, a Qt 6 app started after it shows its menu in the panel, and so does Firefox with both prefs on.
- [ ] Without the applet neither the global nor the registrar's name is there, and both apps keep their menubars.
- [ ] A headless test: a client's `set_address` reaches the published property when its window has the focus.

## Notes

- Qt's own themes all gate on the registrar's name the same way. qt6ct wasn't read.
- Firefox was read from `main`, not from the installed 157.
