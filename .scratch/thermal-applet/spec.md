# Spec: Thermal applet

Written on 2026-10-14 from a grilling session with the developer and a prototype. The applet's shape follows [ADR 0035](../../docs/adr/0035-the-bluetooth-popup-is-an-in-process-mate-panel-applet.md). Where this file and the ADR differ, the ADR is right and this file is to be fixed.

Nothing is built yet. Its tickets are 267 to 270, under "Tickets".

## Problem

Nothing in MATE shows how hot the computer is or how fast its fans spin at a glance, the way TG Pro does on macOS. MATE's own mate-sensors-applet draws readings straight in the panel, with no popup, and isn't installed by default.

## Solution

The **Thermal applet** (CONTEXT.md): a thermometer in mate-panel with the **panel sensors'** (CONTEXT.md) temperatures over the fans' speed, in the manner of TG Pro's menu bar item. Its popup has tiles for the CPU, the GPU, the storage and the fans, with every reading under "Show all sensors". It only watches. It never sets a fan's speed.

## User stories

1. As a user, I want to see the CPU's temperature in the panel, and the GPU's beside it when there is one.
2. As a user, I want to see at a glance in the panel whether the fans are spinning, and how fast.
3. As a user, I want the temperature to turn orange and then red when it gets hot.
4. As a user, I want a quick summary of the hottest CPU, GPU and drive readings, and the fastest fan.
5. As a user, I want to see every sensor the computer has, grouped, with bars I can compare.
6. As a user, I want to choose which temperatures are in the panel.
7. As a user with a 24-pixel panel, I still want it readable.
8. As a user in the US, I want °F.
9. As a user on X11 MATE, I want the same applet.

## Implementation decisions

### The applet

- An in-process mate-panel applet in C and GTK3, in `helpers/caminito-thermal-applet/`, as a Meson subproject of the root [meson.build](../../meson.build). It's called "Temperatures and Fans" in Add to Panel.
- It reads its sources through libsensors. There's no Rust half, no settings window, no private interface, and no privileged helper.
- Neither the applet nor its files depend on Caminito. They go in the normal XDG directories.
- It's a new applet, not a patch to mate-sensors-applet, and it shares nothing with it.
- It shares `helpers/caminito-applet-common/` with the Bluetooth and Power applets: the popup window and themed icons with fallbacks (ADR 0035).
- It's in Add to Panel only, not in a new session's default panel.

### Its sources

- **libsensors** (lm_sensors) for every chip's temperatures (`SENSORS_SUBFEATURE_TEMP_INPUT`, with `_CRIT`) and fans (`SENSORS_SUBFEATURE_FAN_INPUT`, with `_MAX`). Its labels and `ignore` lines from `/etc/sensors3.conf` and `/etc/sensors.d/` apply.
- It reads every 2 s while the popup is closed and every 1 s while it's open. The interval can't be configured.
- **Leaving out:**
  - A reading that can't be read, or that has been exactly 0 since the applet started, isn't shown. The developer's ThinkPad has six of these.
  - Battery, AC and USB-C chips (`BAT*`, `AC`, `ucsi_*`) are left out. They have no temperatures.
- **Grouping:** each reading's group comes from its chip:
  - **CPU:** `coretemp`, `k10temp`, `zenpower`, `cpu_thermal`.
  - **GPU:** `amdgpu`, `nouveau`, `radeon`, `i915`, `xe`.
  - **Storage:** `nvme`, `drivetemp`.
  - **Wireless:** `iwlwifi`, `ath*`, `mt79*`.
  - **Board:** `acpitz`, `pch_*`, `thinkpad`, `dell_smm`, `nct*`, `it87` and the like.
  - **Other:** anything else.
- **Names:**
  - The label when the chip gives one: "CPU Package" for coretemp's `Package id 0`, and "CPU" for k10temp's `Tctl`.
  - The embedded controller's readings are prefixed "EC: ".
  - A GPU's or NVMe's label is prefixed with the device.
  - With no label, the chip's friendly name ("Wi-Fi", "ACPI zone", "Chipset"), numbered when it has several.
- **Not in v1:**
  - NVIDIA's proprietary driver, which has no hwmon. Its temperatures are only through NVML.
  - SATA drives without `drivetemp`. The README says to load the module, and nothing loads it automatically.

### Bands

- Every temperature is in one of three bands:
  - **Normal** below 75 °C.
  - **Warm** from 75 °C.
  - **Hot** from 90 °C, or from the reading's crit minus 5 when that's lower.
- The two thresholds are applet preferences for every reading at once. There are none per sensor.
- Chips' high and crit values alone aren't trusted: coretemp gives 100 for both.
- **Colours:** normal is the theme's text colour, warm is orange and hot is red, in the panel and in the popup alike.

### The panel

The developer chose TG Pro's menu bar item, as on the `prototype/thermal-applet-ui` branch.

