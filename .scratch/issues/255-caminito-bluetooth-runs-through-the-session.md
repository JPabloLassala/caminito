# caminito-bluetooth runs through the session and reads BlueZ

Epic: bluetooth
Type: implementation
Status: resolved
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

- [x] Integration tests against python-dbusmock's `bluez5` template on a private bus: adapters and devices listed, a device added and removed while running, battery read, connect and disconnect, alias, remove.
- [x] The rfkill state is read and set through a seam that the tests fake.
- [x] A second `caminito-bluetooth` reaches the first instead of starting another process.
- [x] `cargo test` and the workspace lints pass.

## Done (2026-10-15)

- **The library names no toolkit.** `caminito_bluetooth::bluez` is BlueZ's model over gio's D-Bus: it watches `org.bluez`, reads `GetManagedObjects` once BlueZ is on the bus and forgets everything when it leaves, and keeps the properties as the variants BlueZ sent, so the adapters and devices are read from them on demand. It subscribes before it reads, and the read's answer replaces whatever came before it. A generation counter stops a read sent before BlueZ restarted from overwriting what the new instance said since. GTK is the binary's alone.
- **The calls:** connect, disconnect, alias (adapter or device), remove (`RemoveDevice` on the device's own adapter), powered, discoverable, and start and stop discovery. Each path is checked as an object path before it reaches gio.
- **Off is the soft block, behind `rfkill::Rfkill`.** `SystemRfkill` reads `/sys/class/rfkill` and writes one 8-byte `CHANGE_ALL` event for Bluetooth to `/dev/rfkill`. Its unit tests use a fake sysfs and a plain file as the device. Bluetooth counts as off when any of its switches is soft-blocked. A hard block alone doesn't count as off: `Blocks::hard` reports it, for the popup and the window to say so.
- **On powers the adapters that come back as well (from the review).** bluetoothd learns of a lifted block on its own time, and an adapter whose radio the block had cut only reappears afterwards. So `switch::Switch` powers each adapter that appears or changes unpowered after on, once per adapter. One the user powers off afterwards stays off.
- **The single instance:** `HANDLES_COMMAND_LINE` with `--background` and `--scan` as main options. A second start's command line reaches the first, which takes its hold however it was started, so closing the window doesn't end the agent's process. The test runs the real binary on a private bus and checks three things: the first owns the name, a second start exits once the first has exported `window/1`, and a second `--background` exits too. GTK needs a display, so the test is ignored, and tools/check runs it under Xvfb.
- **The tests' BlueZ** is python-dbusmock's `bluez5` on a dbus-daemon of each test's own, as both the system and the session bus, with each test on a main context of its own. The template has no `Battery1`, so the fixture adds the interface and emits `InterfacesAdded` itself. Its `StartDiscovery` fails without a discovery filter (python-dbusmock 0.31), so the fixture sets an empty one.
- **The entries** are `caminito-bluetooth.desktop` in `share/applications` and the autostart entry (`--background`, `OnlyShowIn=MATE;`) in `/etc/xdg/autostart`. Both carry `TryExec`, so they do nothing until 261 installs the binary. tools/check validates both where they're installed. The PKGBUILD's checkdepends and Debian's Build-Depends gain python-dbusmock, which the packages' `cargo test` now needs. Everything else in packaging is 261's.
- **What wasn't run:** the whole of tools/check on the developer's Arch machine (no meson build and no cargo-deny here). This was built and tested in an Ubuntu 24.04 container. There, the crate's tests, the single instance under Xvfb, its clippy and caminito's 2085 tests passed. caminito-theme's pixel goldens differ, from Ubuntu's fonts and pango, and clippy 1.97 flags three `manual_range_contains` in caminito-theme. Neither crate is touched here.
