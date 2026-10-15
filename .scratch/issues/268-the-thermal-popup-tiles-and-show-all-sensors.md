# The Thermal popup: tiles and Show all sensors

Epic: thermal-applet
Type: implementation
Status: ready-for-agent
Blocked by: 267
Spec: [Thermal applet](../thermal-applet/spec.md)

## What to build

"The popup" in [the spec](../thermal-applet/spec.md), as the prototype's Popup C with Popup A's bars (`prototype/thermal-applet-ui`, `--variant 3`), redone in GTK3. The pins are drawn but do nothing until 269.

- **Tiles,** two to a row: one for each of CPU, GPU, Storage and Fans that the computer has.
  - A temperature tile shows its group's hottest reading in its band's colour, with that reading's name under it when the group has several readings.
  - The Fans tile shows the fastest fan, or "Stopped".
  - No other group fills in, and tiles don't respond to clicks.
- **"Show all sensors"** is an expander over the groups in the spec's order, each with its icon and name.
  - A temperature row is the name, a bar on the common 20–100 °C scale in its band's colour, and the temperature.
  - Fans come last: icon, name, a bar against `fan*_max` (or against the fastest seen once watched for 10 readings, with no bar before that), and the speed or "Stopped".
- The popup updates in place every second while it's open, without rebuilding, so hovering and the expander aren't disturbed.

## Acceptance criteria

- [ ] Check scripts screenshot the popup, collapsed and expanded, for the fake ThinkPad (CPU and Fan tiles) and the fake desktop (four tiles).
- [ ] A fan with no max shows no bar for its first 10 readings, then a bar. A fan with a max always has one. A stopped fan says "Stopped".
- [ ] A hot reading's bar and value are red, and its tile too when it's the group's hottest.
