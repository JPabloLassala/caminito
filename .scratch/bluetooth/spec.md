# Spec: Bluetooth

Written on 2026-10-14 from a grilling session with the developer. The split between the applet and the program is in [ADR 0035](../../docs/adr/0035-the-bluetooth-popup-is-an-in-process-mate-panel-applet.md). Where this file and the ADR differ, the ADR is right and this file is to be fixed.

Nothing is built yet. Its tickets are 255 to 261, under "Tickets".

## Problem

MATE has no Bluetooth front end. On a Session nothing lets the user turn Bluetooth on or off, see which devices are connected, connect a paired headset, or pair a new keyboard. Pairing a keyboard or phone needs an agent to answer its PIN prompt, and none is running.

## Solution

Two pieces (CONTEXT.md):

- **The Bluetooth applet**, an icon in mate-panel whose popup is like Plasma's: a switch for Bluetooth, the devices, and each one's connection and battery.
- **caminito-bluetooth**, which runs through the whole session as the pairing agent and opens the Bluetooth settings window.

## User stories

1. As a user, I want to see at a glance in the panel whether Bluetooth is off, on, or connected to something.
2. As a user, I want to connect or disconnect my headphones from the panel in two clicks.
3. As a user, I want to see my devices' battery levels in the panel's tooltip and popup.
4. As a user, I want to turn Bluetooth off from the panel and have it stay off after a reboot.
5. As a user, I want to pair a new device from a window that waits for me while I press its buttons.
6. As a user pairing a keyboard or a phone, I want to be shown or asked for its PIN, whether I started the pairing or the device did.
7. As a user, I want to rename or forget a paired device.
8. As a user, I want to name my computer and make it visible to other devices while I'm pairing.
9. As a user without Bluetooth hardware, I want no icon.
10. As a user on X11 MATE, I want the same thing.

## Implementation decisions

### The two pieces

- **The Bluetooth applet** is a mate-panel applet in C and GTK3, loaded in-process, in `helpers/caminito-bluetooth-applet/` as a Meson subproject of the root [meson.build](../../meson.build), like `caminito-locker`. It's called "Bluetooth" in Add to Panel. It shares only `helpers/caminito-applet-common/` with the Power and Thermal applets: the popup window and themed icons with fallbacks (ADR 0035).
- **caminito-bluetooth** is a crate in the Cargo workspace, Rust and GTK4 without libadwaita (ADR 0014), like `caminito-displays`.
- **They share only BlueZ's D-Bus API** (`org.bluez`: the object manager, `Adapter1`, `Device1`, `Battery1`, `AgentManager1`). The applet uses GDBus, and `caminito-bluetooth` uses gio's D-Bus from gtk-rs, so the workspace gets no zbus. There's no private interface. The applet starts `caminito-bluetooth` (or `caminito-bluetooth --scan`) to open the window.
- **Any MATE desktop.** Neither piece depends on Caminito. Their files go in the normal XDG directories, not `share/caminito/xdg/`: the applet's `.mate-panel-applet` file, `caminito-bluetooth.desktop` (`Categories=GTK;Settings;HardwareSettings;`, `OnlyShowIn=MATE;`) and its autostart entry.

### caminito-bluetooth

- A single-instance `GtkApplication`.
- **`--background`**, from an XDG autostart entry at login, registers the agent and opens no window. The process holds itself until the session ends.
- **With no flag**, the running instance opens or raises the window. That's what the Control Center and the applet's "Configure…" do. If there's no running instance, the program starts, registers the agent and opens the window.
- **`--scan`** does the same, but opens the window on Add New Device.

### The pairing agent

- `caminito-bluetooth` registers it with `AgentManager1` and makes it the default agent, so it answers every pairing request in the session, including one a device starts.
- `RequestPinCode` and `RequestPasskey` show a dialog to enter the code.
- `DisplayPinCode` and `DisplayPasskey` show the code to type on the device.
- `RequestConfirmation` shows the number and asks whether it matches the device's.
- `RequestAuthorization`, for a pairing the device started, asks.
- `AuthorizeService` is allowed silently for a paired, trusted device and asks otherwise.
- `Cancel` closes the dialog.
- Every device paired through it is marked trusted.

### On and off

- **Off is an rfkill soft block** on the Bluetooth adapters, so it lasts across reboots through systemd-rfkill, and agrees with the airplane-mode key. On lifts the block and powers the adapters.
- The switch in the popup and in the window both mean this.

### More than one adapter

- The popup treats every adapter as one: the switch is for all of them, and the device list is merged.
- The window has an adapter chooser in This computer only when there are two or more.

### The Bluetooth applet

