# Map: Thermal applet

Status: closed on 2026-10-14. It reached its destination, [the spec](spec.md), in one grilling session with the developer and a prototype.

## Destination

A spec, ready to cut into implementation tickets, for a temperatures-and-fans popup in the panel in the manner of TG Pro.

## Notes

- **Domain:** the **Thermal applet** and **Panel sensor** are in [CONTEXT.md](../../CONTEXT.md). The applet's shape follows [ADR 0035](../../docs/adr/0035-the-bluetooth-popup-is-an-in-process-mate-panel-applet.md), whose Consequences say so.
- **Not in a milestone.** It gates nothing of v0.1.
- **What there is today:** nothing in MATE shows temperatures but mate-sensors-applet, which isn't installed and has no popup. lm_sensors is installed.
- **The developer's machine** is a ThinkPad (Lenovo 20MJ) with Intel graphics:
  - `coretemp` gives a Package reading and four cores, each with high = crit = 100 °C.
  - The `thinkpad` EC gives one fan (`fan1`, with no `fan1_max`, and `pwm1` meaningless under `auto`) and a CPU reading. Its "GPU" reads N/A, and temp3 to temp8 read +0.0 °C.
  - There are also `acpitz`, `pch_skylake` and `iwlwifi_1`.
  - There's no NVMe hwmon and no GPU temperature.
  - Its top panel is 24 px.

## Decisions so far

From the grilling session of 2026-10-14:

- **Only watching.** Fan control is out, because it needs a privileged helper.
- **libsensors** is the only source, from a C and GTK3 in-process applet (ADR 0035), with no settings window.
- **A new applet,** not a patch to mate-sensors-applet.
- **The panel is TG Pro's:** a thermometer, up to two panel sensors over the fastest fan's speed or `Fans Off`. It's one line below 32 px, and a bare number on a vertical panel.
- **Panel sensors** default to CPU and GPU, or the CPU alone. TG Pro's second number is a chassis temperature, which hwmon doesn't have.
- **Bands** are fixed at 75 and 90 °C, set in Preferences, with hot coming sooner for a reading whose crit is lower.
- **Bars** run on one common 20–100 °C scale. The colour carries the danger.
- **Dead readings** are hidden. Fans get a bar only against a max, or against the fastest seen once watched for a while.
- **Design:** the prototype's "Popup C" (tiles) with "Popup A"'s bars under "Show all sensors", which remembers whether it's open. Tiles don't respond to clicks. The prototype is on the `prototype/thermal-applet-ui` branch.
- **Testing:** umockdev, falling back to `CAMINITO_THERMAL_HWMON`.
- **Shared code:** the third applet brings `helpers/caminito-applet-common/`, shared with the Bluetooth and Power applets for the popup window and the icon fallbacks. ADR 0035's Consequences say so, and tickets 259 and 262 extract it or use it.
- **Not in the default panel.** It's in Add to Panel only.
- **The epic is `thermal-applet`,** tickets 267 to 270.
