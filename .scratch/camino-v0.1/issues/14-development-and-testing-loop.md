# Development and testing loop

Type: grilling
Status: resolved
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

How is Camino developed and tested?

- **Running it:** nested under the winit backend vs. on a TTY. 08 made bare `camino`, without `--session`, the entry point with no session. How does a developer run a full nested MATE session?
- **Tests:**
  - headless integration tests (a headless backend plus real Wayland and X11 test clients);
  - golden tests for the theme engine against Marco (07);
  - unit-test seams for window-management policy (focus, placement, tiling, workspaces) kept separate from Smithay types.
- **CI and tooling:** CI (which forge, and which MATE and Smithay versions), and logging and debug tooling (`RUST_LOG` or tracing, a debug overlay).
- **Keeping up:** how to track Smithay master bumps (03 expects breaking changes on every bump).

## Answer

Decided in a grilling session on 2026-09-28. The user took the recommended answer on every question, chose winit-only nested runs (they have two monitors), and chose the union of 07's and 14's theme lists for the goldens. Inputs were what [07](07-decoration-strategy.md), [08](08-session-launch-model.md), [11](11-xwayland-integration.md), [12](12-screen-locking.md) and [13](13-hidpi-and-accessibility-scope.md) handed on, and a source read of Smithay `928d4a9`, niri `1f03391`, cosmic-comp `9f746ea`, marco `41c50d5` (1.29.1), mate-session-manager `57a9178`, mate-panel `58ac6c1` (and tag v1.28.7 `dcf3da2`), and mate-settings-daemon tags v1.28.0 `e35462e` and v1.29.1 `0787cee`. CI timings come from the projects' public GitHub Actions runs. The relevant facts are quoted inline.

How the Marco reference images are made is recorded in [ADR 0009](../../../docs/adr/0009-committed-marco-goldens-from-libmarco-private.md).

### Summary

| Item | Decision |
|---|---|
| Forge and CI | GitHub (private repo `JPabloLassala/camino`) with Actions; commits go straight to `master` |
| Test bar | layered: policy unit tests, a headless integration harness, an X11 tier, theme goldens, and a manual checklist per milestone |
| Policy seam | policy generic over a `PolicyWindow` trait; Smithay geometry types allowed, protocol objects not |
| Policy tests | example tests plus proptest operation sequences checked against invariants |
| Integration tests | Camino's own headless backend, pixman renderer, virtual outputs at any scale, a `wayland-client` test client, insta snapshots |
| X11 tests | a real Xwayland driven by an `x11rb` test client |
| wlcs | not in v0.1 |
| Theme goldens | rendered by `marco-frame-dump` (C, against libmarco-private) under Xvfb, committed; geometry exact, pixels within a tolerance with the font pinned; palette compared separately |
| Golden themes | Menta, ClearlooksRe, Atlanta, eOS, HighContrast, BlackMATE, TraditionalOk, plus Arc, Mint-Y and Yaru vendored |
| Manual checks | `docs/testing/checklist.md`, items tagged by milestone, run before a milestone closes |
| Nested runs | winit only (one output), under `dbus-run-session` with a scratch `XDG_CONFIG_HOME` |
| Real session runs | on a spare tty as a second user, `caminotest` |
| systemd import | skipped and logged when `org.freedesktop.systemd1` isn't on Camino's bus |
| Patched MATE on the dev machine | installed system-wide from 16's packages |
| CI image | `archlinux:latest`, no MATE session components |
| gsettings and D-Bus in tests | vendored schemas with `GSETTINGS_BACKEND=memory`; a private bus per test binary |
| Toolchain | latest stable Rust, no MSRV; clippy `-D warnings`; rustfmt defaults; cargo-deny with Apache-2.0 allowed |
| Smithay | explicit `rev`, one bump per commit, weekly canary against master, `// SMITHAY(#issue)` markers |
| Logging | `tracing` + `EnvFilter` (`RUST_LOG`); journald in a session, stderr otherwise |
| Debug tooling | Tracy behind a feature; damage, opaque-region, tint and frame-debug toggles; `org.mate.Camino.Debug` state dump |

### Running Camino

