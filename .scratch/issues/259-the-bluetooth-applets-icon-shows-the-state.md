# The Bluetooth applet's icon shows the state

Epic: bluetooth
Type: implementation
Status: ready-for-agent
Blocked by: None
Spec: [Bluetooth](../bluetooth/spec.md)

## What to build

The **Bluetooth applet** (CONTEXT.md) without its popup, under "The two pieces" and "The Bluetooth applet" in [the spec](../bluetooth/spec.md), and [ADR 0035](../../docs/adr/0035-the-bluetooth-popup-is-an-in-process-mate-panel-applet.md).

- `helpers/caminito-bluetooth-applet/`: an in-process mate-panel applet in C and GTK3, as a Meson subproject of the root `meson.build`, with its `.mate-panel-applet` file. It's called "Bluetooth" in Add to Panel.
- It reads BlueZ directly over GDBus, through the object manager, `Adapter1`, `Device1` and `Battery1`, and follows changes.
- **The icon** has three states: off, on with nothing connected, and connected. **The tooltip** shows the connected devices and their battery.
- **No adapter, no icon.** It comes back when an adapter appears.
- **Right-click** shows "Turn Bluetooth off" (or on), which is the rfkill block, and "Configure Bluetooth…", which runs `caminito-bluetooth`.

## Acceptance criteria

- [ ] A check script runs mate-panel with the applet against a mock BlueZ and screenshots each state: no adapter, off, on, and connected with a battery in the tooltip.
- [ ] The applet loads under Caminito and under X11 MATE.
- [ ] Stopping BlueZ and starting it again doesn't crash the panel.
