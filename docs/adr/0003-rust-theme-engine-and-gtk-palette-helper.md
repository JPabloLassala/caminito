# 3. Port Marco's theme engine to Rust, and take GTK colours from a helper process

Date: 2026-09-28
Status: Accepted

## Context

Caminito draws server-side frames from Marco/Metacity themes. Every existing renderer descends from Metacity's C engine (about 12k lines in Marco's `theme.c` and `theme-parser.c`). The one Wayland port, metacity-decor, vendors that C and replaces every GTK colour with one configured colour, so themes that rely on fg/bg contrast render wrongly.

Themes take their colours from the GTK3 theme: `window.background` per state, plus `@define-color` names. GTK can't create a style context without a display connection. The theme CSS also isn't simple to read: Menta's colours are mostly `shade()`/`mix()` expressions with GTK-specific HSL semantics, and Adwaita, Arc and Yaru ship their CSS inside gresources.

## Decision

- Caminito ports the engine to Rust as its own crate, drawing through cairo and pango so the output matches Marco pixel for pixel. It's tested with golden images rendered by Marco's C.
- The GTK palette comes from `caminito-gtk-palette`, a small C GTK3 program that Caminito starts as a Wayland client of itself. It reports the default and dark palettes and re-reports when the GTK theme changes. Caminito caches the last palette on disk.

## Consequences

- The engine is testable Rust with no GTK in the compositor process. The price is porting about 12k lines and following Marco's behaviour by hand instead of by rebasing.
- Colours are exactly what GTK computes, including gresource themes and the full CSS cascade. The price is a supervised child process, a startup window covered by the cache, and a runtime dependency on GTK3.
- Rejected:
  - vendoring Marco's C over FFI (every draw call takes a `GtkStyleContext*`, so it would mean a permanent patched fork);
  - a pure-Rust drawing stack such as tiny-skia and cosmic-text (visible drift from Marco);
  - reading GTK CSS in Rust (it would approximate the cascade and `shade()`, and has to track GTK);
  - configured colours as metacity-decor does (lossy).
