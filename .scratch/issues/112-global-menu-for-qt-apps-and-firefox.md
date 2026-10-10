# The global menu of Qt apps and Firefox shows in the panel

Epic: global-menu
Type: implementation
Status: open: the code is in, and the Session runs of a Qt 6 app and of Firefox, with the applet and without it, are the developer's
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
- [x] A headless test: a client's `set_address` reaches the published property when its window has the focus.

## Notes

- Qt's own themes all gate on the registrar's name the same way. qt6ct wasn't read.
- Firefox was read from `main`, not from the installed 157.

## Done (2026-10-10)

- **The code was already on the trunk** when this ticket was shipped, in `67e7390`, the commit that brought in the global menu's history: `protocols/appmenu.rs`, the vendored `caminito/resources/appmenu.xml`, the switch in `dbus/global_menu.rs`, the applet's Caminito backend (`packaging/arch/vala-panel-appmenu-mate-caminito/0001-…`) and the README's *The global menu*. Shipping it was checking each part of *What to build* against that code, and running the tests. Nothing was changed but this file.
- **Each window's address is kept in its surface's data map,** with the `org_kde_kwin_appmenu` that set it, so a released object takes only its own address away. An address with an empty bus name or path is none at all. The window's menu is read from it when the global menu is next published, so the protocol has no handler trait.
- **A dbusmenu address wins over the GTK menus of the same window,** for Firefox with `appmenu-gtk-module` loaded, whose hidden `GtkMenuBar` has nothing in it (`a_dbusmenu_address_wins_over_the_gtk_menus_of_the_same_window`).
- **The global, the registrar's name and the capability follow one switch,** `global_menu_offered`, which outlasts the applet by 114's grace period. The headless tests `the_appmenu_global_and_the_registrars_name_follow_the_applets_bus_name` and `a_dbusmenu_address_reaches_the_property_when_its_window_has_the_focus` meet the second criterion's half without the apps and the third criterion. All 19 of `tests::global_menu` pass.
- **The global is version 1, as the vendored XML is.** It's Qt 6.11.2's copy. plasma-wayland-protocols' `master` is at version 2, which adds only the manager's `release`, so a client of version 2 binds version 1 and loses nothing.
- **Not run here** (a cloud container, with no Session): a Qt 6 app and Firefox with the applet on the panel, and both keeping their menubars without it. They're the developer's, on a Session with `vala-panel-appmenu-mate-caminito` installed.
- **The full suite, in the container,** passed but for tests the container can't run: 4 in `tests::keymap` (no MATE keyboard schemas), `the_pointer_goes_through_an_override_redirect_windows_input_shaped_shadow` (Ubuntu's Xwayland 23.2), and `caminito-theme`'s goldens (342 cases differ by a few pixels of text, from Ubuntu's Pango). `night_light::the_property_is_signalled_when_the_target_changes` failed once with the machine under load and passed alone. The branch changes no code, so they fail the same on the trunk.
