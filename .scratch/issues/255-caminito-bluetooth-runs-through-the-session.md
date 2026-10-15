# caminito-bluetooth runs through the session and reads BlueZ

Epic: bluetooth
Type: implementation
Status: ready-for-agent
Blocked by: None
Spec: [Bluetooth](../bluetooth/spec.md)

## What to build

The base for everything else in the spec: **caminito-bluetooth** (CONTEXT.md), under "The two pieces", "caminito-bluetooth", "On and off" and "More than one adapter" in [the spec](../bluetooth/spec.md).

- **A new crate, `caminito-bluetooth`,** in the workspace: Rust and GTK4 without libadwaita (ADR 0014), `publish = false`, like `caminito-displays`.
- **A single-instance `GtkApplication`.** With `--background` it holds itself and opens no window. With no flag or `--scan`, it opens the window, which for now is an empty placeholder, in the running instance or a new one.
- **A BlueZ model over gio's D-Bus** (no zbus):
  - It reads the adapters, devices and battery levels from `org.bluez`'s object manager.
  - It follows the objects as they appear, change and go.
  - It does: connect, disconnect, set an alias, remove a device, power, discoverable, and start and stop discovery.
  - The window (257, 258) and the agent (256) build on it.
- **On and off as an rfkill soft block** on the Bluetooth adapters, read and set. On also powers the adapters. All adapters together.
- **Its autostart entry** (`--background`, `OnlyShowIn=MATE;`) and `caminito-bluetooth.desktop`, both in the normal XDG directories, not `share/caminito/xdg/`.

## Acceptance criteria

- [ ] Integration tests against python-dbusmock's `bluez5` template on a private bus: adapters and devices listed, a device added and removed while running, battery read, connect and disconnect, alias, remove.
- [ ] The rfkill state is read and set through a seam that the tests fake.
- [ ] A second `caminito-bluetooth` reaches the first instead of starting another process.
- [ ] `cargo test` and the workspace lints pass.
