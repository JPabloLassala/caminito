# Camino

Camino is a Wayland compositor for the MATE desktop, built to be the Wayland counterpart of
[Marco](https://github.com/mate-desktop/marco). It aims to run a full MATE session with Marco's
look and behaviour: frames drawn from Marco themes, Marco's focus, placement and workspace rules,
and configuration read from the existing `org.mate.Marco.*` gsettings.

It is written in Rust on [Smithay](https://github.com/Smithay/smithay).

> **Status:** early development. Camino v0.1 is being built in eight milestones. The first four
> (M1, a nested compositor, M2, Marco frames, M3, XWayland, and M4, on the metal) are done, and
> M5 (the MATE session) is next. Today Camino runs as a window inside another desktop or bare on
> a tty, drawing Marco frames around Wayland and X11 windows on the machine's monitors, with no
> MATE session yet. It isn't ready for daily use.

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

To install it as a package, build `camino-git`'s PKGBUILD. It packages the checkout's committed
`HEAD`, not uncommitted work:

```sh
cd packaging/arch/camino-git
makepkg -si
```

## Running

Camino currently runs only nested, as a winit window inside an existing Wayland or X11 desktop.
`tools/dev-nested` runs the installed `camino` on a private session bus with a scratch
`XDG_CONFIG_HOME`, so it doesn't touch your desktop's bus or dconf settings. `--bin` builds and
runs this checkout's debug build instead, and any other arguments go to `camino`:

```sh
tools/dev-nested        # the installed camino
tools/dev-nested --bin  # target/debug/camino
```

Once Xwayland is ready it prints the two displays, `dev-nested: WAYLAND_DISPLAY=wayland-N
DISPLAY=:M`. Point clients at them:

```sh
WAYLAND_DISPLAY=wayland-1 gtk4-demo
DISPLAY=:1 xterm
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

## Known limitations

- **An Xwayland crash logs you out of a MATE on Camino session.** Camino survives it, drops the
  XWayland windows and starts Xwayland again on the same `DISPLAY`, backing off and giving up after
  repeated crashes. But mate-session is an X11 client, and it exits when it loses its display,
  which ends the session. Outside a session, new X clients just work again.
- **One keyboard layout for every window.** Camino switches layouts globally, so
  `org.mate.peripherals-keyboard-xkb.general`'s `group-per-window` (on by default) and
  `default-group` have no effect. The layout indicator applet runs on Xwayland, so it may not
  show the layout Camino has switched to.

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
push, and builds the PKGBUILD with `makepkg` and `namcap` when `packaging/arch/` changes. A weekly job builds against Smithay master, since Smithay is pinned by commit and bumped one
commit at a time.

Some checks need a real desktop and are run by hand before a milestone closes. They're listed in
[docs/testing/checklist.md](docs/testing/checklist.md).

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
