# Coding standards

How Camino's code is written. Reviews check a diff against this file, [CONTEXT.md](CONTEXT.md) for naming, and the ADRs in [docs/adr/](docs/adr/) for decisions. Where this file and an ADR disagree, the ADR wins and this file is out of date.

## Enforced by CI

- `cargo fmt --all --check`, with rustfmt's defaults. There is no `rustfmt.toml`.
- `cargo clippy --workspace --all-targets -- -D warnings`, so every clippy warning fails the build. Fix the code rather than silencing the lint. If an allow is truly needed, use `#[expect(clippy::…, reason = "…")]` on the smallest item.
- `[workspace.lints]` in [Cargo.toml](Cargo.toml) adds `clippy::undocumented_unsafe_blocks`. Every crate opts in with `lints.workspace = true`.
- `camino-policy` and `camino-layout` are `#![forbid(unsafe_code)]`.
- `cargo deny check` against [deny.toml](deny.toml) for licences and advisories.

## Crates and modules

The layout is ticket 15's ([.scratch/camino-v0.1/issues/15-crate-and-module-architecture.md](.scratch/camino-v0.1/issues/15-crate-and-module-architecture.md)) and [ADR 0010](docs/adr/0010-one-compositor-crate-with-protocol-free-policy-and-theme.md), with `camino-layout` from [ADR 0015](docs/adr/0015-the-layout-logic-is-a-crate-of-its-own.md).

- A crate exists only where it enforces a dependency rule. Everything else is a module of `camino`.
- `camino-policy` names no protocol object. It may use Smithay's geometry types and nothing else from Smithay. It reaches windows only through `PolicyWindow`.
- `camino-theme` knows nothing of Wayland.
- `camino-layout` has no dependencies, and gets none (ADR 0015). camino-displays depends on it too.
- Protocols Camino implements itself go in `camino/src/protocols/`, each as a `FooState` plus a `FooHandler` trait. `output_management` is the first. D-Bus interfaces go in `camino/src/dbus/`.
- `scene` is the only code that knows the layer order.
- Handlers change policy state and mark it dirty. They never update an outward view inline (foreign-toplevel, ext-workspace, EWMH properties, D-Bus properties, the debug dump). The refresh pass after each dispatch does that.
- Wayland, X11, input, policy and rendering all run on the main thread. A new thread is justified only when a library needs its own loop. It talks to the main loop through a `calloop::channel`, and the main thread never blocks on it.
- Policy takes time as an argument (`now`) and never reads a clock, so tests and proptest sequences are deterministic.
- Don't call `std::env::set_var` once other threads are running. Put variables for spawned clients in `Camino::environment`.
- Keep items private. Use `pub(crate)` for crate-internal sharing, and `pub` only for what another crate or `main.rs` uses.

## Errors

- There is no `anyhow` or `thiserror`. A module that can fail defines its own error enum with a documented variant for each cause, a hand-written `Display` and `impl std::error::Error`. `settings::Error` is the model.
- `Display` messages are lowercase with no trailing period, and they read as the cause: `can't start the settings thread: {err}`.
- `?` passes errors up to `run` and `main`, which log them. Returning `Box<dyn Error>` is fine at that top level only.
- A feature Camino can run without (Xwayland, D-Bus, a helper) logs one `warn!` line when it fails and Camino carries on. It doesn't abort start-up.
- Use `expect` for invariants Camino holds itself, with a message that states the invariant as a fact: `.expect("the display joins the event loop")`, not `"failed to insert source"`.
- A bare `unwrap()` outside tests is acceptable only where the value is guaranteed by construction and a message would add nothing: `seat.get_pointer()` after the seat was built with one, or `Mutex::lock` on a lock that is never poisoned. Anything a client, the user's settings or the environment can cause is handled, not unwrapped.

## Logging

- Use `tracing`, not `log` or `println!`.
- Messages are lowercase phrases. Failures begin `can't …` and end with the error: `tracing::warn!("can't load the icon {}: {err}", name)`.
- Use `warn!` for a degraded feature, `error!` for what stops Camino, `info!` for a few start-up facts (the socket, `DISPLAY`), and `debug!` or `trace!` for the rest. Nothing in a per-frame path logs above `debug!`.

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
| Policy, including proptest | `camino-policy/src/tests/`, against `TestWindow` |
| Docking and healing a layout, including proptest | `camino-layout/src/tests.rs` |
| Pure logic in `camino` (settings translation, AccessX, scale rules, the lock state machine) | a `tests` module beside the code |
| Headless integration, X11 tier, D-Bus | `camino/src/tests/`, through `Fixture` |
| Theme goldens and the palette | `camino-theme/tests/`, with files under `tests/goldens/` (ADR 0009) |

- Test names are sentences about behaviour, in snake case: `xwayland_outlives_its_last_client_while_camino_is_attached`.
- Test through the seam, the way a client or the user would see it: the Wayland or X11 client, D-Bus, the debug dump. Don't test through `State` internals when a seam can show the same thing.
- Map windows with `Fixture::map`, and read a window's facts (its geometry, frame extents, button rects, the frame control under the pointer, its `Mapped`) through the fixture's queries, not `state.camino`. `map` returns once the window is managed, its frame is drawn and its client is in sync, so no test dispatches after it to let things settle.
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