**The dev machine shares one systemd user manager and one session bus between logins.** `loginctl` on the developer's machine shows several user sessions (tty1–tty3) but one `manager` session, so every login of the same user talks to `/run/user/1000/bus` and one `systemd --user`. The host desktop is Plasma. mate-session exports its whole environment through `org.freedesktop.systemd1.Manager.SetEnvironment` and `UpdateActivationEnvironment` at start-up (`main.c:651-654`, `gsm-util.c:619-650`), and again on every `gsm_util_setenv` (`gsm-util.c:720-737`). Camino's own export step (08) does the same. A same-user `camino --session` would therefore repoint the host desktop's portals and D-Bus-activated services at Camino's `DISPLAY`, `WAYLAND_DISPLAY` and `XDG_CURRENT_DESKTOP` for as long as it runs. Camino unsets its own variables on exit (08), but mate-session's exports, `XDG_CURRENT_DESKTOP` among them, stay behind. mate-session also needs `org.gnome.SessionManager` as primary owner or it exits (`main.c:129`, `:145-156`).

**Nested runs: winit, on a private bus.**

- A `tools/dev-nested` script runs `dbus-run-session` around Camino (bare, or `--session` for a full nested MATE session), with a scratch `XDG_CONFIG_HOME` so dconf is separate too.
- `dbus-run-session`'s bus only has the standard service directories (`/usr/share/dbus-1/session.conf:27`), so `org.freedesktop.systemd1` isn't on it. mate-session's `SetEnvironment` calls fail with a debug log (`gsm-util.c:737`). This is GNOME's own approach (`dbus-run-session gnome-shell --devkit` from 49).
- **Camino checks whether `org.freedesktop.systemd1` is on its bus** before its systemd import, and when it isn't, skips the import and logs it once. It never shells out to `systemctl --user import-environment`.
- **One output only.** Smithay's winit backend gives one window per init (`backend/winit/mod.rs:68-140`), and niri and cosmic-comp are the same. Smithay's x11 backend could give several windows, but the developer has two monitors, so visual mixed-scale checks happen on the tty instead. Mixed scales are covered in automated tests by the headless backend (below).
- `locker-command` set to `swaylock` for nested runs (12) is allowed but not needed, since `camino-locker` runs nested like any gtk-session-lock client.

**Real sessions: a spare tty, as a second user.**

- The developer creates a `caminotest` user and logs in on a spare tty, where `camino --session` runs exactly as the display manager would run it. It gets its own systemd manager, bus, dconf and runtime directory, so this is the only run that exercises the real systemd export path, and it can't disturb the host session.
- The patched MATE components are **installed system-wide** from the packages 16 produces. Every carried patch falls back to normal behaviour without Camino (see CONTEXT.md), so this is safe, and it tests 16's packaging continuously. No distribution ships what the session needs: mate-panel's libxfce4windowing support is only on master (`meson.build:208-211`, not in v1.28.7), and msd's wlrandr plugin needs 1.29.1 (`plugins/Makefile.am:38-40`), which Arch, Fedora and Debian don't package yet.

### Test layers

**Policy unit tests (the largest layer).**

