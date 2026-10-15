# The Bluetooth applet in the default panel, and the packaging

Epic: bluetooth
Type: implementation
Status: ready-for-agent
Blocked by: 260
Spec: [Bluetooth](../bluetooth/spec.md)

## What to build

"Packaging" and "Blueman" in [the spec](../bluetooth/spec.md).

- **The default panel:** the Bluetooth applet is in a new session's top panel, beside the notification area. Choose how: a carried mate-panel patch to the default layout, or a layout of Caminito's own. Whichever is chosen must work under X11 MATE too.
- **Arch:**
  - `caminito-git` builds and installs the crate and the applet.
  - The build depends on mate-panel's applet library.
  - The package depends on `bluez`.
- **Debian:** `packaging/debian/caminito` does the same against the patched `mate-panel` (ADR 0029).
- **README:** Bluetooth, and that Blueman can live alongside it, with the default agent going to whichever registers last.
- **[checklist.md](../../docs/testing/checklist.md):** items on real hardware: pair a keyboard with a PIN, a phone with a confirmation, and a headset; turn Bluetooth off and reboot.

## Acceptance criteria

- [ ] A fresh user on a Session gets the applet in the top panel.
- [ ] `makepkg` and the Debian build install both pieces, and the Control Center lists Bluetooth.
- [ ] The README and the checklist are updated.
