# Coding standards

How Caminito's code is written. Reviews check a diff against this file, [CONTEXT.md](CONTEXT.md) for naming, and the ADRs in [docs/adr/](docs/adr/) for decisions. Where this file and an ADR disagree, the ADR wins and this file is out of date.

## Enforced by `tools/check`

- `cargo fmt --all --check`, with rustfmt's defaults. There is no `rustfmt.toml`.
- `cargo clippy --workspace --all-targets -- -D warnings`, so every clippy warning fails the build. Fix the code rather than silencing the lint. If an allow is truly needed, use `#[expect(clippy::…, reason = "…")]` on the smallest item.
- `[workspace.lints]` in [Cargo.toml](Cargo.toml) adds `clippy::undocumented_unsafe_blocks`. Every crate opts in with `lints.workspace = true`.
- `caminito-policy`, `caminito-layout` and caminito-displays's library are `#![forbid(unsafe_code)]`.
- `cargo deny check` against [deny.toml](deny.toml) for licences and advisories.

## Crates and modules

The layout is ticket 15's ([.scratch/issues/15-crate-and-module-architecture.md](.scratch/issues/15-crate-and-module-architecture.md)) and [ADR 0010](docs/adr/0010-one-compositor-crate-with-protocol-free-policy-and-theme.md), with `caminito-layout` from [ADR 0015](docs/adr/0015-the-layout-logic-is-a-crate-of-its-own.md).

- A crate exists only where it enforces a dependency rule. Everything else is a module of `caminito`.
- `caminito-policy` names no protocol object. It may use Smithay's geometry types and nothing else from Smithay. It reaches windows only through `PolicyWindow`.
- `caminito-theme` knows nothing of Wayland.
- `caminito-layout` has no dependencies, and gets none (ADR 0015). caminito-displays depends on it too.
- `caminito-displays` is a program of its own (ADR 0014), and depends on `caminito-layout` alone of Caminito's crates. Its library, the page's state and its client of output management, names no toolkit, so that Caminito's headless tests can drive it. GTK is the binary's.
- Protocols Caminito implements itself go in `caminito/src/protocols/`, each as a `FooState` plus a `FooHandler` trait. `output_management` is the first. D-Bus interfaces go in `caminito/src/dbus/`.
- `scene` is the only code that knows the layer order.
- Handlers change policy state. The refresh pass after each dispatch queues the redraw for a Policy change (`Policy::take_scene_changed`) and updates the outward views (foreign-toplevel, ext-workspace, EWMH properties, D-Bus properties, the debug dump); a handler never updates one inline. A handler queues a redraw only for what Policy doesn't hold: the lock, idle's blank, the bell, a Layer surface, the cursor, a selection, a client's commit, an output's change, a popup's own buffer.
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

- Use `tracing`, not `log` or `println!`. caminito-displays, which GLib's main loop runs, tells the user in its window and warns through `glib::g_warning!`.
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
- When reference code is copied or adapted (niri, anvil, cosmic-comp, Marco), cite the source file and line.

## Tests

Where each tier lives, per ticket 15's test seams:

| What | Where |
|---|---|
| Policy, including proptest | `caminito-policy/src/tests/`, against `TestWindow` |
| Docking and healing a layout, including proptest | `caminito-layout/src/tests.rs` |
| caminito-displays's page, including proptest | `caminito-displays/src/page/tests.rs` |
| caminito-displays's page and client against Caminito | `caminito/src/tests/caminito_displays.rs`, through `Fixture` |
| Pure logic in `caminito` (settings translation, AccessX, scale rules, the lock state machine) | a `tests` module beside the code |
| Headless integration, X11 tier, D-Bus | `caminito/src/tests/`, through `Fixture` |
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
