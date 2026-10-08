# The global menu of MATE's own apps shows in the panel

Epic: global-menu
Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [110](110-global-menu-for-gtk-application-apps.md)

## What to build

Most of MATE's apps build a classic `GtkMenuBar` and export nothing. `appmenu-gtk-module` (Arch `extra`, 25.04) exports one as `org.gtk.Menus` through the same `set_dbus_properties` that 110 reads (ADR 0017).

- **Try it first.** Whether it works with pluma, caja, atril and mate-terminal on Wayland wasn't run. If it doesn't, stop and take it back to the developer: the other way is a carried patch.
- **`caminito-git` takes it as an optional dependency.** The session adds it to `GTK_MODULES` when it's installed.
- **The packaging ships a gschema override** that turns `org.appmenu.gtk-module`'s `run-on-wayland` on.
- **The module hides menubars by the registrar's name,** not by gtk_shell's capability (`support.c`). So Caminito has to own `com.canonical.AppMenu.Registrar` while an applet is present, from this ticket on. `RegisterWindow` itself is [113](113-global-menu-for-xwayland-windows.md)'s.

## Acceptance criteria

- [ ] In a Session with the applet on the panel, pluma, caja, atril and mate-terminal show their menus in the panel, and the items work.
- [ ] Without the applet, and without the module installed, each keeps its menubar in the window.
- [ ] The module changes nothing in X11 MATE or the Wayfire session that it didn't change before.

## Notes

- **Tried by hand on 2026-10-08,** in a Session, with Arch's 25.04-3 unpacked beside the system and loaded through `GTK_MODULES`, a throwaway process owning the registrar's name, and `WAYLAND_DEBUG=1`:
  - atril sends `set_dbus_properties` with a menubar path, a window path, an application path and its bus name. Whether the menu it serves is right wasn't looked at.
  - mate-calc and mate-terminal send none. Each logs `gdk_wayland_window_set_dbus_properties_libgtk_only: assertion 'GDK_IS_WAYLAND_WINDOW (window)' failed`, so the module calls it on a window that isn't a Wayland one yet.
  - pluma and caja weren't tested: each was already running and the new process handed over to it.
  - `run-on-wayland` is already on by default in this build, so the override may not be needed.
  - So the module as packaged covers only some of MATE's apps. The decision this ticket asks for is the developer's: a fix to the module (an upstream fix, or the AUR's newer `appmenu-gtk-module-wayland` 26.08.03, not tried), or another way.

- **Built on 2026-10-08** (`227d586`), after the developer said to continue, which was taken as choosing a patched package over waiting for upstream:
  - `appmenu-gtk-module-caminito` is Arch's 25.04 with the fix below, as an upstream fix (ADR 0011). It's an optional dependency of `caminito-git`. The fix is still to be sent upstream.
  - The session adds the module to its command's `GTK_MODULES` only when that package is in pacman's database. With the stock one a plain window would have no menu at all while the applet is there.
  - The registrar's name was already 112's. No gschema override: the default is on.
  - Still to do: the acceptance criteria in a Session, with the applet and the package installed. ADR 0011 and ADR 0017 don't name the package yet, and 0017 still promises the override.
  - An XWayland GTK 3 app the session starts loads the module too. Whether its menu then reaches the applet through 113 wasn't run.
- **Tried again on 2026-10-08,** from the 25.04 source built beside the system, each app on a bus of its own (`dbus-run-session`) with a throwaway owner of the registrar's name, so pluma and caja started fresh:
  - The cause is in `hijacked_window_realize` (`src/hijack.c`). For a window that isn't a `GtkApplicationWindow` it makes the window's data before it chains up to the realize, so `gtk_widget_get_window` is still NULL when `platform.c` passes it to `gdk_wayland_window_set_dbus_properties_libgtk_only`. The data is kept, so it's never tried again.
  - As packaged: atril and pluma send a menubar path (their windows are `GtkApplicationWindow`s, and GTK sends it). caja sends its application id and path with no menubar. mate-calc and mate-terminal fail the assertion.
  - With the call moved after the chain-up, all five send a menubar path, and `org.gtk.Menus.Start` on it answers with the app's own menus (File, Edit, View; Calculator, Mode, View). Nothing else was changed:

    ```diff
    --- a/subprojects/appmenu-gtk-module/src/hijack.c
    +++ b/subprojects/appmenu-gtk-module/src/hijack.c
    @@ -78,16 +78,16 @@
     	if (visual && (hint == GDK_WINDOW_TYPE_HINT_DND))
     		gtk_widget_set_visual(widget, visual);
     
    -// In Wayland the DBUS Menu need to be register before realize the window.
    +	if (pre_hijacked_window_realize != NULL)
    +		pre_hijacked_window_realize(widget);
    +
    +// A plain GtkWindow has no GdkWindow to set the D-Bus properties on until it's realized.
     #ifdef GDK_WINDOWING_WAYLAND
     	if (GDK_IS_WAYLAND_DISPLAY(gdk_display_get_default()) && is_hint_viable &&
     	    (!GTK_IS_APPLICATION_WINDOW(GTK_WINDOW(widget))))
     		gtk_window_get_window_data(GTK_WINDOW(widget));
     #endif
     
    -	if (pre_hijacked_window_realize != NULL)
    -		pre_hijacked_window_realize(widget);
    -
     #ifdef GDK_WINDOWING_X11
     	if (is_hint_viable
     #if GTK_MAJOR_VERSION == 3
    ```

  - Not run: the menus in the applet, whether the items work, and whether the menubar leaves the window. Those need 110's applet. Under X11 the patch changes nothing the module does: the X11 branch already runs after the chain-up.
  - Upstream's master has the same order in `hijack.c`, and `platform.c` is unchanged from 25.04. Upstream had commits on 2026-10-07 and 2026-10-08, the Wayfire backend among them, so a fix offered there may land.
  - `run-on-wayland` defaults to true in the schema of 25.04's source, so the override isn't needed.
- The AUR's `appmenu-gtk-module-wayland` 26.08.03 is another project (guiodic's, "PoC: GTK global menu on Plasma Wayland"). It turns the menubar into a dbusmenu with `libdbusmenu-gtk3` and gives it to the surface through `org_kde_kwin_appmenu`, which is [112](112-global-menu-for-qt-apps-and-firefox.md)'s channel and not 110's. It's no fix for this ticket.
- GTK4 has no modules, so a GTK4 app that isn't a `GtkApplication` with a menubar has no global menu.
