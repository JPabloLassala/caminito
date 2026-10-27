# Coding standards

How Caminito's code is written. Reviews check a diff against this file, [CONTEXT.md](CONTEXT.md) for naming, and the ADRs in [docs/adr/](docs/adr/) for decisions. Where this file and an ADR disagree, the ADR wins and this file is out of date.

## Enforced by `tools/check`

- `cargo fmt --all --check`, with rustfmt's defaults. There is no `rustfmt.toml`.
- `cargo clippy --workspace --all-targets -- -D warnings`, so every clippy warning fails the build. Fix the code rather than silencing the lint. If an allow is truly needed, use `#[expect(clippy::…, reason = "…")]` on the smallest item.
- `[workspace.lints]` in [Cargo.toml](Cargo.toml) adds `clippy::undocumented_unsafe_blocks`. Every crate opts in with `lints.workspace = true`.
- `caminito-policy`, `caminito-layout`, `caminito-version`, caminito-about, caminito-displays's library, caminito-wallpaper's library and dialog, caminito-behaviour, caminito-bluetooth, caminito-network and caminito-notifications's library are `#![forbid(unsafe_code)]`. caminito-notifications's binary calls libcanberra, as `caminito` does.
- `cargo deny check` against [deny.toml](deny.toml) for licences and advisories.
- `reuse lint`: every file has a licence and a copyright, from [REUSE.toml](REUSE.toml) or its own SPDX header, and each licence's text is in [LICENSES/](LICENSES/) (ADR 0051).

## Crates and modules

The layout is ticket 15's ([.scratch/issues/15-crate-and-module-architecture.md](.scratch/issues/15-crate-and-module-architecture.md)) and [ADR 0010](docs/adr/0010-one-compositor-crate-with-protocol-free-policy-and-theme.md), with `caminito-layout` from [ADR 0015](docs/adr/0015-the-layout-logic-is-a-crate-of-its-own.md).

