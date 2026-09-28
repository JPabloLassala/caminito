# Crate and module architecture

Type: grilling
Status: resolved
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

How is Camino's code split up? Decide:

- the crates, for example the compositor binary, the theme engine (07), the `camino-gtk-palette` helper (07), a policy crate with no Smithay dependency, the protocol implementations Camino builds (06), the D-Bus services (`org.mate.Camino`, the `org.mate.ScreenSaver` shim, `GrabAccelerators`) and the settings layer (09);
- the core state and event-loop shape (calloop), and where the scene graph (03) lives;
- how window-management policy sees windows independently of whether they are xdg or X11;
- the ownership boundaries that make 14's test seams work.

Look at the shapes of niri, cosmic-comp and anvil (03), and decide which to follow and where to depart from them.

## Answer

Decided on 2026-09-28. The user asked for the recommended answer on every question. Inputs were what [07](07-decoration-strategy.md), [11](11-xwayland-integration.md), [12](12-screen-locking.md), [13](13-hidpi-and-accessibility-scope.md) and [14](14-development-and-testing-loop.md) handed on, and a source read of niri `1f03391`, cosmic-comp `9f746ea` and Smithay `928d4a9` (anvil included), the same revisions as 14. The relevant facts are quoted inline.

The crate boundaries and the single-threaded loop are recorded in [ADR 0010](../../../docs/adr/0010-one-compositor-crate-with-protocol-free-policy-and-theme.md).

### Summary

