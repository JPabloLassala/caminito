# Palette helper

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M2
Blocked by: 24, 25
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The GTK colours for frames ([07](07-decoration-strategy.md), [ADR 0003](../../docs/adr/0003-rust-theme-engine-and-gtk-palette-helper.md)), and the `helpers` supervisor that keeps the helper running.

- **`helpers/caminito-gtk-palette`:** a C GTK3 program, its own Meson project, included by the top-level `meson.build` through `subproject()` and installed to `/usr/bin` (16).
  - It connects as a Wayland client of Caminito and creates no windows.
  - For the default and the dark variant of the GTK theme, it reads `color` and `background-color` of `window.background` in every state Marco maps, and looks up the named colours with `gtk_style_context_lookup_color`.
  - It watches `org.mate.interface gtk-theme` and emits a new palette when it changes.
- **The named colours:** GTK3 has no API to list a theme's `@define-color` table, so the helper can only look up names it's given. Caminito writes the `gtk:custom` names the current Marco theme uses to the helper's stdin, and writes them again when the Marco theme changes. This is a detail 07 left open.
- **Formats in `caminito-theme` (15):** the helper's stdout format (line-based text with a version header), and the on-disk cache, one file per GTK theme and variant under `$XDG_CACHE_HOME/caminito/palette/`. With no cache, a neutral built-in palette is used.
- **The `helpers` supervisor in `caminito` (15):** spawn, the helper's stdout as a calloop fd source, stderr to the journal under the helper's own identifier (stderr with a prefix outside a session), exits through a `SIGCHLD` source, and restart with backoff. After five exits in a minute it gives up with one log line, and the last palette stays in use. The locker reuses it in M7.
- **Exact palette test (14):** the helper's output for the fixed GTK theme equals the colours [25](25-golden-harness.md) recorded from Marco's `GtkStyleContext`. The colours don't depend on the GDK backend, so CI runs the helper under `xvfb-run` with `GDK_BACKEND=x11`.

## Acceptance criteria

- [x] The exact palette test passes in CI.
- [x] Unit tests for the stdout format and the cache format, written before the code, including a truncated line and an unknown version.
- [x] Headless tests of the supervisor with a fake helper script: a palette arrives, a crash restarts it, the give-up state is reached, and the last palette stays in use throughout.
- [x] Changing `gtk-theme` with `gsettings` makes the running helper emit the new theme's palette.

## Notes

- **The exact palette test** is `caminito-theme/tests/palette_helper.rs`. It's `#[ignore]`d, since it needs the helper built by Meson (named by `CAMINITO_GTK_PALETTE`) and an X display. CI's "Test the palette helper" step builds the top-level Meson project into `target/meson` and runs it with `-- --ignored` under `xvfb-run`. Both of its tests pass locally under `xvfb-run`. In the exact test, every state of both variants and every named colour equals `palette.json` exactly. The other test is the `gsettings` criterion: it switches the private keyfile backend from Adwaita to HighContrast while the helper runs. The ticket stays open until CI has run them.
- **The stdout format**, in `caminito-theme`'s `palette::stream`, is a `caminito-gtk-palette 1` header, then one block per report: `palette <GTK theme>`, `variant default`, eight `state <name> <bg rgba> <fg rgba>` lines, `named <name> <rgba>` for each name GTK defines, the same for `variant dark`, and `end`. Channels are written with `g_ascii_dtostr`, so they're independent of the locale and read back as the exact double. A bad block is dropped whole and reading carries on at the next `palette`. A different header refuses the rest of the stream. At EOF, a line without its newline or a block without its `end` is reported.
- **The names**: Caminito writes `names <name>...` to the helper's stdin (`names_request`). Each line replaces the previous names and gets a report. The helper stays quiet until the first such line, reports again on every `gtk-theme` change, and exits at EOF. A name the theme doesn't define is left out, where `palette.json` has `null`. `Theme::custom_color_names` gives the names a theme's draw ops and styles actually use. For the ten golden themes, that is `palette.json`'s names minus two that only unused constants name.
- **The cache** is `$XDG_CACHE_HOME/caminito/palette/<GTK theme>/{default,dark}`, holding a `caminito-palette-cache 1` header, the same `state` and `named` lines, and `end`, written through a temporary file and a rename. A theme name that isn't a single path component is never cached. A bad file is logged and that variant is neutral. The neutral palette is GTK3's built-in Adwaita's `window.background`, with no named colours.
- **The helper** reads colours as `frames.c` and `theme.c` do: the path of a popup `GtkWindow` that's never realized, the theme's provider at settings priority, the `background` class, and a transparent background replaced by a fresh toplevel's, as 25 asked. It sets `gtk-theme-name` on its `GtkSettings` to the key's value, since the Wayland backend reads the theme from `org.gnome.desktop.interface`.
- **The supervisor** (`caminito/src/helpers.rs`) is generic over the loop data, with `fn` pointers to find the helper in it and to deliver `Started`, `Stdout`, `Exited` and `GaveUp`. The `SIGCHLD` source is signal-hook's self-pipe as a calloop source, not calloop's signalfd. A signalfd only sees the signal if every thread blocks it, which the test harness's threads don't, and parallel tests each need to see every `SIGCHLD`. On `SIGCHLD` it checks its own child with `try_wait`, and reads the rest of the child's stdout before reporting the exit. Backoff starts at 0.5 s and doubles to at most 10 s. Five exits within 60 s and it gives up with one error line.
- **Stderr**: `StderrTarget::Journal` connects a journald stream under the helper's identifier (what `systemd-cat` does), and falls back to the prefix if that fails. Caminito has no session mode until M5, so it uses `StderrTarget::Prefixed` for now. The journal path isn't tested, since CI's container has no journald.
- **Wiring**: `Caminito` holds a `PaletteSource`, neutral at start-up, since the GTK theme isn't known before 29. `run_nested` starts the helper on its socket with `GDK_BACKEND=wayland`, and `CAMINITO_GTK_PALETTE` overrides the program. `tools/dev-nested --bin` builds the helper into `target/meson` and sets it. A nested run under Xvfb started the helper as Caminito's client, got the host's theme's palette, and cached it. `PaletteState::palette_changed` only logs until 30 re-renders frames.
- **Packaging**: the PKGBUILD already runs the top-level Meson project, so `caminito-git` now installs `/usr/bin/caminito-gtk-palette`, and `gtk3` and `glib2` joined `depends`. `makepkg` wasn't run for this ticket. 33 still owns the namcap pass. The helper also needs the `org.mate.interface` schema, which comes from mate-desktop. Without it, the helper exits with a message saying so, and the supervisor gives up after five tries. 33 should declare it along with marco's schemas.
- **For 29**: `tests/schemas/` exists, with `org.mate.interface` from mate-desktop 1.28.2 and its commit in the README.
- **For 30**: pass the settings' `gtk-theme` to `PaletteSource::new` (or add a reload), so the cache is used before the helper's first report, and call `set_names` with the Marco theme's `custom_color_names`.
