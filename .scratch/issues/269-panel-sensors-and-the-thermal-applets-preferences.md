# Panel sensors and the Thermal applet's Preferences

Epic: thermal-applet
Type: implementation
Status: ready-for-agent
Blocked by: 268
Spec: [Thermal applet](../thermal-applet/spec.md)

## What to build

The pins under "The popup" and "Preferences" in [the spec](../thermal-applet/spec.md).

- **The pin** on a temperature row shows on hover, and stays shown on the **panel sensors'** (CONTEXT.md) rows.
  - Clicking an unpinned row's pin adds that reading to the panel sensors, dropping the older one when there are already two.
  - Clicking a pinned row's pin removes it, unless it's the only one.
- **Applet preferences,** in the applet's GSettings:
  - the panel sensors, by reading ID (chip and subfeature), falling back to the defaults when a pinned chip is gone
  - °C or °F
  - the warm and hot thresholds
  - whether Show all is open
- **The Preferences dialog** (right-click) has the units and the two thresholds.

## Acceptance criteria

- [ ] Pinning a third reading drops the older one, and the panel and tooltip follow at once.
- [ ] The panel sensors, the units and Show all's state survive restarting mate-panel.
- [ ] °F applies to the panel, the tooltip and the popup. Changing a threshold recolours them.
- [ ] A pinned reading whose chip disappears falls back to the defaults without a crash.