- **A thermometer glyph,** from a themed icon with fallbacks (`xsi-temperature-symbolic`, `temperature-normal-symbolic`, `sensors-temperature-symbolic`).
- **Two lines beside it:**
  - **Top:** the panel sensors' temperatures with units, each in its band's colour, e.g. `77°C / 85°C`.
  - **Bottom:** the fastest fan's speed (`1897 RPM`), or `Fans Off` when every fan reads 0. It's left out when the computer has no fans.
- **Panel sensors:**
  - One or two of them.
  - By default, the CPU's reading, and the hottest GPU reading when there's a GPU. With no GPU, the CPU's reading alone.
  - The CPU's reading is coretemp's Package or k10temp's Tctl. With neither, it's the hottest reading.
  - The user changes them with the pins in the popup.
- **Below 32 px** (MATE's default panel is 24 px), it's one line: `77°C / 85°C  1897 RPM`.
- **On a vertical panel,** it's the thermometer with the first panel sensor's temperature as a bare number (`77°`) under it, with no second one and no fans.
- **The tooltip** is every panel sensor's name and temperature, and every fan's name and speed: "CPU Package 55 °C · Fan 2865 RPM".
- **Left-click** toggles the popup. **Right-click** shows Preferences and About.

### The popup

The developer chose the prototype's "Popup C" with Popup A's bars (`--variant 3`). Redo it in GTK3, keeping its layout. From the top:

- **Tiles,** two to a row:
  - One for each of CPU, GPU, Storage and Fans that the computer has, so four at most. No other group fills in.
  - A temperature tile is the group's icon and name, its hottest reading in its band's colour, and that reading's name under it when the group has more than one reading.
  - The Fans tile is the fastest fan's speed, or "Stopped", and that fan's name when there are several. It's called "Fan" when there's one.
  - Tiles don't respond to clicks.
- **"Show all sensors",** an expander. Whether it's open is an applet preference, kept across openings and restarts. Under it are the groups in the order above, each with its icon and name as a heading, then its rows:
  - **A temperature row** is the name, a bar, the temperature, and a pin.
  - **The bar** runs on one scale for every reading, from 20 °C (empty) to 100 °C (full), in the reading's band's colour. The length shows the temperature and the colour shows the danger, so a drive at 80 °C is a two-thirds bar but red.
  - **The pin** shows on hover and stays on the panel sensors' rows. Clicking an unpinned row's pin adds that reading to the panel sensors, dropping the older one when there are already two. Clicking a pinned one removes it, unless it's the only one.
  - **Fans** come last as rows: a fan icon, the name, a bar, and the speed or "Stopped". The bar is against the fan's `fan*_max`, or against the fastest it has spun since the applet started, once it has been watched for 10 readings. Until then there's no bar.

### Preferences

- °C or °F, with °C the default. It applies to the panel, the tooltip and the popup.
- The warm and hot thresholds, by default 75 °C and 90 °C.
- The panel sensors and whether Show all is open, which the popup sets, are kept here too.

### Not in v1

- **Fan control** (Auto, Max, a custom speed or curve). It needs a privileged helper, polkit and per-machine EC quirks, so it's an epic of its own if ever.
- **A window** with history charts, and notifications on overheating.
- **Readings outside hwmon:** NVIDIA through NVML, and SMART through UDisks.
- **A chassis or ambient temperature as a default panel sensor.** hwmon has no such kind. A user can pin `SYSTIN` or a chip's "Ambient".
- **Translations,** which no Caminito program has.

## Testing

- Check scripts in the tickets' check directories run mate-panel with the applet against a fake hwmon tree, and take screenshots for the developer. The tree is made with `umockdev`, if libsensors reads it. If it doesn't, the applet reads a fake tree from `CAMINITO_THERMAL_HWMON=<dir>`, for tests only. The first ticket finds out which.
- They cover:
  - The developer's ThinkPad: one fan with no max, no GPU, and dead readings.
  - A desktop: an AMD CPU, a Radeon, two NVMe drives, three fans, and a stopped GPU fan.
  - Warm and hot readings.
  - One and two panel sensors.
  - 24 and 32 px panels, and a vertical one.
  - °F.
- [checklist.md](../../docs/testing/checklist.md) gets items for real hardware: the readings match `sensors`, and a load (`stress-ng --cpu 0`) turns the panel orange and spins the fan up. These are the developer's to run.

## Not decided

Nothing.

## Tickets

Four slices, filed in `.scratch/issues/` as tickets 267 to 270.

| | Slice | Blocked by |
| --- | --- | --- |
| 267 | [**The Thermal applet shows the temperatures in the panel.**](../issues/267-the-thermal-applet-shows-the-temperatures-in-the-panel.md) | None |
| 268 | [**The Thermal popup: tiles and Show all sensors.**](../issues/268-the-thermal-popup-tiles-and-show-all-sensors.md) | 267 |
| 269 | [**Panel sensors and the Thermal applet's Preferences.**](../issues/269-panel-sensors-and-the-thermal-applets-preferences.md) | 268 |
| 270 | [**The Thermal applet's packaging.**](../issues/270-the-thermal-applets-packaging.md) | 267, 268, 269 |
