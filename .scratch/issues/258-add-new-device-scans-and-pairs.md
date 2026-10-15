# Add New Device scans and pairs

Epic: bluetooth
Type: implementation
Status: ready-for-agent
Blocked by: 256, 257
Spec: [Bluetooth](../bluetooth/spec.md)

## What to build

Add New Device, under "The settings window" in [the spec](../bluetooth/spec.md).

- The window's **+** button (257) and `--scan` show Add New Device in the detail pane, as in the prototype's "Window C". It scans while it's shown. It lists devices that have a name and aren't paired, with their icons.
- A click pairs the device: the agent's dialogs (256) come up as needed, then the device connects and moves to Devices. A failure is shown on its row.
- `caminito-bluetooth --scan` opens the window with this in the detail pane.

## Acceptance criteria

- [ ] Tests against dbusmock: discovery starts when the page is shown and stops when it's left, a found device is listed, and pairing moves it to Devices.
- [ ] Devices without a name aren't listed.
- [ ] `--scan` opens the page, whether or not an instance is running.
