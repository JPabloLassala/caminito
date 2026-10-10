# The application's name, a dialog's parent menu, and the panel's restarts

Epic: global-menu
Type: implementation
Status: open: the code is in and headless-tested; the four Session criteria are the developer's, by a nested run of 114-checks/run.sh and a check in a MATE Session
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
- [x] Headless tests for the parent's menu and for the grace period, with no sleep.

## Notes

- Later, and not this ticket: Alt+F and F10 opening the global menu.

## Done (2026-10-10)

- **All three parts were already on trunk when this ticket was shipped,** folded into 110, 112, 113 and 115 as the ticket allowed: `Switch` in `caminito/src/dbus/global_menu.rs`, which takes `now` and holds the registrar's name, the appmenu global and the capability for `GRACE` (10 s) after the applet's name loses its owner; `Caminito::global_menu`, which walks policy's parents until a window has a menu; and the carried patch `0002-applet-Let-the-application-s-name-be-turned-off-unde.patch`. This ticket's commit adds one test and closes the gaps in checking it.
- **vala-panel-appmenu 25.04 has no way to hide the name,** only `bold-application-name`, so the patch is needed. Under Caminito it makes the name always bold and puts `show-application-name` (default true, per applet through `MatePanel.AppletSettings`) in that preference's place. A window with no menu is the backend's `STUB` model, the name alone, and with the name off nothing. The widget restocks on any property's `notify`, so the preference takes effect without a restart.
- **The X11 dialog got a test of its own,** `app_menu_registrar::a_dialog_without_a_registered_menu_has_its_parents`: the xdg tests covered the walk up, but an XWayland parent's menu comes from the registrar. It passed at once, the behaviour being in already. The grace period's headless tests (`global_menu::the_capability_stays_for_the_grace_period_after_the_applet_leaves`, `a_restart_of_the_panel_within_the_grace_period_changes_nothing`, `the_appmenu_global_and_the_registrars_name_follow_the_applets_bus_name`, and `Switch`'s unit tests) give Caminito the time its timer would, with no sleep.
- **The patched applet was built** from the AUR's 25.04 tag with all three carried patches and the PKGBUILD's meson options, in an Ubuntu 24.04 container against MATE 1.26's `libmate-panel-applet`, not mate-panel-caminito's: it compiles and links, and the compiled schema gives a new applet `show-application-name` true. The patches' sums match the PKGBUILD's.
- **Not run:** `114-checks/run.sh` and the Session criteria (no Wayland desktop, mate-panel-caminito or MATE Session in the container), so nothing the applet draws was looked at. caminito's library tests ran under `dbus-run-session` with the golden fonts: 1234 pass, and 6 fail that this change doesn't touch and that fail the same way alone (4 `keymap` and one `window_properties` test on Ubuntu's xkeyboard-config and Xwayland 23.2, one `xwayland_restart` test under load only). `cargo clippy -p caminito --no-deps` is clean; the container's clippy 0.1.97 flags three manual `RangeInclusive::contains` in caminito-theme, which this ticket didn't change.
