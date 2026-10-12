# Ubuntu MATE's X11-only autostarts in mate-session's lists

Epic: debian-packaging
Type: research
Status: open: the code is in; building it and a login on Arch are the developer's
Milestone: none: after v0.1
Blocked by: 191
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "Caminito and the patches" in [the spec](../debian-packaging/spec.md), and ADR 0014.

- List the `/etc/xdg/autostart` entries that `ubuntu-mate-desktop` and its recommends install on Ubuntu 26.04, from the archive's Contents. `mate-hud`, `plank` and the indicator applets are the likely ones.
- For each, decide by the ADR's rule: skipped under Caminito, moved to XWayland, or left alone because it works on Wayland. Record it in this ticket.
- Add the decisions to the series' `0003` (skipped) and `0004` (on XWayland) in `packaging/patches/mate-session-manager/`. They're gated on Caminito, as the rest is.
- Build `mate-session-manager-caminito` on Arch with the new series. An id Arch doesn't install changes nothing there.

## Acceptance criteria

- [x] The list and each decision are in this ticket, with the package each entry comes from.
- [ ] The patched series applies, and its tests, if any, pass. The session runs on Arch with nothing changed, by the developer. *The series applies and its check passes; the Arch build and login are the developer's.*

## Answer

Read on 2026-10-12 from the archive's `resolute` and `resolute-updates` indices (`main`, `universe`, amd64) and `Contents-amd64.gz`. The install is `ubuntu-mate-desktop` 1.308 with its depends and recommends, recursively, as apt installs them, an alternative already chosen counting as met: 1,954 packages. Each package that installs an entry in `/etc/xdg/autostart` or `/usr/share/mate/autostart` was downloaded and its entry and program read: `OnlyShowIn`, `NotShowIn` and conditions, and whether the program calls X11 (`nm -D` for `gdk_x11_*`, Xlib and libwnck; the source for Python and shell). Under a Session, `XDG_CURRENT_DESKTOP` is `MATE`, `DISPLAY` is XWayland's and `GDK_BACKEND` is unset (0001), so a plain Xlib or xcb program reaches XWayland by itself, and a GDK one opens on Wayland.

`mate-hud` and `plank` were the likely ones and are in. The indicator applets aren't: they're GLib services with no display.

### Skipped (0003)

| Entry | Directory | Package | Why |
| --- | --- | --- | --- |
| `mate-hud.desktop` | `/usr/share/mate/autostart` | `mate-hud` 22.10.3 | Searches the focused window's menu, found by python-xlib's `_NET_ACTIVE_WINDOW` and grabs on the X root. Caminito brokers the menus (ADR 0017), so on XWayland it would see XWayland's windows alone and grab Alt over them. Off by default in 26.04 (`org.mate.hud enabled=false`), but mate-tweak turns it on. |
| `plank.desktop` | none: it's the `dock` required component | `plank` 0.11.89 | A dock through libwnck and `gdk_x11_*`, with no check for the display. mate-tweak's dock layouts set `org.mate.session.required-components dock='plank'`, and mate-session starts it from `/usr/share/applications/plank.desktop` as an autostart entry of that id, so the list reaches it. mate-panel's Wayland tasklist lists the windows under Caminito; on XWayland plank would list XWayland's. |
| `dock-replace.desktop` | `/usr/share/mate/autostart` | `ubuntu-mate-default-settings` 26.04.0 | Ubuntu MATE's script that starts the dock again 15 seconds into the login. It starts plank outside mate-session, so plank's id doesn't cover it. |
| `picom.desktop` | `/etc/xdg/autostart` | `picom` 12.5 (a recommend of `mate-tweak`) | An X compositor, xcb throughout. Compositing is Caminito's. The entry has no `OnlyShowIn`. |
| `magnus-autostart.desktop` | `/etc/xdg/autostart` | `magnus` 1.0.3 | A magnifier that copies the X root window through `Gdk.pixbuf_get_from_window`; on Wayland it gets nothing. Magnifying is the compositor's (13). Only with `screen-magnifier-enabled`. |

### On XWayland (0004)

| Entry | Directory | Package | Why |
| --- | --- | --- | --- |
| `tilda.desktop` | `~/.config/autostart`, written by mate-tweak's pull-down terminal option | `tilda` 2.0.0 | A pull-down terminal that calls `gdk_x11_get_default_xdisplay` and `XGrabKey` unchecked, so it would crash on Wayland. Nothing stands in for it. Its key reaches it only while an XWayland window has the focus, as mate-power-manager's did before 107. |

`mate-power-manager.desktop` (`mate-power-manager` 1.26.1) and `mate-screensaver.desktop` (`mate-screensaver-common` 1.26.2) are in Ubuntu's install too, and were already in the lists. `mate-maximus-autostart.desktop` (`mate-netbook-common`) isn't in 26.04's install, and stays.

### Left alone

They work on Wayland, reach XWayland by themselves, or don't start in MATE.

