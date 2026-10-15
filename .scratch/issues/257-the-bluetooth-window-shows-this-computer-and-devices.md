# The Bluetooth settings window shows This computer and Devices

Epic: bluetooth
Type: implementation
Status: ready-for-agent
Blocked by: 255
Spec: [Bluetooth](../bluetooth/spec.md)

## What to build

"The settings window" in [the spec](../bluetooth/spec.md), apart from Add New Device (258).

**The layout is a list beside a detail pane.** The developer chose "Window C" from the prototype on the `prototype/bluetooth-ui` branch (`.scratch/bluetooth/prototype/bluetooth_ui_prototype.py --variant 3`). Rewrite it in Rust. Don't port it.

- **The list:** a header with the Bluetooth switch, then This computer pinned at the top, then the paired devices. A **+** button at the bottom is a placeholder until 258.
- **The detail pane** shows the selection.

- **This computer:**
  - The switch for Bluetooth, which is the rfkill block.
  - The adapter's name (`Alias`), which can be edited.
  - The adapter chooser, only with two or more adapters.
  - Discoverable on while the window is open, and off when it closes.
- **A device:**
  - A large icon, its name, its status, and a battery bar (red below 20%).
  - Its type and address, and a name field that renames it.
  - Connect or Disconnect, and Forget (with a confirmation).
  - The list and the pane follow BlueZ as devices change. If the selected device is forgotten, the selection goes back to This computer.
- The window replaces 255's placeholder. The Control Center lists it under Hardware.

## Acceptance criteria

- [ ] Tests against dbusmock: renaming the adapter and a device, forgetting a device, connecting and disconnecting, and discoverable going on when the window opens and off when it closes.
- [ ] The adapter chooser appears with two mock adapters and not with one.
- [ ] A screenshot from a check script, in both themes.