- Window-management policy (focus, stacking, placement, workspaces, edge snapping, Marco's transient and window-type rules) is generic over a `PolicyWindow` trait. Camino implements it for xdg and X11 windows, and tests implement it with a `TestWindow`. This is niri's shape: `Layout<TestWindow>` with `TestWindow` implementing `LayoutElement` (`layout/tests.rs:26-47`, `layout/mod.rs:131`).
- Policy may use Smithay's plain geometry types (`Rectangle`, `Size`, `Point`, `Logical`/`Physical`) but no protocol object, surface or `Output` handle. That is what "no Smithay dependency" means for 15's policy crate.
- Tests are example tests plus **proptest operation sequences** (add, remove and move windows and outputs, focus, workspace switches, transients) with invariants checked after every operation. Examples:
  - at most one focused window per seat;
  - transients stay above their parents;
  - every window is on exactly one workspace, or sticky;
  - the stacking order is a permutation of the managed windows.

  The pattern is niri's `check_ops` and `verify_invariants` (`layout/tests.rs:1636-1648`). A few hundred cases run by default, and a large count runs in the weekly job, as niri does with `PROPTEST_CASES=200000` (`ci.yml:108-141`).
- The keyboard filter (AccessX sticky, slow and bounce keys from 13), 13's default-scale rule, and 09's settings translation are unit-tested in the same way.

**Headless integration tests.**

- Smithay has no headless backend (`src/backend/` has none). Camino gets its own, behind a test feature, modelled on niri's (`backend/headless.rs`):
  - virtual outputs of any size **and any fractional scale**, so 13's mixed-scale cases are testable even though nested runs have one output;
  - rendering through Smithay's **pixman** renderer, so CI needs no GPU or EGL.
- A `Fixture` runs Camino and test clients in one calloop, joined by `UnixStream::pair()` and `insert_client`, with `roundtrip` helpers (niri's `tests/fixture.rs:15-153`).
- The test client is a small Rust `wayland-client` implementation of the protocols Camino's logic depends on: xdg-shell, xdg-decoration and KDE server-decoration, layer-shell, fractional-scale and viewporter, ext-session-lock, and foreign-toplevel and ext-workspace as a panel would use them.
- Assertions are **insta text snapshots** of configure sequences, states, sizes, positions, enter and leave events, and preferred scales, as niri does (5,280 `.snap` files). Pixels aren't compared at this layer.
- D-Bus services (the screensaver shim, `org.freedesktop.a11y.Manager`, `org.mate.Camino`, `org.mate.Camino.Debug`) are tested here with a D-Bus test client.

**X11 tier.**

- Nobody else tests XWayland: niri uses xwayland-satellite and has no X in its tests, and cosmic-comp has almost no tests. This is where Camino's own X code (its X connection, 11's restart path) is densest.
- These tests spawn a real Xwayland from the headless fixture and drive it with an `x11rb` test client. They check `_NET_WM_DESKTOP`, `_NET_WORKAREA`, `_NET_FRAME_EXTENTS`, XSETTINGS and the X11 scale values, window types and focus, and override-redirect stacking.

**wlcs: not in v0.1.** Smithay runs it only through `wlcs_anvil` with an allowlist of four filters (`ci.yml:303-316`). A conformance suite for protocol basics Smithay already implements is worth little next to Camino's own tests.

**Theme goldens.** See [ADR 0009](../../../docs/adr/0009-committed-marco-goldens-from-libmarco-private.md).

- **The reference harness, `marco-frame-dump`,** is a small C program in the Camino repo. It links the installed `libmarco-private` and calls `meta_theme_load`, `meta_theme_calc_geometry` and `meta_theme_draw_frame`. The draw call takes type, flags, size, title layout, button layout and per-button states as arguments and draws onto any `cairo_t` (`theme.c:5870-5913`), so it can write PNGs through a cairo image surface. It needs an open GDK display (`gdk_window_get_scale_factor(gdk_get_default_root_window())`, `theme.c:4173`), so it runs under `xvfb-run`. `marco-theme-viewer` can't be used, since it is interactive with hardcoded flags and button states (`theme-viewer.c:461-510`, `preview-widget.c:195-201`). Marco itself has no rendering tests.
- **Goldens are committed**, as PNGs plus a JSON file of geometry (borders, button rects, title rect) per case, with the Marco version, commit and GTK theme recorded beside them. A script regenerates them on the developer's machine when Marco is bumped or a case is added. CI only compares Camino with the committed files, and needs no Marco.
- **Determinism:** both sides get the same pinned inputs:
  - a vendored font behind a private `FONTCONFIG_FILE`, passed as an explicit font description rather than read from gsettings;
  - grayscale antialiasing, `hintstyle` slight, 96 DPI;
  - ASCII titles;
  - a fixed GTK theme for colours and a fixed icon theme (or fixed pixbufs) for icons.
- **Comparison:** geometry must match exactly. Pixels, including the title text, must match within a small per-channel tolerance and a small count of differing pixels, tuned once the first themes pass. The colours the harness's `GtkStyleContext` produces for the fixed GTK theme, and the colours `camino-gtk-palette` reads for it, are compared exactly in a separate test, so a colour bug fails as a colour bug.
- **Themes:**
  - from Marco and mate-themes: Menta (the default, v3 with SVG buttons), ClearlooksRe, Atlanta (v1 basics), eOS (v3), HighContrast, BlackMATE and TraditionalOk;
  - third-party, vendored under `tests/themes/` at a recorded upstream commit: Arc, Mint-Y and Yaru, which cover the format tiers 04 found in the wild but stock themes don't use.
- **Cases:**
  - frame types normal, dialog, modal dialog, utility and border;
  - focused and unfocused, maximized, shaded, tiled left and right;
  - each button in normal, prelight and pressed;
  - scales 1 and 2.

**gsettings and D-Bus in automated tests.**

- The schemas Camino reads (`org.mate.Marco.*`, `org.mate.interface`, `org.mate.accessibility-keyboard`, `org.mate.font-rendering` and the rest of 09's list) are vendored under `tests/schemas/`, with the upstream commit each came from recorded. They are compiled at test time and used with `GSETTINGS_BACKEND=memory`. An upstream schema change becomes a deliberate re-vendor.
- Each test binary that uses D-Bus runs under its own `dbus-run-session`.

### Checks handed on from earlier tickets

| Check (from) | Where |
|---|---|
| Xwayland's `-terminate` doesn't fire while Camino's connections are attached (11) | X11 tier: wait past the idle timeout with no clients, check Xwayland is still alive |
| Xwayland restart on the same display, under bare `camino` (11) | X11 tier: kill Xwayland, check it comes back on the same display with the root properties published |
| X11 clipboard contents survive the app quitting, through msd's data-control manager (11) | checklist (needs msd) |
| Killing the locker while locked leaves the session locked, and the locker comes back (12) | headless: a test locker client on ext-session-lock, killed by the test |
| The locker give-up path (12) | headless for the state and message; checklist for how it looks |
| The suspend race with a slow locker (12) | checklist (real logind) |
| `loginctl unlock-session` recovery (12) | checklist |
| Orca binds the keyboard monitor, and reads `camino-locker` (13) | checklist |
| KeyboardMonitor semantics: allowlist, grabs, the lone-modifier rule (13) | headless, with a D-Bus test client |
| AccessX sticky, slow and bounce keys (13) | unit tests on the keyboard filter |
| A GTK3 window dragged across a 1.0 and a 2.0 output (13) | headless snapshots of enter, leave and preferred scales; checklist on the tty with two monitors for the visual part |
| The default scale rule against real EDIDs (13) | unit tests on a table of EDID sizes: KWin's cases ported, plus the developer's own monitors |

### Manual checklist

- One file, `docs/testing/checklist.md`. Each item has steps, the expected result, the debug tool that shows it (below), and the milestone that introduces it.
- Before a milestone closes (17), the developer runs every item tagged with that milestone or an earlier one that still applies, and records the result in the milestone's closing commit message.
- The session-level items above start the list. Anything that needs real logind, real hardware, msd or Orca belongs here, not in CI.

### CI

- **GitHub Actions** in an `archlinux:latest` container with GTK3, cairo, pango, pixman, Xwayland, Xvfb and a few test clients, and **no MATE session components**. Arch is the closest distribution: it has marco 1.29.0 and libxfce4windowing 4.20.7, while Fedora rawhide and Debian sid are behind (Debian at 1.26/1.27).
- **On push and pull request:** rustfmt, clippy with `-D warnings`, cargo-deny, build, the unit tests, the headless and X11 tiers, and the golden comparisons.
- **Weekly cron:**
  - the proptest run with a large case count;
  - a build and test against Smithay's current master, which opens or updates an issue when it fails;
  - each carried MATE patch applied to and built against its upstream component's git head, so a patch that breaks in a rebase is caught early. How the patches are stored and built is 16's. ([16](16-packaging.md): `format-patch` series beside each PKGBUILD in `packaging/arch/`.)
- **Workflow:** the developer commits straight to `master`. There's no branch protection and no required pull requests.
- **Toolchain:** latest stable Rust and no MSRV, since Camino isn't a library and Arch ships current Rust. `rust-toolchain.toml` pins `channel = "stable"` with rustfmt and clippy. rustfmt uses its defaults.
- **cargo-deny** checks advisories and licences. Camino is GPL-2.0-or-later. Apache-2.0 crates are allowed (the combined work is then distributable under GPLv3, and Smithay's dependency tree is full of them). GPL-3-only crates and unknown licences are rejected, which keeps 03's rule that niri's and cosmic-comp's code is a design reference only.

### Tracking Smithay master

- **Pin an explicit `rev`** of `smithay` in `[workspace.dependencies]`, as niri does (`Cargo.toml:28-38`).
- **Bump on purpose, one bump per commit.** The bump commit carries every adaptation and passes the full suite. For scale: over the last year niri bumped about 20 times and cosmic-comp about 25. About half were lockfile-only, and a few rewrote 10–46 files (Smithay's Dispatch2 refactor was one). No upstream CI tests downstream compositors.
- **The weekly canary** (above) builds against Smithay master and opens or updates an issue when it fails, so breakage shows up as it lands.
- **Workarounds for Smithay bugs** carry a `// SMITHAY(#issue)` comment, so a bump can find them with grep and remove them.
- Dependabot doesn't open Smithay bumps. niri configures it but still bumps by hand.

### Logging and debug tooling

- **Logging:** `tracing` with `tracing-subscriber`'s `EnvFilter`, read from `RUST_LOG`, as niri, cosmic-comp and anvil all do.
  - In a session (`--session`, and stderr not a tty), logs go to **journald** through `tracing-journald`, as cosmic-comp does (`logger/mod.rs:8-43`).
  - Bare `camino`, or a tty stderr, logs to stderr.
  - Camino connects the stderr of its helpers (Xwayland, `camino-gtk-palette`, the locker) to the journal under their own identifiers.
  - `journalctl --user -t camino` is where 09's "unsupported key" messages are found.
- **Profiling:** a `profile-with-tracy` feature through the `profiling` crate (anvil's approach, `anvil/src/main.rs:27-35`), with spans at the render, input and scene entry points.
- **Debug toggles** as `org.mate.Camino` keybindings and debug D-Bus methods:
  - damage;
  - opaque regions;
  - a tint on client surfaces (niri's three, `niri-ipc/src/lib.rs:805-809`);
  - **frame debug:** outlines each frame's strips, invisible resize border and title rect, and labels the scale the strips were rendered at.
- **State inspection:** an `org.mate.Camino.Debug` D-Bus interface dumps windows, workspaces, stacking, outputs and scales, focus, the lock state and the X11 scale as JSON, for `busctl` and the checklist. There's no niri-style CLI.
- No egui overlay in v0.1.

### Handed on

- **[07](07-decoration-strategy.md):** the golden set is 07's surveyed themes plus ClearlooksRe, eOS and HighContrast, with the third-party ones vendored ([ADR 0009](../../../docs/adr/0009-committed-marco-goldens-from-libmarco-private.md)).
- **[08](08-session-launch-model.md):** Camino's systemd import is skipped and logged when `org.freedesktop.systemd1` isn't on its bus.
- **15 Crate and module architecture** ([15](15-crate-and-module-architecture.md): `camino-policy` and `camino-test-client` crates, a `cfg(test)` headless backend, in-crate integration tests, `tools/marco-frame-dump`, `dbus::debug` and `debug`, `tracing` in `logging`, helper stderr through `helpers`):
  - the `PolicyWindow` trait as the policy seam (Smithay geometry types allowed, protocol objects not);
  - the headless backend and its test feature;
  - the test client crate;
  - `marco-frame-dump` as a C subproject beside `camino-gtk-palette` and `camino-locker`;
  - where `org.mate.Camino.Debug` and the debug toggles live;
  - where `tracing` is initialised, and the helpers' stderr going to the journal.
- **16 Packaging** ([16](16-packaging.md): the packages replace stock MATE and every patch is gated on Camino, [ADR 0011](../../../docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-camino.md); the patches are `format-patch` files in `packaging/arch/<pkgname>/`; CI runs `makepkg` and `namcap` when they change):
  - the patched MATE packages must install system-wide on the developer's machine without breaking MATE on X11 (the fall-back rule);
  - the carried patches need a form that the weekly CI job can apply to upstream git heads;
  - Camino's package ships no test or debug binaries.
- **17 Milestone slicing:** each milestone's exit criterion includes its checklist items, recorded in the closing commit message. The first milestone includes the CI workflow, the headless fixture and the policy test scaffolding. ([17](17-milestone-slicing.md): exit criteria are runnable, tested and checked, with checklist tags `M1`–`M8`; all three are in M1.)