| Entry | Package | Why |
| --- | --- | --- |
| `ayatana-indicator-{application,datetime,messages,notifications,power,printers,session,sound}.desktop` | `ayatana-indicator-*` | GLib services, no display. |
| `nm-applet.desktop` (both directories; Ubuntu's in `mate/autostart` wins) | `network-manager-applet`, `ubuntu-mate-default-settings` | GTK3; `gdk_x11_display_get_type` only as a type check. |
| `blueman.desktop` | `blueman` | GTK3 Python; its X11 plugin checks for an X11 screen first. |
| `polkit-mate-authentication-agent-1.desktop` | `mate-polkit-common` | GTK3, as on Arch. |
| `mate-settings-daemon.desktop` | `mate-settings-daemon` | Its own carried patches. |
| `mate-optimus.desktop` | `mate-optimus` | GTK3 Python, an AppIndicator. |
| `onboard-autostart.desktop` | `onboard` | Runs on Wayland over uinput (13). Only with `screen-keyboard-enabled`. |
| `orca-autostart.desktop` | `orca` | AT-SPI; Caminito serves its keyboard monitor (13). Only with `screen-reader-enabled`. |
| `spice-vdagent.desktop` | `spice-vdagent` | Plain Xlib on `DISPLAY`, so XWayland already, and it allows GDK's `wayland,x11`. |
| `at-spi-dbus-bus.desktop` | `at-spi2-core` | Sets its X root property only when an X display opens. |
| `gnome-keyring-{pkcs11,secrets}.desktop`, `xdg-user-dirs.desktop`, `user-dirs-update-gtk.desktop`, `snap-userd-autostart.desktop`, `ubuntu-report-on-upgrade.desktop`, `im-launch.desktop`, `geoclue-demo-agent.desktop`, `org.gnome.DejaDup.Monitor.desktop`, `org.gnome.Evolution-alarm-notify.desktop`, `update-notifier.desktop`, `ubuntu-advantage-notification.desktop` | `gnome-keyring`, `xdg-user-dirs`, `xdg-user-dirs-gtk`, `snapd`, `ubuntu-report`, `im-config`, `geoclue-2.0`, `deja-dup`, `evolution-data-server`, `update-notifier` | No display, or GTK3 with no X11 calls. |
| `ubuntu-mate-settings-overlay.desktop`, `caja-dropbox-autostart.desktop` | `ubuntu-mate-default-settings` | Shell: copies settings, or starts Dropbox if it's installed. |
| `org.gnome.SettingsDaemon.DiskUtilityNotify.desktop` | `gnome-disk-utility` | `OnlyShowIn=GNOME`. |
| `print-applet.desktop` (Ubuntu's in `mate/autostart` wins), `ayatana-indicator-keyboard.desktop` | `system-config-printer`, `ubuntu-mate-default-settings` | `NotShowIn=MATE`. |

## Done (2026-10-12)

- **The check came first.** `206-checks/autostart-check.sh` clones the PKGBUILD's pinned commit, applies the series with `patch -Np1` as `prepare()` does, builds it, and starts `mate-session` twice on Xvfb under `dbus-run-session`: with `MATE_WM_NAME=Caminito` and without. Every listed id is an entry recording the `GDK_BACKEND` it got, beside a control; plank is the `dock` required component through a schema override. Against the old series it failed for the six new ids alone, and it passes with the new one. Without the variable, all ten start as before.
- **Run in an Ubuntu 24.04 cloud container,** with the build's dependencies from apt and mate-desktop's schemas from `mate-desktop-common`. No Arch build and no login: `makepkg -si` and a Session on a tty are the developer's, and the second criterion stays open for them. Remove makepkg's old copies of the patches first (191).
- **plank goes in by its id, not by a check of the `dock` component.** mate-session starts a required component as an autostart entry named after its `.desktop` file, so `plank.desktop` in the skipped list covers it, and a user's `~/.config/autostart/plank.desktop` too. Patch 0002's way, skipping the component in `main.c`, would have skipped any dock, Wayland ones included.
- **mate-hud is skipped, not moved.** It's X11-only and nothing does its search, which is the second list's rule, but its work is the focused window's menu, which Caminito owns under a Session, as the first list's rule has it. A HUD over Caminito's menu broker would be new work; no ticket was filed.
- **tilda's entry is the user's, not a package's.** mate-tweak writes it to `~/.config/autostart` when the pull-down terminal is turned on. The lists are by id, so it's covered wherever it's read from (ADR 0014).
- **The patches were rewritten in a git tree and exported with `format-patch --zero-commit --no-signature`,** so 0003 and 0004 have new subjects and file names. The PKGBUILD names them, with new `b2sums`, at `pkgrel=4`. ADR 0014 has a qualifying line.
- **`tools/check` wasn't run whole.** In the container, `rustfmt` passed and `clippy` stopped at `gdk4-sys`, which wants the Arch machine's gtk4. The change touches no code of the tree's own, only the carried series.