- **The icon** has three states: off, on with nothing connected, and connected. Icon themes disagree on the names: Breeze has no `bluetooth-active` or `bluetooth-disabled`, only `network-bluetooth-activated` and `network-bluetooth-inactive`. So the applet asks for a themed icon with fallbacks, the MATE and Adwaita names first and then Breeze's.
- **The tooltip** names the connected devices and their battery levels.
- **No adapter, no icon:** the applet hides itself, and comes back when one appears.
- **Left-click** toggles the popup, which is a GTK3 popup of the panel.
- **Right-click** opens a menu: "Turn Bluetooth off" (or on) and "Configure Bluetooth…". **Middle-click** does nothing.

### The popup

Plasma's layout, which the developer chose from the prototype ("Popup A") on 2026-10-14:

- A header with a switch for Bluetooth.
- The connected devices, then the paired ones that aren't connected. Each row has the device's icon (from `Device1.Icon`), its name and its battery when `Battery1` is there.
- A row expands to Connect or Disconnect, and a line of details: the type and the address.
- "Add New Device…" at the bottom runs `caminito-bluetooth --scan` and closes the popup.
- When Bluetooth is off, the list is replaced by a line saying so.
- The popup lists no unpaired devices.

### The settings window

Its layout is a list beside a detail pane, like Plasma's settings page. The developer chose it from the prototype ("Window C", on the `prototype/bluetooth-ui` branch) on 2026-10-14.

- **The list** on the left:
  - A header with the switch for Bluetooth.
  - **This computer**, pinned at the top.
  - The paired devices, each with its icon, name and "Connected" when it is.
  - A **+** button at the bottom, for Add New Device.
- **The detail pane** on the right shows what is selected: This computer, a device, or Add New Device.
- **This computer:** the switch for Bluetooth, the adapter's name (`Alias`), editable, and the adapter chooser when needed. The computer is discoverable while the window is open, and stops being so when it closes.
- **A device:**
  - A large icon, its name, and its status.
  - A battery bar when `Battery1` is there, red below 20%.
  - Its type and address, and a name field that renames it (sets `Alias`).
  - Connect or Disconnect, and Forget (`RemoveDevice`).
- **Add New Device**, from **+** or `--scan`, fills the detail pane and scans while it's shown. It lists devices that have a name and aren't paired, and a click pairs one. The agent's dialogs come up as needed, and the device then connects and moves to Devices.
- No trust or block toggles. Pairing trusts, and blocking is left to `bluetoothctl`.

### Not in v1

- Notifications for connections or low battery.
- OBEX file transfer.
- Audio profiles. They're PipeWire's, and mate-media's sound settings list them.
- Translations, which no Caminito program has.

### Blueman

Nothing stops Blueman from being installed beside it. BlueZ gives the default agent to whichever registered last, and the README says so.

### Packaging

- `caminito-git` (Arch) and `packaging/debian/caminito` (Debian) both build and install the new crate and the applet.
- The applet's build needs mate-panel's applet library headers, from `mate-panel-caminito` or the patched Debian `mate-panel` (ADR 0029).
- The applet is in the default top panel of a new session, beside the notification area.

## Testing

- **Rust:** integration tests against python-dbusmock's `bluez5` template on a private bus. They cover listing adapters and devices, battery, connect and disconnect, rename and forget, a scan finding a device, each agent request, and the rfkill state, faked where the mock has none.
- **The applet:** check scripts in the tickets' check directories, which start a mock BlueZ, run mate-panel with the applet and take screenshots for the developer.
- [checklist.md](../../docs/testing/checklist.md) gets items for real hardware: pairing a keyboard with a PIN, a phone with a confirmation, and a headset, then off and a reboot. These are the developer's to run.

## Not decided

Nothing.

## Tickets

Seven slices, filed in `.scratch/issues/` as tickets 255 to 261.

| | Slice | Blocked by |
| --- | --- | --- |
| 255 | [**caminito-bluetooth runs through the session and reads BlueZ.**](../issues/255-caminito-bluetooth-runs-through-the-session.md) | None |
| 256 | [**The pairing agent answers every pairing request.**](../issues/256-the-pairing-agent-answers-every-pairing-request.md) | 255 |
| 257 | [**The settings window shows This computer and Devices.**](../issues/257-the-bluetooth-window-shows-this-computer-and-devices.md) | 255 |
| 258 | [**Add New Device scans and pairs.**](../issues/258-add-new-device-scans-and-pairs.md) | 256, 257 |
| 259 | [**The Bluetooth applet's icon shows the state.**](../issues/259-the-bluetooth-applets-icon-shows-the-state.md) | None |
| 260 | [**The Bluetooth applet's popup lists and connects devices.**](../issues/260-the-bluetooth-applets-popup.md) | 258, 259 |
| 261 | [**The applet in the default panel, and the packaging.**](../issues/261-bluetooth-in-the-default-panel-and-the-packaging.md) | 260 |
