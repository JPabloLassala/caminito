# 17. Caminito brokers the global menu, and a patched vala-panel-appmenu draws it

Date: 2026-10-07
Status: Accepted. Qualified on 2026-10-10 by ticket 111: classic GTK apps need the patched `appmenu-gtk-module-caminito`, not the stock module, and no gschema override. Qualified on 2026-10-29 by ADR 0053: `caminito-firefox-global-menu` sets Firefox's two prefs, for a user who installs it.

## Context

The developer wants a global menu in the panel for Firefox, GTK apps, Qt apps and VS Code. Marco has none, so this isn't parity work, and it comes after v0.1.

Apps publish a menubar through three channels, and each ties the menu to a window:

- `org_kde_kwin_appmenu`, a Wayland protocol: Qt 6 and Firefox give a surface a bus name and an object path that serve `com.canonical.dbusmenu`.
- `gtk_surface1.set_dbus_properties`: a GTK3 or GTK4 `GtkApplication` gives a surface the paths of its `org.gtk.Menus` and `org.gtk.Actions`. `appmenu-gtk-module` does the same for a GTK3 app that builds a classic `GtkMenuBar`, which is most of MATE's.
- `com.canonical.AppMenu.Registrar`, on the session bus: an X11 client calls `RegisterWindow` with its window id and a dbusmenu path. Electron exports its menu no other way.

Only the compositor can tell which surface or X11 window is the focused one. And three things make an app give up its own menubar: an owner of the registrar's name, which Qt, Firefox, Electron and `appmenu-gtk-module` all look for even where they never call it; the appmenu global; and gtk_shell's `global_menu_bar` capability. Qt looks for the registrar once in each process, and Firefox hides its menubar once in each window. Only GTK apps follow a change while they run.

On Wayland mate-panel loads in-process applets only.

## Decision

- **Caminito is the broker.** It takes all three channels, keeps each window's menu, and publishes the focused window's on `org.mate.Caminito` as one property that tells when it changes: the kind of menu, where it is on the bus, and the application's name. It publishes no other window's menu, no window id and no surface (ADR 0002).
- **Caminito owns `com.canonical.AppMenu.Registrar`.** It already knows each XWayland window's id, and the name has to come and go with the other two.
- **The focused window is never the panel or one of its popups.** A window with no menu of its own shows its transient parent's.
- **A global menu applet on a panel is the switch.** The applet owns a well-known bus name, and Caminito watches it. While it has an owner Caminito owns the registrar's name, offers the appmenu global and sends the capability, always the three together. The menu claim, a fourth channel added by ADR 0018, follows the same switch. When the owner goes Caminito waits about ten seconds for it to come back, so that a restart of the panel changes nothing, and then takes all three away.
- **The applet is vala-panel-appmenu's MATE applet with a carried patch** that adds a Caminito backend beside its Wayfire one, in a patched package `vala-panel-appmenu-mate-caminito` (ADR 0011). The backend is used when `org.mate.Caminito` has an owner.
- **Classic GTK apps come through `appmenu-gtk-module`,** as a patched package `appmenu-gtk-module-caminito` (ADR 0011), an optional dependency. The stock module tells a Wayland compositor nothing for most of MATE's apps, and keeps pluma and atril from showing a window while nothing owns the registrar's name (111). The session adds the module to its command's `GTK_MODULES` when the patched package is installed. The module's `run-on-wayland` key is on by default, so the packaging doesn't set it.
- **Firefox's two prefs and VS Code's settings are the user's to set,** and documented. VS Code has a global menu only on XWayland (`--ozone-platform=x11`) with `window.titleBarStyle` set to `native`. On native Wayland with the custom title bar it needs an extension and the menu claim (ADR 0018).
- **Every applet shows the one focused window's menu,** whatever panel or output it's on. Whether the application's name is shown before the menus is a preference of the applet, on by default.

## Consequences

- An app that starts before the panel has loaded the applet, as a restored or autostarted one can at login, keeps its own menubar until it's restarted if it's a Qt app, Firefox or an Electron app. The same holds the other way when the applet is removed. This was accepted knowingly, over a settings key.
- Removing the applet takes about ten seconds to show in GTK apps.
- With the menubar out of the window, Alt+F and F10 don't open it. Routing them to the applet is left to a later ticket: it needs Caminito to grab the mnemonics and to let the panel open a popup with no input serial.
- VS Code on XWayland is drawn at the X11 scale (ADR 0008), so it's soft on an output with a fractional scale.
- An Electron app on native Wayland takes the appmenu global to mean it may use a global menu, and then registers an id that isn't an X11 window's. Whether it loses its menu wasn't run. It has to be checked before the appmenu global ships.
- The patched package's baseline is the AUR's, not Arch's, which no patched package had before. The backend could be offered upstream, as Wayfire's was. Until the developer decides to, it's a carried patch.
- Rejected:
  - The applet finding the focused window's menu itself (it can't for a menu tied to a surface).
  - An `org.mate.Caminito` key, read at start-up and set by the applet, as the switch (no race at login, but a second place that says whether the applet is there).
  - Advertising always (no menubar anywhere for a user without the applet).
  - Another process as the registrar, vala-panel-appmenu's or kded's (a start-up order to get right, and a name Caminito couldn't take away).
  - A new applet in C in this repository (every quirk of drawing two menu formats would be ours to find).
  - A new applet in Rust (the GTK3 bindings are unmaintained, and it would run inside the panel's process).
  - Forcing VS Code onto X11 or setting Firefox's prefs from the packaging (other programs' defaults aren't the patched packages' to change).
