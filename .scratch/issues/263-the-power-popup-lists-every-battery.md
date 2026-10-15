# The Power popup lists every battery

Epic: power-applet
Type: implementation
Status: ready-for-agent
Blocked by: 262
Spec: [Power applet](../power-applet/spec.md)

## What to build

The batteries table under "The popup" in [the spec](../power-applet/spec.md), as in the prototype's "Popup C" (`prototype/power-applet-ui` branch, `--variant 3`). The prototype is GTK4, so redo it in GTK3 and keep its layout.

- **One table:** icon, then name with state below it, then a bar, then the percentage.
- **Laptop batteries come first** (UPower Battery with `PowerSupply` true). Their state is Charging, Discharging, Not charging or Fully charged, with "· limit N%" when the limit is on.
- **A ⋮ on each laptop battery:**
  - "Health N%".
  - Where `ChargeThresholdSupported`, a switch "Limit charging to N%" (`ChargeEndThreshold`) that calls `EnableChargeThreshold`. A polkit refusal leaves the switch as it was.
- **Then the peripherals,** for UPower's other devices with a percentage, with their icon from their type.
- Below 20% the percentage is in the warning colour.
- The table follows UPower as devices come and go.

## Acceptance criteria

- [ ] Check-script screenshots against dbusmock: one battery, two, none (peripherals only), and a low peripheral.
- [ ] The limit switch appears only where supported, and toggling it calls `EnableChargeThreshold`.
- [ ] A mouse turned off leaves the table.
