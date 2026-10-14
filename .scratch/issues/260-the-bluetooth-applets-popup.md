# The Bluetooth applet's popup lists and connects devices

Epic: bluetooth
Type: implementation
Status: ready-for-agent
Blocked by: 258, 259
Spec: [Bluetooth](../bluetooth/spec.md)

## What to build

"The popup" in [the spec](../bluetooth/spec.md), in the manner of Plasma's.

- Left-click toggles a GTK3 popup of the panel.
- The popup has:
  - A header switch for Bluetooth, for every adapter.
  - The connected devices, then the paired ones that aren't connected, from all adapters merged. Each row has its icon, name and battery.
- A row expands to Connect or Disconnect, and the device's type and address.
- "Add New Device…" runs `caminito-bluetooth --scan` and closes the popup.
- With Bluetooth off, a line says so in place of the list.

## Acceptance criteria

- [ ] A check script against a mock BlueZ screenshots the popup off, with connected and disconnected devices, and with a row expanded, on a top and a bottom panel.
- [ ] Connect and Disconnect in the popup change the mock device's state.
- [ ] The popup closes when it loses focus, and on Escape.
