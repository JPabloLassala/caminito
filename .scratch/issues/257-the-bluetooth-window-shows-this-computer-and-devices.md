# The Bluetooth settings window shows This computer and Devices

Epic: bluetooth
Type: implementation
Status: ready-for-agent
Blocked by: 255
Spec: [Bluetooth](../bluetooth/spec.md)

## What to build

"The settings window" in [the spec](../bluetooth/spec.md), apart from Add New Device (258).

- **This computer:**
  - The switch for Bluetooth, which is the rfkill block.
  - The adapter's name (`Alias`), which can be edited.
  - The adapter chooser, only with two or more adapters.
  - Discoverable on while the window is open, and off when it closes.
- **Devices:** the paired devices, each with Connect or Disconnect, Rename and Forget (with a confirmation). They follow BlueZ as devices change.
- The window replaces 255's placeholder. The Control Center lists it under Hardware.

## Acceptance criteria

- [ ] Tests against dbusmock: renaming the adapter and a device, forgetting a device, connecting and disconnecting, and discoverable going on when the window opens and off when it closes.
- [ ] The adapter chooser appears with two mock adapters and not with one.
- [ ] A screenshot from a check script, in both themes.
