# The Bluetooth applet's icon shows the state

Epic: bluetooth
Type: implementation
Status: open: the code is in, and loading the applet under Caminito, on a Session, is the developer's to check
Blocked by: None
Spec: [Bluetooth](../bluetooth/spec.md)

## What to build

The **Bluetooth applet** (CONTEXT.md) without its popup, under "The two pieces" and "The Bluetooth applet" in [the spec](../bluetooth/spec.md), and [ADR 0035](../../docs/adr/0035-the-bluetooth-popup-is-an-in-process-mate-panel-applet.md).

- `helpers/caminito-bluetooth-applet/`: an in-process mate-panel applet in C and GTK3, as a Meson subproject of the root `meson.build`, with its `.mate-panel-applet` file. It's called "Bluetooth" in Add to Panel.
- **`helpers/caminito-applet-common/`**, the static library the Bluetooth, Power and Thermal applets share (ADR 0035): the popup window placed against the applet's button, left-click toggling it and losing focus closing it, and themed icons with fallbacks. If it doesn't exist yet, this ticket extracts it. If it does, this ticket uses it.
- It reads BlueZ directly over GDBus, through the object manager, `Adapter1`, `Device1` and `Battery1`, and follows changes.
- **The icon** is a themed icon with fallbacks, because Breeze has none of MATE's Bluetooth names (see the spec). It has three states: off, on with nothing connected, and connected. **The tooltip** shows the connected devices and their battery.
- **No adapter, no icon.** It comes back when an adapter appears.
- **Right-click** shows "Turn Bluetooth off" (or on), which is the rfkill block, and "Configure Bluetooth…", which runs `caminito-bluetooth`.

## Acceptance criteria

- [x] A check script runs mate-panel with the applet against a mock BlueZ and screenshots each state: no adapter, off, on, and connected with a battery in the tooltip.
- [ ] The applet loads under Caminito and under X11 MATE.
- [x] Stopping BlueZ and starting it again doesn't crash the panel.

## Done (2026-10-15)

- **Built test-first at the pure seam,** `bt-state.c`: the icon's state and the tooltip from snapshots built by hand. Off is "no adapter powered", which an rfkill block leaves too, since bluetoothd powers a blocked adapter off. Nothing reads rfkill's state. A device counts as connected only on a powered adapter. The tooltip reads "Bluetooth is on" and then a line for each connected device: "WH-1000XM4: 80%", or the device's name alone when it has no Battery1.
- **BlueZ comes through a `GDBusObjectManagerClient` without auto-start** (`bt-bluez.c`). It follows bluetoothd's name, so stopping it removes every object and starting it adds them back. Every signal collapses into one idle update, and property changes count only on Adapter1, Device1 and Battery1. `test-bluez.c` runs it against python-dbusmock's `bluez5` template on a private bus standing in for the system bus, with `bluez-mock.py` adding `AddBattery`, which the template lacks. Its cases are the states, a restart, bluetoothd starting after the applet, powering every adapter, and freeing the applet while it's still starting. The test is skipped where no Python has dbusmock.
- **Off is `RFKILL_OP_CHANGE_ALL` on `RFKILL_TYPE_BLUETOOTH` written to `/dev/rfkill`.** On also powers the adapters, retrying `Powered` for 3 s while bluetoothd catches up with the unblock. Where `/dev/rfkill` can't be opened, the menu powers the adapters alone and logs a warning, so the switch still does something until the next boot. A hard block isn't shown: the menu then offers "Turn Bluetooth on", which does nothing visible.
- **No adapter hides the whole applet widget.** It's shown again when one appears, with nothing left in the panel in between: the screenshots' first frame is empty.
- **The icon names** for each state are MATE's plain name, then Adwaita's symbolic one, then Breeze's: off is `bluetooth-disabled`, `bluetooth-disabled-symbolic`, `network-bluetooth-inactive`; on is `bluetooth-active`, `bluetooth-active-symbolic`, `network-bluetooth`; connected is `bluetooth-paired`, `bluetooth-active-symbolic`, `network-bluetooth-activated`. GTK 3 tries every name in the current theme before its parents. Breeze 5.115 on Ubuntu has no `network-bluetooth-inactive`, so off falls through to Adwaita's. The spec names `network-bluetooth-activated` for Breeze's on, but `network-bluetooth` is its plain on icon and the activated one is kept for connected. MATE's own theme and Adwaita have no connected icon, so connected looks like on there; Humanity and Breeze tell them apart.
- **caminito-applet-common was extracted here,** as the first applet built (ADR 0035). It has `caminito-applet-icon` (a GtkImage sized to the panel, with themed fallbacks) and `caminito-applet-popup`:
  - The popup is a `GTK_WINDOW_POPUP` placed by `gdk_window_move_to_rect` against the applet, away from the panel's edge.
  - It's shown from inside a seat grab, so that on Wayland it maps as a grabbing xdg_popup.
  - Left-click toggles it. A click outside it, Escape, a broken grab, another grab or an unmap closes it.
  - A click on the applet that has just closed it doesn't reopen it for 250 ms.
  
  `test-popup.c` covers the anchors and, on a display, the clicks. The Bluetooth applet doesn't open the popup yet: that's 260's.
- **The check script** (`259-checks/run.sh`) runs mate-panel on Xvfb with the applet alone on a top panel, against the mock, once per icon theme it finds. `screenshots/` holds what it took here in mate, Humanity, Adwaita and Breeze: no adapter, on, off, connected with the tooltip, the menu, off and on from the menu, and BlueZ stopped and started again. The panel ran through all of them. The menu's off and on step runs only where `/dev/rfkill` isn't writable, so that it can't block the computer's own radio.
- **Not run here:** loading under Caminito. That needs a Session, which this cloud container (Ubuntu 24.04, no OrbStack machine) hasn't got, so the status stays open for it. The top-level `meson setup` couldn't run either, as gtk-session-lock isn't packaged on Ubuntu, so the two subprojects were built and tested under a scratch top-level project with the same `subproject_dir`. The Wayland paths of the popup (the xdg_popup grab, and the compositor dismissing it) are untested, and 260 will exercise them.

