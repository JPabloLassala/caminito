# Map: Power applet

Status: closed on 2026-10-14. It reached its destination, [the spec](spec.md), in one grilling session with the developer and a prototype.

## Destination

A spec, ready to cut into implementation tickets, for a power popup in the panel in the manner of Plasma's Power and Battery.

## Notes

- **Domain:** the **Power applet** and **Stay awake** are in [CONTEXT.md](../../CONTEXT.md). The applet's shape follows [ADR 0035](../../docs/adr/0035-the-bluetooth-popup-is-an-in-process-mate-panel-applet.md), whose Consequences say so.
- **Not in a milestone.** It gates nothing of v0.1.
- **What there is today:**
  - mate-power-manager runs on XWayland in a Session (ADR 0014). Its tray icon is XEmbed, and mate-panel on Wayland hosts StatusNotifierItems only, so nothing shows the battery.
  - mpm's Brightness and Inhibit applets are out-of-process, so they likely don't load either.
  - Caminito owns idle, blanking and Powered off (128, 129). It respects mate-session's inhibitors and the Screensaver shim's.
  - mpm keeps the lid, power and suspend keys (107).
- **The developer's machine:**
  - power-profiles-daemon 0.30 on both bus names, and UPower 1.91.
  - Two batteries (BAT0 and BAT1), and a Logitech mouse that reports its battery.
  - No ACPI platform profile. All three profiles come from `intel_pstate`.

## Decisions so far

From the grilling session of 2026-10-14:

- **An applet, not a new settings window.** "Configure Power Management…" opens mate-power-preferences.
- **In the popup:**
  - The power profile, with the reason Performance is degraded and who holds a profile.
  - The laptop's batteries, with health and a charge-limit switch for each.
  - Peripherals' batteries.
  - **Stay awake**, and the applications keeping the computer awake.
- **Not in the popup:** the lid note (Caminito has no external-monitor rule, and the lid is mpm's) and brightness (another applet).
- **Peripherals' batteries are in both** the Bluetooth popup and this one.
- **Shape as in ADR 0035:** a C and GTK3 in-process applet in `helpers/`, reading UPower, power-profiles-daemon and mate-session over D-Bus. No Rust half, and no code shared with the Bluetooth applet.
- **Stay awake** holds mate-session's `Inhibit` (suspend and idle), from the applet's own process.
- **Profiles:** two or three, never more. Performance is hidden when the daemon doesn't list it. The section is hidden with no daemon.
- **The icon** is the battery glyph, with an optional percentage beside it that is off by default, or a plug with no battery.
- **mpm's own icon** is off by default through a gsettings override, so X11 MATE doesn't show two.
- **Design:** the prototype's "Popup C", with Stay awake as a switch row rather than a tile. The prototype is on the `prototype/power-applet-ui` branch.
- **The epic is `power-applet`,** tickets 262 to 266.
