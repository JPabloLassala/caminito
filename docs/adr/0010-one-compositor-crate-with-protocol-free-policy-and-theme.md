# 10. One compositor crate, with protocol-free policy and theme crates, on one thread

Date: 2026-09-28
Status: Accepted

## Context

Camino needs Marco's full window-management behaviour (05), tested as a property-tested policy with no protocol objects (14), plus a Rust port of Marco's theme engine tested against goldens (ADR 0003, ADR 0009). The reference compositors split their code differently.

- niri is one large compositor crate. Its layout is generic over a trait, but that trait takes `WlSurface`, `Output` and renderer types (`layout/mod.rs:131-340`). Wayland, input, layout and rendering run on one thread, and D-Bus runs on zbus's own thread.
- cosmic-comp is one crate too. Its policy sits on `Space` with themes and animations inside it, and has almost no tests. It shares the shell with one render thread per KMS surface through `Arc<RwLock<Shell>>`.
- All three reference compositors, anvil included, wrap Smithay's `desktop::Window` for both xdg and XWayland windows.

Camino also has its own X connection beside Smithay's `X11Wm`, and both must work on the same window state (11).

## Decision

- **Four Rust crates in a virtual workspace.** `camino` is the compositor, as a library plus `main.rs`, and holds every protocol, D-Bus service, the settings layer, XWayland, the scene and the backends as modules. `camino-policy` holds window-management policy and uses Smithay only for geometry types. `camino-theme` is the theme engine, with no Smithay or GTK. `camino-test-client` holds the test clients. A crate exists only where it enforces a dependency rule.
- **Policy is `Policy<W: PolicyWindow>`.** It owns windows, workspaces, stacking, focus and placement, and applies decisions through the trait. `camino`'s `Mapped`, a wrapper over Smithay's `desktop::Window`, implements it for xdg and XWayland windows alike.
- **One thread runs Wayland, X11, input, policy and rendering**, in one calloop `EventLoop<State>` with `State { backend, camino }` and a `Backend` enum. Only zbus's executor and a GLib thread for GSettings run elsewhere, and they talk to the loop over channels. The main thread never blocks on them.
- **No retained scene graph and no `desktop::Space`.** Render elements are assembled per output every frame from a fixed layer order, and every outward view of policy state (foreign-toplevel, ext-workspace, EWMH properties, D-Bus properties) is refreshed from it in one pass per loop iteration.

## Consequences

- The policy seam from 14 is enforced by the compiler: a protocol type can't be named in `camino-policy`. The price is a trait boundary, and `Mapped` translating between policy's requests and each protocol.
- `xwm` and Camino's X connection share window state without locks, and nothing needs `Arc<RwLock<_>>`. The price is that a slow frame, or slow policy code, delays input and D-Bus replies. Rendering can't be spread across GPUs or outputs without revisiting this.
- `camino` will be large, like niri's crate. Its protocol and D-Bus modules keep Smithay's `State` plus `Handler` shape, so any of them can move into its own crate later.
- Rejected:
  - niri's layout trait shape with protocol types in it (policy couldn't be tested without Smithay's Wayland objects);
  - cosmic-comp's render threads and shared shell;
  - anvil's generic `State<B: Backend>` (a type parameter on every handler for three backends);
  - Camino's own xdg/X11 window enum (it would duplicate `desktop::Window`);
  - separate protocol, D-Bus and config crates (public APIs with a single consumer);
  - `desktop::Space` (one global stacking order, with no workspaces, Marco layers, override-redirect windows or lock).
