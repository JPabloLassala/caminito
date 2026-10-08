# The global menu of a GtkApplication app shows in the panel

Epic: global-menu
Type: implementation
Status: resolved
Milestone: none: after v0.1
Blocked by: none

## What to build

The first slice of the global menu (ADR 0017), end to end on the one channel where apps follow a change while they run.

- **Caminito keeps each window's GTK menu.** `gtk_surface1.set_dbus_properties` is received and discarded today (`protocols/gtk_shell.rs`). Keep the bus name and the menubar, app menu, window and application object paths with the window.
- **Caminito publishes the focused window's menu** on `org.mate.Caminito`: one property with the kind of menu, where it is on the bus and the application's name, with a change notification. The panel and its popups are never the focused window. No other window's menu, no window id, no surface.
- **The applet is the switch.** It owns a well-known bus name, which this ticket chooses. While the name has an owner Caminito sends `gtk_shell1.capabilities` with `global_menu_bar` (bit 2) to every client, and without it when the owner goes. The grace period is [114](114-app-name-transients-and-the-grace-period.md)'s.
- **`vala-panel-appmenu-mate-caminito`,** a patched package on the AUR's `vala-panel-appmenu-mate` 25.04, with a carried patch that adds a Caminito backend beside `lib/appmenu-wayfire.vala`. It's used when `org.mate.Caminito` has an owner. The `.mate-panel-applet` file needs `InProcess=true` and `Wayland` in `Platforms`, and the applet links the patched panel's `libmate-panel-applet` (`.so.2`).

## Acceptance criteria

- [x] In a Session with the applet on the panel, a GTK3 and a GTK4 `GtkApplication` app with a menubar show it in the panel and not in the window, and its items work.
- [x] Focusing another window changes the menu. Opening a menu in the applet doesn't blank it.
- [x] Removing the applet puts the menubar back in the window without a restart.
- [x] Under X11 MATE and the Wayfire session the patched applet behaves as the stock one.
- [x] Headless tests through the fixture: the capability follows the bus name, and the property follows the focus.

## Notes

- The rendering of `org.gtk.Menus` is vala-panel-appmenu's already. Only the backend is new.
- Not run: whether the applet's wnck backend, which the patch leaves for X11, loads at all in a Wayland panel.
