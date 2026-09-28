# Compositor skeleton

Type: implementation
Status: open
Milestone: M1
Blocked by: 18
Map: [Camino v0.1](../map.md)

## What to build

`camino`'s core, following niri's shape as 15 describes it, with no protocols beyond what's needed to start a Wayland socket and render an empty output.

- **`state` (15):**
  - `State { backend: Backend, camino: Camino }` in one `EventLoop<State>`;
  - `Camino` holds the display handle, the loop handle, the seat, the outputs and `policy: Policy<Mapped>`;
  - Smithay's dispatch is delegated to `State`.
- **The refresh pass:** after each loop dispatch, it refreshes the outward views and flushes clients. There are no views yet; the hook is there so later tickets only add to it.
- **`backend` (15):** `Backend { Winit, Headless }`, with `Tty` added in M4.
  - `Winit`: one window as one output, with the GLES renderer.
  - `Headless` (`cfg(test)`): virtual outputs of any size and fractional scale, rendered with pixman. Smithay's `renderer_pixman` is enabled only through dev-dependencies.
- **`scene` (15):** per-output render-element assembly in the final layer order. Only the window layer has contents in M1, and the other layers are empty stubs in the right places.
- **`window`:** `Mapped` wrapping Smithay's `desktop::Window`, with `impl PolicyWindow for Mapped`.
- **`logging` and `main.rs` (14):** `tracing` with `EnvFilter` from `RUST_LOG`, to stderr for now. `--session` is parsed but refused until M5.

## Acceptance criteria

- [x] `cargo run -p camino` opens a winit window, creates a Wayland socket and logs its name, and clears the output every frame.
- [x] A `cfg(test)` test builds a `State` on the headless backend with two outputs at scales 1.0 and 1.5, and dispatches the loop.
- [x] The release build has neither the headless backend nor pixman (`cargo tree -e features -p camino --edges normal`).
- [ ] CI stays green, and the container gains the libraries the winit and pixman builds need.