| Item | Decision |
|---|---|
| Workspace | a virtual Cargo workspace with four crates: `camino`, `camino-policy`, `camino-theme` and `camino-test-client` |
| Crate rule | a crate exists only where it enforces a dependency rule; everything else is a module of `camino` |
| C programs | `helpers/camino-gtk-palette`, `helpers/camino-locker` (shipped) and `tools/marco-frame-dump` (not shipped), each its own Meson project, never built by cargo |
| Protocols Camino builds | modules in `camino/src/protocols/`, a `FooState` plus `FooHandler` trait each, as in Smithay and niri |
| D-Bus | modules in `camino/src/dbus/`, async zbus on zbus's own executor thread, talking to the loop over calloop channels |
| Settings | a module; GSettings lives on one GLib thread, and the translation to typed config is pure and unit-tested |
| Core state | `State { backend: Backend, camino: Camino }` in one `EventLoop<State>` (niri's shape); `Backend` is an enum |
| Threads | one main thread for Wayland, X11, input, policy and rendering; other threads only for zbus's executor and GLib's main loop |
| Window abstraction | `Mapped` wraps Smithay's `desktop::Window`, which already covers xdg and X11, and implements `PolicyWindow` |
| Policy | `camino-policy`: `Policy<W: PolicyWindow>` owns windows, workspaces, stacking, focus and placement; uses Smithay only for geometry types |
| Scene | no retained scene graph and no `desktop::Space`; per-output render elements rebuilt each frame from a fixed layer order |
| Outward views | foreign-toplevel, ext-workspace, EWMH root properties, D-Bus properties and the debug dump are refreshed from policy state in one pass per loop iteration |
| Tests | policy tests in `camino-policy`; the headless and X11 tiers in `camino/src/tests/` (`cfg(test)`); golden tests in `camino-theme/tests/`, golden files in `tests/goldens/`; the client in `camino-test-client` |

### Reference shapes

- **niri** is one compositor crate (about 84k lines) with `lib.rs` and `main.rs`, plus `niri-config`, `niri-ipc` and `niri-visual-tests`. `State { backend: Backend, niri: Niri }` (`niri.rs:722-725`) runs in one `EventLoop<State>` (`main.rs:175`, `:279`). `Niri` has about 150 fields. Its layout is generic over `LayoutElement`, but that trait has 44 items and takes `WlSurface`, `Output` and renderer types (`layout/mod.rs:46-51`, `:131-340`), so it isn't Smithay-free. Integration tests live in the crate at `src/tests/`, with no `tests/` directory. D-Bus is blocking zbus on zbus's thread, reaching the loop through `calloop::channel` (`dbus/mod.rs:59-89`). The headless backend isn't gated; a runtime flag picks it (`niri.rs:744-747`).
- **cosmic-comp** is one crate of about 72k lines, plus a config crate. `State { backend, common }` (`state.rs:224-230`) with a `BackendData` enum. The shell is shared as `Arc<RwLock<Shell>>` with one render thread per KMS surface (`backend/kms/surface/mod.rs:260`). Window policy sits on `Space<CosmicMapped>` with themes and animations inside it (`shell/layout/floating/mod.rs:56-65`), and there are almost no tests. D-Bus is async zbus on calloop's futures executor, with some `block_on` calls on the main thread (`dbus/mod.rs:39-50`, `logind.rs:9,26`).
- **anvil** (about 10k lines) is generic over a thin `Backend` trait (`state.rs:1206-1213`), and uses one global `Space<WindowElement>` with no workspaces.
- **Common ground:** all three wrap Smithay's `desktop::Window` rather than defining an xdg/X11 enum (`CosmicSurface(pub Window)`, `surface.rs:157`; anvil's `WindowElement(pub Window)`, `element.rs:40`; niri's `Mapped`, `mapped.rs:51-52`). None has its own X connection.

**Camino follows niri** for the state shape, the backend enum, handler layout, in-crate integration tests, D-Bus threading and per-frame rendering. It departs in three places:

- the policy is in its own crate and is Smithay-free apart from geometry (14);
- there are no render threads and no shared `RwLock` state, unlike cosmic-comp;
- 03 named cosmic-comp and anvil as the closest shapes, and said Camino owns the scene graph. Camino still owns the scene (below), but as per-frame assembly in niri's style rather than a retained graph;
- Camino has its own X connection (11), which nobody else has.

### Crates

A crate exists only where it enforces a dependency rule. Everything else is a module of `camino`, because modules can be split out later but a premature crate boundary forces public APIs everywhere.

```
Cargo.toml                    virtual workspace; smithay pinned by rev (14)
camino/                       the compositor: lib.rs + main.rs
camino-policy/                window-management policy
camino-theme/                 Marco theme engine
camino-test-client/           wayland-client and x11rb test clients (publish = false)
helpers/camino-gtk-palette/   C, Meson
helpers/camino-locker/        C, Meson
tools/marco-frame-dump/       C, Meson, never packaged
tools/dev-nested              the nested-run script (14)
resources/                    vendored protocol XML (gtk-shell.xml), the org.mate.Camino schema
tests/                        shared test data: schemas/, themes/, fonts/, goldens/ (14)
docs/                         ADRs, docs/testing/checklist.md
```

- **`camino`** is the compositor, as a library plus a small `main.rs`, as niri does. The library isn't a public API. It exists so in-crate tests and `main.rs` share one build.
- **`camino-policy`** depends on `smithay` with `default-features = false`, for `Rectangle`, `Point`, `Size`, `Logical` and `Physical` only. `smithay::utils` is ungated (Smithay's `src/lib.rs:71` and `src/utils/mod.rs:26-28`), so this builds without wayland-server. It still pulls in a few dependencies unconditionally, such as libxkbcommon (the `[dependencies]` table of Smithay's `Cargo.toml`), which is accepted. It has no other Wayland, X11, GLib or D-Bus dependency. A protocol type can't be named in it, so the seam from 14 holds at compile time.
- **`camino-theme`** depends on cairo-rs, pango, pangocairo, librsvg and image, and has no Smithay, GTK, Wayland or D-Bus dependency. It owns the parser, model, expressions, geometry and draw ops, plus the `Palette` type, the helper's stdout format and the on-disk cache format, so the palette test from 14 lives here. The golden tests are in `camino-theme/tests/`.
- **`camino-test-client`** is a dev-dependency of `camino`. It holds the `wayland-client` test client for the protocols 14 lists, an `x11rb` client, and zbus proxies for Camino's D-Bus names. It needs no Camino internals, so wayland-client stays out of the compositor's normal dependencies.
- **Rejected crates:**
  - a protocol crate, because each protocol is a `FooState` plus `FooHandler` and can be moved out later with no change of shape;
  - a D-Bus crate, because every interface is a thin forwarder into `State`;
  - a settings or config crate like `niri-config`, because the settings layer is GSettings plus a translation, and its only consumer is `camino`;
  - an IPC crate, because D-Bus is Camino's IPC (ADR 0002), and the debug dump is JSON over `org.mate.Camino.Debug` (14).

**C programs** each get a Meson project and are never built by cargo or a `build.rs`. `camino-gtk-palette` and `camino-locker` live in `helpers/` because they're Helpers (CONTEXT.md) and ship. `marco-frame-dump` lives in `tools/` because it only makes test data and never ships (14). How they're built and installed together is 16's.

### Modules of `camino`

| Module | Holds |
|---|---|
| `main.rs`, `logging` | argument parsing (`--session`), `tracing` set-up (journald in a session, stderr otherwise, 14) |
| `state` | `State`, `Camino`, `State::new`, the refresh pass |
| `backend` | `Backend { Tty, Winit, Headless }`; `tty` (DRM/KMS, libinput, libseat), `winit`, `headless` (`cfg(test)`) |
| `handlers` | `impl FooHandler for State` for Smithay's protocols and Camino's own, one file per area (niri's `handlers/`) |
| `protocols` | `foreign_toplevel`, `ext_workspace`, `output_management`, `gtk_shell`; `raw` generates `gtk_shell1` from `resources/gtk-shell.xml` with `wayland-scanner` |
| `window` | `Mapped` (Smithay `Window` plus Camino's per-window data), `impl PolicyWindow for Mapped`, window rules |
| `layer` | layer surfaces through Smithay's `layer_map_for_output` |
| `scene` | the layer order and per-output render-element assembly (below) |
| `decoration` | frame strips as cached `MemoryRenderBuffer`s, button hover and press state, frame input regions, the palette helper's output and cache (07) |
| `input` | libinput device settings, the keyboard path, pointer focus, move and resize grabs (which ask policy for snapping and resistance) |
| `input::accessx` | sticky, slow and bounce keys as a pure filter over key events and time (13) |
| `input::bindings` | Camino's keybindings and accelerators grabbed through `GrabAccelerators` |
| `output` | output lifecycle, the default-scale rule as a pure function (ADR 0008), the X11 scale |
| `xwayland` | spawn, `Ready`, the restart path (11) |
| `xwayland::xwm` | `impl XwmHandler for State`: XWayland windows into `Mapped` and policy |
| `xwayland::xconn` | Camino's X connection (x11rb) as a calloop fd source: desktops, work area, frame extents, struts, bell, the root messages Smithay drops. It never writes what Smithay maintains, such as `_NET_SHOWING_DESKTOP` or XSETTINGS (11) |
| `xwayland::xsettings` | the XSETTINGS table (msd's translation plus the X11 scale), applied through Smithay's `X11Wm::set_xsettings` and re-applied after every `start_wm` (ADR 0006, 11) |
| `lock` | the lock state machine (blank, pending, locked, locker-lost) as a pure type, and the code that applies it (12) |
| `idle` | idle timers, blanking, DPMS, inhibition from idle-inhibit and mate-session's `IsInhibited` |
| `helpers` | one supervisor for the palette helper and the locker: spawn, stderr to the journal under the helper's identifier, restart with backoff, give up. Xwayland, the third Helper, is spawned by Smithay in `xwayland`, and only its stderr goes through `helpers` |
| `session` | `--session` start-up order: environment export and the systemd check (08, 14), then waiting until `dbus` owns its names, then mate-session as a child; its exit ends the Session |
| `settings` | the GLib thread with every `GSettings` object, the pure translation into typed config, "unsupported key" logging (09) |
| `dbus` | one file per name or peer (below) |
| `debug` | the debug toggles' state and building the JSON state dump (14); Tracy spans behind `profile-with-tracy`. `dbus::debug` only exposes them |
| `tests` | `cfg(test)`: the fixture, the headless and X11 tiers (14) |

**The keyboard path** is one function in `input` that calls its stages in 13's order, each returning whether the key goes further:

1. `lock`: while locked, only VT and layout switching, AccessX and the keyboard monitor pass (12, 13);
2. `input::accessx`, before xkbcommon;
3. `dbus::a11y`'s keyboard monitor grabs and watches;
4. `input::bindings`, Camino's own bindings and then `GrabAccelerators`;
5. Smithay's `KeyboardHandle` to the focused client.

**D-Bus** files in `dbus/`:

| File | Name or peer |
|---|---|
| `camino.rs` | `org.mate.Camino`: `ShowingDesktop`, `GetActiveToplevel`, `GrabAccelerators` and `AcceleratorActivated` (06) |
| `debug.rs` | `org.mate.Camino.Debug`, a thin interface over the `debug` module (14) |
| `screensaver.rs` | the Screensaver shim: `org.mate.ScreenSaver` and `org.freedesktop.ScreenSaver` (12) |
| `a11y.rs` | `org.freedesktop.a11y.Manager`, the Keyboard monitor (13) |
| `logind.rs` | the system bus: `Lock` and `Unlock`, `SetLockedHint`, the sleep inhibitor and `PrepareForSleep` (12) |
| `session_manager.rs` | mate-session's `IsInhibited` (06) |
| `panel.rs` | calls to `org.mate.Panel` `MainMenu` and `RunDialog` (06) |
| `systemd.rs` | the environment import, skipped when `org.freedesktop.systemd1` isn't on the bus (08, 14) |

### Core state and the event loop

- **`State { backend: Backend, camino: Camino }`** in one `EventLoop<State>`, as niri does. `Camino` holds the display handle, the loop handle, Smithay's protocol states, `policy: Policy<Mapped>`, outputs, the seat, the lock, idle, XWayland, the current config, decoration caches, the helpers, the D-Bus handles and the debug toggles. Smithay's `delegate_dispatch2!(State)` covers dispatch (niri's `niri.rs:443`).
- **`Backend` is an enum**, as in niri and cosmic-comp, not anvil's generic trait. A generic `State<B>` would put a type parameter on every handler for three backends. The enum's `Headless` variant exists only under `cfg(test)`.
- **Everything that touches Wayland, X11, input, policy or rendering runs on the main thread.** Camino's X connection is a calloop fd source there too, so `xwm` and `xconn` both work on `&mut State` and share window state without locks (11's "sharing Camino's window state").
- **Threads exist only where a library needs its own loop.** Each hands results to the main loop through a `calloop::channel`, and the main thread never blocks on them:
  - **zbus:** async zbus on its own executor thread. Interface methods are thin. They send a request with a reply sender into the loop and await the reply, as niri does with blocking zbus (`dbus/mod.rs:97-111`). Signals and property changes go the other way from the refresh pass. cosmic-comp's `block_on` on the main thread is avoided, because the key path emits keyboard-monitor signals and must never wait on the bus.
  - **GLib:** one thread runs a GLib main loop and owns every `GSettings` object, since GSettings change notifications need an iterated main context. At start-up, `State::new` waits for its first full snapshot, so the config is complete before anything maps. After that it sends typed changes. The rare writes Camino makes go back through `MainContext::invoke`.
- **No render threads.** cosmic-comp runs one per KMS surface with the shell behind `Arc<RwLock<Shell>>`. niri shows a single thread is enough for a desktop compositor, and it keeps policy and scene state free of locks.
- **Helpers** are child processes, not threads: the palette's stdout is a calloop fd source, and exits come through a `SIGCHLD` source.

### Policy and the window abstraction

- **The window type is `Mapped`**, which wraps Smithay's `desktop::Window`. `Window` already covers both xdg toplevels and XWayland windows (`X11Surface`), and all three reference compositors wrap it rather than defining their own enum. The few differences, such as configure versus `X11Surface::configure`, `close` versus `WM_DELETE_WINDOW`, and the window-type source, are `match`es on `underlying_surface()` inside `Mapped`.
- **`camino-policy` owns the model.** `Policy<W: PolicyWindow>` holds the windows, workspaces, per-workspace stacking with Marco's layers, focus, placement, transient rules, maximize, fullscreen, shade, minimize, show-desktop, edge snapping, and work areas from struts. It also defines the keybinding action enum, the policy's config struct, and output descriptors (id, logical rectangle, scale).
- **`PolicyWindow`** follows niri's shape: policy owns `W` values, reads facts from them, and calls methods on them to apply its decisions. The trait is small and names no protocol object:
  - **facts:** a stable id, the window type (normal, dialog, modal dialog, utility, splash and so on, 06), the transient parent's id, size hints, requested position, above, sticky and skip-taskbar hints, pid, app id or `WM_CLASS`, whether it's SSD, and its frame extents;
  - **requests:** set geometry and states, set activated, close, and set the output list the window is on.

  The frame extents come from `camino-theme` through `Mapped`, so policy places frames the way Marco does without depending on the theme crate.
- **Time is an argument.** Auto-raise, click-and-drag thresholds and the like take a `now` from the caller, so proptest sequences are deterministic.
- **Events in, effects out.** The compositor calls policy with an event (map, unmap, commit with new hints, click, action, output added or removed, settings changed). Policy updates its model and applies decisions through `W`. For 14's `TestWindow`, "apply" means recording the call.

### Scene

- **No retained scene graph and no `desktop::Space`.** Space has one global stacking and knows nothing of workspaces, Marco's layers, override-redirect windows or the lock. Camino builds each output's render elements every frame, as niri does, and Smithay's `OutputDamageTracker` keeps it cheap.
- **The layer order**, top to bottom:
  1. lock surfaces, or the solid lock background (12);
  2. the overlay layer-shell layer;
  3. override-redirect windows (11);
  4. the top layer-shell layer;
  5. the current workspace's windows in policy's stacking order, each with its frame strips and popups;
  6. the bottom layer-shell layer;
  7. the background layer-shell layer, with caja's `desktop` surface above msd's `mate-background` on the same output (06).
- `scene` is the only code that knows this order. The frame debug toggle (14) adds its outlines at assembly time.

### Outward views

Several outputs mirror policy state to the outside:

- wlr-foreign-toplevel and ext-foreign-toplevel-list;
- ext-workspace;
- the desktop atoms, `_NET_WORKAREA` and `_NET_FRAME_EXTENTS` on Camino's X connection;
- `_NET_SHOWING_DESKTOP` and the XSETTINGS table, including the X11 scale, through Smithay's `X11Wm` (11);
- `ShowingDesktop` and other D-Bus properties;
- the Debug dump.

None of them is updated inline from a handler. Handlers change policy state and mark it dirty. A **refresh pass** after each loop dispatch, niri's `refresh_and_flush_clients` (`main.rs:279`), compares against what each view last sent, sends the differences, then flushes the Wayland clients. So a view can't drift from policy, and a burst of changes becomes one update.

### Test seams

| Layer (14) | Lives in | Seam |
|---|---|---|
| Policy unit and proptest | `camino-policy/src/` | `TestWindow: PolicyWindow`, no Smithay protocol types possible |
| AccessX, the default-scale rule, settings translation, the lock state machine | beside their code in `camino` | pure functions or types, unit tests in the module |
| Headless integration | `camino/src/tests/` | `Backend::Headless` (`cfg(test)`) with pixman and virtual outputs; `Fixture` runs `State` and `camino-test-client` in one calloop over `UnixStream::pair()` |
| X11 tier | `camino/src/tests/` | the headless fixture plus a real Xwayland and `camino-test-client`'s x11rb client |
| D-Bus | `camino/src/tests/` | `camino-test-client`'s proxies on the test binary's private bus |
| Theme goldens and palette | `camino-theme/tests/` | the committed files under `tests/goldens/` (ADR 0009) |

- **Integration tests live in `camino/src/tests/`, not `camino/tests/`,** as in niri, because they need `State` and `cfg(test)`-only hooks. This makes `camino`'s lib tests one test binary, so D-Bus isolation is one `dbus-run-session` around it (14).
- **The headless backend is `cfg(test)`, not a Cargo feature.** This changes 14's "behind a test feature" on purpose. Every user of the backend is in `camino`'s own tests, so a feature would only add a flag to every test command. Smithay's `renderer_pixman` is enabled through `camino`'s dev-dependencies, so the shipped binary contains neither, as 16 requires. If a tool outside the crate ever needs the backend, it becomes a feature then.

### Handed on

- **16 Packaging:**
  - the Cargo workspace plus three Meson projects: `helpers/camino-gtk-palette`, `helpers/camino-locker` and `tools/marco-frame-dump`, the last never packaged;
  - `resources/` holds the `org.mate.Camino` schema;
  - the headless backend and pixman renderer aren't in the release build.
- **17 Milestone slicing:** the first milestone creates the workspace, the four crates, the `State` and `Backend` skeleton with `Headless`, `PolicyWindow` with `TestWindow`, the refresh pass, and the CI from 14.