- A crate exists only where it enforces a dependency rule. Everything else is a module of `caminito`.
- `caminito-policy` names no protocol object. It may use Smithay's geometry types and nothing else from Smithay. It reaches windows only through `PolicyWindow`.
- `caminito-theme` knows nothing of Wayland.
- `caminito-layout` has no dependencies, and gets none (ADR 0015). caminito-displays depends on it too.
- `caminito-displays` is a program of its own (ADR 0014), and depends on `caminito-layout` alone of Caminito's crates. Its library, the page's state, its client of output management and the Night Light tab's state (142), names no toolkit, so that Caminito's headless tests can drive it. GTK is the binary's.
- `caminito-version` is the version string and the build script that finds the build's revision (182). `caminito` and `caminito-about` both depend on it, so `caminito --version` and the dialog can't disagree. It has no dependencies.
- `caminito-about` is a program of its own (182), since the compositor doesn't link GTK and caminito-displays is the display settings program. It depends on `caminito-version` alone of Caminito's crates.
- `caminito-wallpaper` is the wallpaper's programs (ADR 0031): its drawer, `caminito-wallpaperd`, which a Session starts, and its dialog, `caminito-wallpaper` (212), in GTK4 without libadwaita. It depends on none of Caminito's crates. The drawer links no GTK: GLib's loop and GSettings through gio, gdk-pixbuf and cairo. The crate depends on GTK for the dialog alone, and a binary links only the crates it uses, so the library names no GTK and `ldd caminito-wallpaperd` shows none. Its library's pure parts (a monitor's identity, the positioning, the settings an output gets, a dynamic wallpaper's picture for a moment, a HEIC's metadata, a slideshow's picture for a time and an output, a picture of the day's sources and cache, the dialog's page and the grids' groups) name no toolkit and are unit-tested beside the code.
- `caminito-behaviour` is Workspace Behaviour (CMN-184), the settings program for how Caminito itself behaves, in GTK4 without libadwaita: a tab for each page, Hot corners and Key remaps (CMN-189) now. It depends on none of Caminito's crates, and reads and writes `org.mate.Caminito`'s keys, which Caminito follows as they change. Its library, each page's keys through gio, the Remap templates' files, the System shortcuts from Marco's and Caminito's keys, and the open windows' app ids on a Wayland connection of its own, names no toolkit; GTK is the binary's.
- `caminito-bluetooth` is the Bluetooth program (255, ADR 0035): the pairing agent and the settings window, in GTK4 without libadwaita, in one single-instance process through the session. It depends on none of Caminito's crates, and works under X11 MATE too. Its library, BlueZ's model over gio's D-Bus (no zbus), the rfkill block, the switch, the pairing agent behind its `Dialogs` seam (256) and the settings window's state (257), names no toolkit; GTK is the binary's, the agent's dialogs too.
- `caminito-network` is NetworkManager's secret agent in a Session (ADR 0041), in GTK4 without libadwaita, in one single-instance process that exits at once outside Caminito. It depends on none of Caminito's crates. Its library, the secret agent over gio's D-Bus (no zbus, no libnm) behind its `Dialogs` seam and the VPN plugins' auth-dialogs it runs (CMN-198), the agent-owned secrets it keeps in the Secret Service under nm-applet's attributes (CMN-197), and the connection notifications (CMN-199, ADR 0044), names no toolkit; GTK is the binary's, the dialogs too.
- `caminito-notifications` is the notification daemon of a Session (ADR 0051, CMN-258), a Helper of Caminito's, in GTK4 without libadwaita, in one single-instance process. It depends on none of Caminito's crates. Its library, the Desktop Notifications server over gio's D-Bus (no zbus) behind its `Bubbles` and `Sounds` seams, and a body's markup made safe for Pango, names no toolkit; GTK, the bubbles' layer surface through gtk4-layer-shell, and libcanberra are the binary's.
- Protocols Caminito implements itself go in `caminito/src/protocols/`, each as a `FooState` plus a `FooHandler` trait. `output_management` is the first. D-Bus interfaces go in `caminito/src/dbus/`.
- `scene` is the only code that knows the layer order.
- Handlers change policy state. The refresh pass after each dispatch queues the redraw for a Policy change (`Policy::take_scene_changed`) and updates the outward views (foreign-toplevel, ext-workspace, EWMH properties, D-Bus properties, the debug dump); a handler never updates one inline. A handler queues a redraw only for what Policy doesn't hold: the lock, idle's blank, the bell, a Layer surface, the cursor, a selection, a client's commit, an output's change, a Frame's look, a popup's own buffer.
- `scene` reads Policy only through the queries that `SCENE_QUERIES` lists in `caminito-policy/src/tests/invariants.rs`. A new query goes in that list, in `SceneView` beside it and in `SceneReads` in `policy.rs`, or its change isn't drawn; a test holds the list to `scene.rs`.
- Wayland, X11, input, policy and rendering all run on the main thread. A new thread is justified only when a library needs its own loop. It talks to the main loop through a `calloop::channel`, and the main thread never blocks on it.
- Policy takes time as an argument (`now`) and never reads a clock, so tests and proptest sequences are deterministic.
- A feature that waits on Caminito's clock gives a `deadline()` and an `elapse(now)` and joins the list in `deadlines`. It sets a calloop `Timer` of its own only when it waits on a process or a protocol object.
- Don't call `std::env::set_var` once other threads are running. Put variables for spawned clients in `Caminito::environment`.
- Keep items private. Use `pub(crate)` for crate-internal sharing, and `pub` only for what another crate or `main.rs` uses.

## Errors

- There is no `anyhow` or `thiserror`. A module that can fail defines its own error enum with a documented variant for each cause, a hand-written `Display` and `impl std::error::Error`. `settings::Error` is the model.
- `Display` messages are lowercase with no trailing period, and they read as the cause: `can't start the settings thread: {err}`.
- `?` passes errors up to `run` and `main`, which log them. Returning `Box<dyn Error>` is fine at that top level only.
- A feature Caminito can run without (Xwayland, D-Bus, a helper) logs one `warn!` line when it fails and Caminito carries on. It doesn't abort start-up.
- Use `expect` for invariants Caminito holds itself, with a message that states the invariant as a fact: `.expect("the display joins the event loop")`, not `"failed to insert source"`.
- A bare `unwrap()` outside tests is acceptable only where the value is guaranteed by construction and a message would add nothing: `seat.get_pointer()` after the seat was built with one, or `Mutex::lock` on a lock that is never poisoned. Anything a client, the user's settings or the environment can cause is handled, not unwrapped.

## Logging

- Use `tracing`, not `log` or `println!`. `caminito --version` prints its answer, which isn't a log line, with `println!` (182). caminito-displays, which GLib's main loop runs, tells the user in its window and warns through `glib::g_warning!`. caminito-wallpaperd, the Wallpaper dialog, caminito-bluetooth, caminito-network and caminito-notifications, which GLib's main loop runs too, log through GLib's macros under their own domains, with the same levels.
- Messages are lowercase phrases. Failures begin `can't …` and end with the error: `tracing::warn!("can't load the icon {}: {err}", name)`.
- Use `warn!` for a degraded feature, `error!` for what stops Caminito, `info!` for a few start-up facts (the socket, `DISPLAY`), and `debug!` or `trace!` for the rest. Nothing in a per-frame path logs above `debug!`.

## `unsafe`

- Every `unsafe` block has a `// Safety:` comment right above it saying why the call is sound here (clippy enforces this).
- Prefer a safe Smithay or rustix API over `unsafe`, even if it costs a copy.

## Comments and docs

- Every module starts with a `//!` doc saying what it owns. Public items get a `///` doc.
- Comments say *why*, or name the constraint the code respects. They don't narrate *what* the code does.
- Refer to a ticket by its number in parentheses, e.g. `(14)`, and to an ADR by its number. If a ticket's decision was changed on purpose, the comment says so.
- Use CONTEXT.md's terms and avoid the ones it lists under *Avoid*. For example: compositor, never "Wayland window manager"; carried patch, never "fork".
- Copy or adapt reference code only from a project whose licence allows GPL-3.0-or-later (niri, anvil, Marco, mate-screensaver), keep its notice, give the file SPDX lines with its original copyright, Caminito's and both licences (as `GPL-3.0-or-later AND MIT`), and cite the source file and line. From cosmic-comp, which is GPL-3.0-only, take behaviour only, never code (ADR 0051).

## Tests

Where each tier lives, per ticket 15's test seams:

| What | Where |
|---|---|
| Policy, including proptest | `caminito-policy/src/tests/`, against `TestWindow` |
| Docking and healing a layout, including proptest | `caminito-layout/src/tests.rs` |
| caminito-displays's page, including proptest | `caminito-displays/src/page/tests.rs` |
| caminito-displays's Night Light tab: the sliders a schedule shows, the pause and the note | `caminito-displays/src/night_light/tests.rs` |
| caminito-displays's page and client against Caminito | `caminito/src/tests/caminito_displays.rs`, through `Fixture` |
| The Wallpaper dialog's page, and the Picture and Dynamic grids' groups | `caminito-wallpaper/src/page/tests.rs` and `caminito-wallpaper/src/grid.rs` |
| A dynamic wallpaper's picture and blend for a moment, against the installed `cosmos` XML | `caminito-wallpaper/src/dynamic/tests.rs` |
| A slideshow's picture for a time and an output's rank, with the clock passed in | `caminito-wallpaper/src/slideshow/tests.rs` |
| A HEIC's metadata from its container, and its picture for a position of the sun or a time, against fixtures made in the test and one HEIC made by libheif | `caminito-wallpaper/src/heic/tests.rs` |
| Each picture of the day's source against a saved answer, and the cache against a fake of the network | `caminito-wallpaper/src/daily/tests.rs` |
| The Wallpaper dialog's client of output management and its page against Caminito | `caminito/src/tests/caminito_wallpaper.rs`, through `Fixture` |
| The real caminito-wallpaperd against Caminito | `caminito/src/tests/caminito_wallpaperd.rs`, through `Fixture`, ignored unless `CAMINITO_WALLPAPERD` names the drawer |
| The Sound popup's wheel over a slider, on GDK's Wayland backend against Caminito (CMN-215) | `caminito/src/tests/sound_popup_wheel.rs`, through `Fixture`, ignored unless `CAMINITO_SOUND_WHEEL_PROBE` names the Sound applet's built `sd-wheel-probe` |
| The Dock applet's items, clicks, menu and minimise target, its launch token, pulse, bounce and highlight, and its Trash over gvfs's `trash:///` on a session bus of each test's own, on GDK's Wayland backend with libxfce4windowing against Caminito (CMN-219, CMN-220, CMN-221) | `caminito/src/tests/dock_applet.rs`, through `Fixture`, ignored unless `CAMINITO_DOCK_PROBE` names the Dock applet's built `dk-probe` |
| Workspace Behaviour's Hot corners keys read, written and followed, against `data/`'s schema compiled by the test, over GSettings' memory backend | `caminito-behaviour/src/hot_corners/tests.rs` |
| Workspace Behaviour's Key remaps keys read, written and followed, and what an app id gets from them, over the same compiled schema (`caminito-behaviour/src/testing.rs`) | `caminito-behaviour/src/key_remaps/tests.rs` |
| Remap templates parsed, written and read back, listed from Caminito's and the user's directories in a temporary directory, saved and deleted | `caminito-behaviour/src/remap_templates/tests.rs` |
| The System shortcuts the Key remaps page marks, from Marco's accelerators and Caminito's keys | `caminito-behaviour/src/system_shortcuts/tests.rs` |
| BlueZ's model, the rfkill block, the switch, the pairing agent and the settings window's state against python-dbusmock's `bluez5` template on a private bus, with the rfkill block and the agent's dialogs faked and the test calling the agent as BlueZ would | `caminito-bluetooth/tests/`, through `mock::run`; the rfkill block's sysfs and event, and a device's kind, beside the code |
| caminito-bluetooth's single instance, the real binary | `caminito-bluetooth/tests/single_instance.rs`, ignored unless run under a display, as tools/check runs it under Xvfb |
| caminito-network's secret agent against python-dbusmock's `networkmanager` template, with an AgentManager added, on a private bus, with the dialogs faked, a fake Secret Service on the same bus (`tests/mock/secret_service.rs`), and the test calling the agent as NetworkManager would, and a VPN's secrets through a fake auth-dialog (`tests/fake-auth-dialog`) | `caminito-network/tests/agent.rs`, through `mock::run`; a `.name` file's lookup and the auth-dialog's input and output beside the code |
| caminito-network's connection notifications against python-dbusmock's `networkmanager` and `notification_daemon` templates on a private bus, with the test emitting each active connection's and device's `StateChanged` as NetworkManager would, with its reasons, and calling `UserActivating` as the Network applet would | `caminito-network/tests/notifications.rs`, through `mock::run` |
| caminito-network's single instance and its exit outside Caminito, the real binary | `caminito-network/tests/single_instance.rs`; the single instance ignored unless run under a display, as tools/check runs it under Xvfb |
| caminito-notifications's server on a private bus of each test's own, called as an app would and with the signals the app gets, with the bubbles and the sounds faked and the fake acting on a bubble as the user would (CMN-258) | `caminito-notifications/tests/server.rs`, through `bus::run`; a body's markup and a timeout's rules beside the code |
| Pure logic in `caminito` (settings translation, AccessX, scale rules, the lock state machine) | a `tests` module beside the code |
| Intellihide's hiding and the pointer's rest at the edge, against `TestWindow` (CMN-224) | `caminito-policy/src/tests/intellihide.rs` |
| Intellihide's protocol and slide, through the test client: the debug dump's account of where a panel is drawn, and pixels | `caminito/src/tests/intellihide.rs`, through `Fixture` |
| The mate-panel patch's own `wayland-intellihide.c` understood by Caminito, in a probe on GDK's Wayland backend (CMN-224) | `caminito/src/tests/mate_panel_intellihide.rs`, through `Fixture`, ignored unless `CAMINITO_INTELLIHIDE_PROBE` names the probe `.scratch/checks/CMN-224/run.sh` builds from the patch |
| Headless integration, X11 tier, D-Bus | `caminito/src/tests/`, through `Fixture` |
| The version string, and where a build's revision comes from | `caminito-version/src/string.rs`, which the build script compiles too |
| The `caminito` binary's own arguments, such as `--version` | `caminito/tests/`, running the binary |
| Theme goldens and the palette | `caminito-theme/tests/`, with files under `tests/goldens/` (ADR 0009) |

- Test names are sentences about behaviour, in snake case: `xwayland_outlives_its_last_client_while_caminito_is_attached`.
- Test through the seam, the way a client or the user would see it: the Wayland or X11 client, D-Bus, the debug dump. Don't test through `State` internals when a seam can show the same thing.
- Map windows with `Fixture::map`, and read a window's facts (its geometry, frame extents, button rects, the frame control under the pointer, its `Mapped`) through the fixture's queries, not `state.caminito`. `map` returns once the window is managed, its frame is drawn and its client is in sync, so no test dispatches after it to let things settle.
- A test of behaviour both kinds of window share runs on both, through `on_both_kinds!`, as `<name>::xdg` and `<name>::x11`. A test or module that maps only one kind says why.
- Text output that's easier to read than to assert goes in an `insta` snapshot. Snapshots are committed, and their diffs are reviewed like code.
- The headless backend and `renderer_pixman` are `cfg(test)` and dev-dependencies only. Nothing test-only reaches the release binary.
- No sleeps to wait for a state. Dispatch the fixture until the condition holds. A fixed wait is justified only when the thing being tested is that nothing happens, and the comment says why the time is long enough.
- `unwrap()` and `expect` are fine in tests.

## Dependencies

- Shared dependencies are declared once in `[workspace.dependencies]`. Smithay is pinned by `rev` on one line, which the canary rewrites, and `smithay-drm-extras`, from the same repository, on the next, at the same `rev`.
- Add a dependency only when it earns its weight. It must pass `cargo deny`, and its features are enabled per crate, as narrowly as possible.
- A dev-only dependency never becomes a normal dependency just to make a test easier.

## Commits

- The subject is imperative and names the ticket: `Spawn Xwayland and add the X11 test tier (ticket 34)`.
- The body is prose. It says what changed and why, what was left for a later ticket (by number), and how it was tested.
