# Map: Bluetooth

Status: closed on 2026-10-14. It reached its destination, [the spec](spec.md), in one grilling session with the developer.

## Destination

A spec, ready to cut into implementation tickets, for Bluetooth on a MATE desktop: a panel popup like Plasma's, a settings window, and a pairing agent.

## Notes

- **Domain:** **caminito-bluetooth** and the **Bluetooth applet** are in [CONTEXT.md](../../CONTEXT.md). The split between them is [ADR 0035](../../docs/adr/0035-the-bluetooth-popup-is-an-in-process-mate-panel-applet.md).
- **Not in a milestone.** It gates nothing of v0.1.
- **What there is today:** MATE has no Bluetooth front end, and `caminito-desktop` depends on none, so nothing in a Session can pair a device. `bluez` is installed. No crate in the workspace speaks D-Bus yet.
- **What the panel allows:** mate-panel is a layer surface, and on Wayland it loads in-process applets only (ADR 0017). Its notification area hosts StatusNotifierItems, but a status-notifier click gives a menu, not a popup. No program in this repository owned a panel applet before this one.

## Decisions so far

From the grilling session of 2026-10-14:

- **Two pieces, one API:** a C and GTK3 in-process applet for the icon and popup, and `caminito-bluetooth` in Rust and GTK4 for the settings window and pairing agent. They share only BlueZ's D-Bus API (ADR 0035).
- **Any MATE desktop,** not only a Session: the entries go in the normal XDG directories.
- **The agent** is BlueZ's default agent, in the long-running `caminito-bluetooth`, which XDG autostart starts with `--background`. It's a single-instance `GtkApplication`.
- **The popup copies Plasma's,** with battery levels. "Add New Device…" opens the window's scan page. With no adapter, the icon is hidden.
- **Off is an rfkill soft block,** so it lasts across reboots.
- **The window** has three parts: This computer, Devices and Add New Device. Discoverable is on only while the window is open. There are no trust or block toggles.
- **gio's D-Bus** in Rust, not zbus.
- **No notifications, no OBEX file transfer and no audio profiles** in v1.
- **Blueman can live alongside it.** The README says so.
- **In the default panel layout.** It ships for Arch and Debian.
- **Tests:** python-dbusmock's `bluez5` template for the Rust side, and check scripts for the applet.
- **The epic is `bluetooth`,** tickets 255 to 261.
