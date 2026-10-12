# caminito-wallpaperd draws a picture or a colour on each output

Epic: wallpaper
Type: implementation
Status: resolved
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

The tracer bullet of the epic: the new crate `caminito-wallpaper` and its drawer, `caminito-wallpaperd`, under "Who draws", "An output's wallpaper" and "The settings" in [the spec](../wallpaper/spec.md), and [ADR 0031](../../docs/adr/0031-caminito-wallpaper-draws-the-wallpaper-in-place-of-msds-plugin.md). Only the Picture and Colour kinds; the other three are later tickets and their settings are read as a Colour until then.

- **The drawer** is a layer-shell client with one surface per output in the background layer, in the namespace Caminito's scene already orders behind caja's desktop. It links no GTK.
- **The store** is gsettings under `org.mate.Caminito`: one relocatable set of keys per monitor identity, a "same on all outputs" key, and the lists of added pictures and folders. Changes are read live.
- **A monitor's identity** is a saved layout's: make, model and serial, or the connector when it reports none.
- **An output never seen before** takes the primary output's wallpaper. While "same on all outputs" is on, every output has the same settings.
- **Positioning:** Fill (the default), Fit, Stretch, Centre and Tile, with the output's colour in the bars Fit and Centre leave. It follows the output's mode and scale.
- **Every change cross-fades** over about a second.
- **In a Session** the drawer starts with the session and is restarted if it exits, and mate-settings-daemon's background plugin is off. Nothing changes outside a Session.
- **On first run** the picture `org.mate.background` names, with its positioning, becomes every output's wallpaper.
- `caminito-git`'s PKGBUILD installs the drawer.

## Acceptance criteria

- [x] Unit tests of the toolkit-free parts: each positioning's rectangle for a picture and an output of given sizes; the settings an output gets from the store by identity, with "same on all outputs" on and off, and for a monitor never seen.
- [x] A headless test with the real drawer against Caminito: one background-layer surface per output, a second output added gets one, and a removed output's is gone with none left over (ticket 88's failure).
- [x] A change of a key is drawn without a restart.
- [x] First run takes MATE's picture, and a second run doesn't take it again.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item, in a Session with two monitors: a different picture on each set with `gsettings`, each positioning by eye, the cross-fade, unplugging and plugging a monitor, and `killall caminito-wallpaperd` bringing the wallpaper back.

## For the implementing agent to settle

Recorded in the Done section:

- the schema's keys and the path a monitor's identity becomes;
- which crates draw and decode, given that the drawer links no GTK;
- how the Session starts and restarts the drawer, and how the plugin is turned off there and nowhere else.

## Done (2026-10-12)

- **The tests came first, at three seams.** Unit tests in `caminito-wallpaper` for the pure parts: each positioning's rectangle (`positioning.rs`), the settings an output gets by identity with "same on all outputs" on and off and for a monitor never seen, and the first and second run (`settings/tests.rs`), against a store in memory. `caminito/src/tests/caminito_wallpaperd.rs` runs the real drawer against the headless fixture, on a socket and a keyfile GSettings of the test's own, as the locker's tests do: one `mate-background` surface per output, a third output added, one unplugged and plugged back with none left over (88), a key rewritten and the new colour drawn by the same process, and MATE's picture taken by the first run and not by the second. They're `#[ignore]` and need `CAMINITO_WALLPAPERD`; `tools/check` runs them after the workspace's build. A Session test checks the drawer starts with the command on Caminito's socket, comes back after `kill -9`, and is stopped at the end.
- **The schema.** `org.mate.Caminito.wallpaper` at `/org/mate/caminito/wallpaper/`: `same-on-all-outputs` (on), `outputs` (the monitors seen), `pictures`, `folders` and `took-mate-background`. Each wallpaper is a relocatable `org.mate.Caminito.wallpaper.output` (`kind`, `picture`, `positioning`, `colour` as `#rrggbb`): every output's at `all/`, read while the switch is on, and each monitor's at `outputs/<identity>/`. The identity is `edid-<make>-<model>-<serial>` or `connector-<connector>`, every byte but a letter or digit as `_` and its hex, so no part can run into the next. The drawer reads make, model and serial from output management's heads, and makes a surface only once its monitor's head is described, so the first frame is already the monitor's.
- **A monitor never seen** copies the primary output's own set while the switch is off, or `all/` while it's on or the primary is unseen too, and is listed in `outputs` from then on. The primary output is `org.mate.Caminito`'s `PrimaryOutput`, read when a monitor is new; without the bus, the first output.
- **The first run** writes `org.mate.background`'s picture, its `picture-options` mapped to the nearest positioning (spanned is Fill) and its `primary-color` to `all/`, and sets `took-mate-background`. Nothing bridges back yet: that's 211.
- **Who draws and decodes:** gdk-pixbuf decodes (with EXIF orientation), cairo composes, and the frame is copied into a memfd `wl_shm` buffer, up to three per output, reused on release. GLib's loop runs it, through gio for GSettings and D-Bus and glib-unix for the Wayland fd; wayland-client is pure Rust. No GTK, and no `unsafe` (`forbid`). A buffer is the output's pixels at its fractional scale through a viewport, or at `wl_output`'s integer scale without them.
- **The cross-fade** is a second, paced by frame callbacks, blending the frame it started from with the new one; a change during a fade starts from the blend. A change of size or scale draws at once. An output with no frame callbacks (off, or behind the lock) holds a fade until it has one again; nothing hurries it on a timer.
- **The Session** starts the drawer with its command, as a helper with the default restart policy (back after 0.5 s, given up after five exits in a minute), with the command's variables, and stops it as it ends. A bare `caminito` starts none, nor do the tests unless a test names a program.
- **msd's background plugin is off under Caminito** by a new carried patch, `0012-background-Do-not-start-under-Caminito.patch`, gated on `MATE_WM_NAME=Caminito` as 0005 is, at `mate-settings-daemon-caminito` 1.29.1-8; `caminito-git` depends on that release and installs `caminito-wallpaperd`. 0002 has no effect from then on and can go, as ADR 0031 has it; 0006 stays, being a fix on any compositor. Neither was dropped here.
- **Run in an Ubuntu 24.04 cloud container,** with the build's dependencies from apt. The workspace's tests, caminito's 1833, and the ignored drawer tests pass. The patch series was applied to the 1.29.1 tag with `patch -Np1` but not built, and no PKGBUILD was run: `makepkg` and a Session with two monitors are the developer's, in the checklist item and in 220. `tools/check`'s meson, cargo-deny and extension steps weren't run, and its clippy step fails on three `manual_range_contains` in `caminito-theme`, which the base has too with clippy 1.97; the crates this ticket touches are clippy-clean.

