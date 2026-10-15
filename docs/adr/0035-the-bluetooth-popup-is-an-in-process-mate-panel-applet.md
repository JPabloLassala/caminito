# 35. The Bluetooth popup is an in-process mate-panel applet, and BlueZ is the only thing it shares

Date: 2026-10-14
Status: Accepted

## Context

MATE has no Bluetooth front end of its own. The developer wants one like Plasma's: a panel icon whose popup lists the devices, with a switch for Bluetooth, expanding rows to connect and disconnect, and battery levels. Behind the popup are a settings window and a pairing agent.

Caminito's own programs are Rust and GTK4 (ADR 0014). But on Wayland, mate-panel loads in-process applets only, so a program can't embed its own window in the panel. A popup that belongs to the panel has to come from code running inside mate-panel, and that code is GTK3.

## Decision

- **The Bluetooth applet is a C and GTK3 in-process mate-panel applet,** built as a Meson subproject under `helpers/`. It is thin: the icon, its tooltip, the popup and the right-click menu.
- **Everything else is `caminito-bluetooth`, in Rust:** the settings window and the pairing agent, in one long-running process started by XDG autostart.
- **The two share nothing but BlueZ's D-Bus API.** The applet reads and acts on BlueZ directly through GDBus, and `caminito-bluetooth` does the same through gio. There's no private interface between them. The applet's only call into `caminito-bluetooth` is starting it, to open or raise the window.
- **The agent never runs in the panel's process.** A pairing prompt or a crash in the agent doesn't take the panel down.

## Consequences

- C and GTK3 are a second language and toolkit in Caminito's own code. The applet stays small to keep that cost small.
- Logic that both halves need, such as listing devices, connecting, and reading the battery, is written twice. It's thin over BlueZ, and keeping a private API in step across the two would cost more.
- The applet works under X11 MATE too, since it doesn't depend on Caminito.
- The Power applet (2026-10-14, epic `power-applet`) follows the same shape for the same reasons: a thin C and GTK3 in-process applet that reads UPower, power-profiles-daemon and mate-session directly. It has no Rust half, because mate-power-preferences is its settings window. The two applets share no code until a third one appears.
- The Thermal applet (2026-10-14, epic `thermal-applet`) follows the same shape: a thin C and GTK3 in-process applet that reads libsensors directly. It has no Rust half and no settings window, because it only watches. Setting fans' speeds would need a privileged helper, which is why that's left out.
- With the Thermal applet as the third, the three applets share `helpers/caminito-applet-common/`. It's a static library with only the plumbing they all need: the popup window placed against the applet's button, left-click toggling it and losing focus closing it, and themed icons with fallbacks. Whichever applet is built first extracts it, and the others use it. Each applet's sources, and its logic over D-Bus or libsensors, stay its own.
- **The applet is in a new session's top panel through a layout of Caminito's own** (CMN-105), not a carried mate-panel patch. `data/caminito.layout` is mate-panel's `default.layout` with the applet beside the notification area, and `90_caminito.gschema.override` makes it `org.mate.panel`'s `default-layout`. Both are in `caminito`'s package, with the applet the layout names, so a layout never names an applet that isn't installed, and no patch is rebased with the baseline. The cost is a copy of `default.layout` to keep in step by hand, and a new user of X11 MATE on the same machine gets the layout too: the applet works there, and hides itself with no adapter. Only a user with no panel settings gets it. The Power and Thermal applets, which follow, can add their lines to this file.
- Rejected:
  - A StatusNotifierItem icon from the Rust process. It's cheap and all Rust, but its click gives a dbusmenu menu that the panel draws, with no switches, expanding rows or custom layout.
  - A layer-shell popup drawn by the Rust process. It works only under Caminito, needs a privileged client, and can't be placed against the icon, because the panel doesn't publish where the icon is.
  - An applet in Rust. The GTK3 bindings are unmaintained, as ADR 0017 found.
  - A private D-Bus API from `caminito-bluetooth` that the applet merely views. It would need versioning across two packages, and the popup would go empty whenever the process wasn't running.
