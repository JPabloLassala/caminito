# Camino

Camino is a Wayland compositor for the MATE desktop, built to be the Wayland counterpart of
[Marco](https://github.com/mate-desktop/marco). It aims to run a full MATE session with Marco's
look and behaviour: frames drawn from Marco themes, Marco's focus, placement and workspace rules,
and configuration read from the existing `org.mate.Marco.*` gsettings.

It is written in Rust on [Smithay](https://github.com/Smithay/smithay).

> **Status:** early development. Camino v0.1 is being built in eight milestones, and the first
> (M1, a nested compositor) is in progress. Today Camino runs only as a window inside another
> desktop, with no frames, XWayland or MATE session yet. It isn't ready for daily use.

## Goals for v0.1

- Full Marco parity: every `org.mate.Marco.*` key and keybinding action honoured, except the
  X11-only ones.
- Server-side frames drawn from Marco/Metacity themes by a Rust port of Marco's theme engine,
  tested against reference renderings from Marco itself.
- XWayland, with X11 apps managed and framed like native ones.
- A "MATE on Camino" session entry that runs mate-session, mate-panel (window list and workspace
  switcher), caja's desktop and mate-settings-daemon.
- Fractional output scaling, screen locking, idle and blanking, Orca and AccessX.

Out of scope: tiling beyond Marco's edge snapping, showy effects, non-Linux platforms, and
replacing the Wayfire-based MATE Wayland session.

## Milestones

| # | Milestone | Ends with |
|---|---|---|
| M1 | Nested compositor | GTK and Qt apps mapped, focused, stacked, moved and resized on several workspaces, nested under winit |
| M2 | Marco frames | SSD windows framed from the Marco theme, matching the goldens |
| M3 | XWayland | X11 apps managed and framed like native ones |
| M4 | On the metal | DRM/KMS, libinput, several outputs at fractional scales |
| M5 | MATE session | "MATE on Camino" logs in and out with mate-panel, caja's desktop and msd |
| M6 | MATE integrations | Alt+F1/F2, pager layout, Show Desktop, mate-control-center dialogs, window screenshots |
| M7 | Lock, idle and accessibility | the lock fails closed; idle, blanking, DPMS, Orca and AccessX work |
| M8 | Marco parity | every Marco key and action accounted for; `v0.1.0` is tagged |

## Building

Camino targets Arch Linux and builds with stable Rust (see [rust-toolchain.toml](rust-toolchain.toml)).

```sh
sudo pacman -S --needed rustup gcc libxkbcommon pixman dbus
cargo build --workspace
```

## Running

Camino currently runs only nested, as a winit window inside an existing Wayland or X11 desktop.
Run it on a private session bus so it doesn't touch your desktop's:

```sh
dbus-run-session -- cargo run -p camino
```

It logs the socket it listens on (`WAYLAND_DISPLAY=wayland-N`). Point clients at it:

```sh
WAYLAND_DISPLAY=wayland-1 gtk4-demo
```

Logs go to stderr at `info`; set `RUST_LOG` (for example `RUST_LOG=camino=debug`) for more.

The keybindings are a hard-coded set of Marco's defaults until M8 reads them from gsettings:

| Keys | Action |
|---|---|
| Ctrl+Alt+Left / Right | Switch workspace |
| Alt+Tab | Switch windows |
| Alt+F4 | Close window |
| Alt+F10 | Toggle maximised |

`camino --session`, which will start a full MATE session, isn't supported yet (M5).

## Testing

```sh
cargo test --workspace --exclude camino
dbus-run-session -- cargo test -p camino
```

The tests are layered:

- **Policy:** window-management decisions are tested through a fake window type, with proptest
  invariants.
- **Headless:** `camino`'s integration tests run the compositor on a pixman headless backend and
  drive it with a real `wayland-client` test client, asserting with [insta](https://insta.rs)
  snapshots.

CI runs rustfmt, clippy (`-D warnings`), cargo-deny and the tests in an Arch container on every
push. A weekly job builds against Smithay master, since Smithay is pinned by commit and bumped one
commit at a time.

## Layout

| Crate | What it is |
|---|---|
| [camino](camino/) | The compositor: backends, Wayland protocol handlers, input and rendering |
| [camino-policy](camino-policy/) | Window-management policy (focus, stacking, placement, workspaces, transients), free of protocol types |
| [camino-theme](camino-theme/) | Marco's theme engine, with no Smithay or GTK (from M2) |
| [camino-test-client](camino-test-client/) | The Wayland client the integration tests drive |

## Design documents

- [CONTEXT.md](CONTEXT.md): the project's vocabulary. Camino is a *compositor*, never a "window
  manager".
- [docs/adr/](docs/adr/): architecture decision records, from the choice of Rust and Smithay to how
  MATE packages are patched.

## License

GPL-2.0-or-later. See [COPYING](COPYING).
