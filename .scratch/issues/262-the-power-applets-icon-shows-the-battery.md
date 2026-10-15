# The Power applet's icon shows the battery

Epic: power-applet
Type: implementation
Status: ready-for-agent
Blocked by: None
Spec: [Power applet](../power-applet/spec.md)

## What to build

The **Power applet** (CONTEXT.md) without its popup's sections, under "The applet", "Its sources" (UPower) and "The icon" in [the spec](../power-applet/spec.md).

- `helpers/caminito-power-applet/`: an in-process mate-panel applet in C and GTK3, as a Meson subproject of the root `meson.build`, with its `.mate-panel-applet` file. It's called "Power" in Add to Panel.
- **`helpers/caminito-applet-common/`**, the static library the Bluetooth, Power and Thermal applets share (ADR 0035): the popup window placed against the applet's button, left-click toggling it and losing focus closing it, and themed icons with fallbacks. If it doesn't exist yet, this ticket extracts it. If it does, this ticket uses it.
- **The icon** follows UPower's display device: a battery glyph by percentage, a charging variant, or a plug with no battery. It's a themed icon with fallbacks (MATE and Adwaita names, then Breeze's).
- **The tooltip** is the percentage and the time: "… left", "Full in …" or "Plugged in".
- **An applet preference** "Show percentage" (off by default) puts the percentage beside the glyph.
- **Left-click** toggles an empty popup that later tickets fill, with "Configure Power Management…" at its bottom running `mate-power-preferences`. **Right-click** has Preferences and "Configure Power Management…".

## Acceptance criteria

- [ ] A check script against dbusmock's `upower` template screenshots the icon and tooltip on battery, charging, full, and with no battery.
- [ ] "Show percentage" shows and hides the number.
- [ ] The applet loads under Caminito and under X11 MATE, and survives UPower restarting.
