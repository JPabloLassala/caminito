# Palette helper

Type: implementation
Status: open
Milestone: M2
Blocked by: 24, 25
Map: [Camino v0.1](../map.md)

## What to build

The GTK colours for frames ([07](07-decoration-strategy.md), [ADR 0003](../../../docs/adr/0003-rust-theme-engine-and-gtk-palette-helper.md)), and the `helpers` supervisor that keeps the helper running.

- **`helpers/camino-gtk-palette`:** a C GTK3 program, its own Meson project, included by the top-level `meson.build` through `subproject()` and installed to `/usr/bin` (16).
  - It connects as a Wayland client of Camino and creates no windows.
  - For the default and the dark variant of the GTK theme, it reads `color` and `background-color` of `window.background` in every state Marco maps, and looks up the named colours with `gtk_style_context_lookup_color`.
  - It watches `org.mate.interface gtk-theme` and emits a new palette when it changes.
- **The named colours:** GTK3 has no API to list a theme's `@define-color` table, so the helper can only look up names it's given. Camino writes the `gtk:custom` names the current Marco theme uses to the helper's stdin, and writes them again when the Marco theme changes. This is a detail 07 left open.
- **Formats in `camino-theme` (15):** the helper's stdout format (line-based text with a version header), and the on-disk cache, one file per GTK theme and variant under `$XDG_CACHE_HOME/camino/palette/`. With no cache, a neutral built-in palette is used.
- **The `helpers` supervisor in `camino` (15):** spawn, the helper's stdout as a calloop fd source, stderr to the journal under the helper's own identifier (stderr with a prefix outside a session), exits through a `SIGCHLD` source, and restart with backoff. After five exits in a minute it gives up with one log line, and the last palette stays in use. The locker reuses it in M7.
- **Exact palette test (14):** the helper's output for the fixed GTK theme equals the colours [25](25-golden-harness.md) recorded from Marco's `GtkStyleContext`. The colours don't depend on the GDK backend, so CI runs the helper under `xvfb-run` with `GDK_BACKEND=x11`.

## Acceptance criteria

- [ ] The exact palette test passes in CI.
- [ ] Unit tests for the stdout format and the cache format, written before the code, including a truncated line and an unknown version.
- [ ] Headless tests of the supervisor with a fake helper script: a palette arrives, a crash restarts it, the give-up state is reached, and the last palette stays in use throughout.
- [ ] Changing `gtk-theme` with `gsettings` makes the running helper emit the new theme's palette.
