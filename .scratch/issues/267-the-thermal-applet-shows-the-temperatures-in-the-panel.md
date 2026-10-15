# The Thermal applet shows the temperatures in the panel

Epic: thermal-applet
Type: implementation
Status: ready-for-agent
Blocked by: None
Spec: [Thermal applet](../thermal-applet/spec.md)

## What to build

The **Thermal applet** (CONTEXT.md) without its popup's contents, under "The applet", "Its sources", "Bands" and "The panel" in [the spec](../thermal-applet/spec.md), and [ADR 0035](../../docs/adr/0035-the-bluetooth-popup-is-an-in-process-mate-panel-applet.md).

- `helpers/caminito-thermal-applet/`: an in-process mate-panel applet in C and GTK3, as a Meson subproject of the root `meson.build`, with its `.mate-panel-applet` file. It's called "Temperatures and Fans" in Add to Panel.
- **`helpers/caminito-applet-common/`**, the static library the Bluetooth, Power and Thermal applets share (ADR 0035): the popup window placed against the applet's button, left-click toggling it and losing focus closing it, and themed icons with fallbacks. If it doesn't exist yet, this ticket extracts it. If it does, this ticket uses it.
- **Readings** come from libsensors every 2 s, or 1 s while the popup is open. Each is grouped and named as the spec says. Dead readings are hidden, and battery, AC and USB-C chips are left out.
- **Bands:** normal, warm from 75 °C, and hot from 90 °C or from crit minus 5 when that's lower. The thresholds are fixed in this ticket; 269 makes them preferences.
- **The panel item:**
  - the thermometer glyph
  - the default **panel sensors** (CONTEXT.md): the CPU's reading, and the hottest GPU reading when there's a GPU
  - each temperature in its band's colour
  - the fastest fan's speed or `Fans Off` under them
  - one line below 32 px, and a bare number on a vertical panel
- **The tooltip** is every panel sensor and every fan.
- **Left-click** toggles an empty popup that 268 fills. **Right-click** has Preferences (empty until 269) and About.
- **The check harness:** find out whether libsensors reads a umockdev-faked `/sys/class/hwmon`. If it does, the checks use umockdev. If it doesn't, add `CAMINITO_THERMAL_HWMON=<dir>`, which makes the applet read a fake tree directly, for tests only. Record which in the spec's Testing section.

## Acceptance criteria

- [ ] A check script with a fake ThinkPad tree (one fan with no max, no GPU, six readings at 0) screenshots the panel item at 24 and 32 px: `55°C` over `2865 RPM`, with the dead readings ignored.
- [ ] A fake desktop tree (k10temp, amdgpu, two NVMe drives, three fans) screenshots `CPU / GPU` over the fastest fan, then warm and hot readings in orange and red, then every fan at 0 as `Fans Off`.
- [ ] A vertical panel shows the thermometer and a bare number.
- [ ] The applet loads under Caminito and under X11 MATE, and survives a chip disappearing (a module unloaded).
