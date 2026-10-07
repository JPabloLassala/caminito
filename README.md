<p align="center">
  <img src="data/icons/camino.svg" width="106" height="128" alt="The Camino logo">
</p>

# Camino

Camino is a Wayland compositor for the MATE desktop, built to be the Wayland counterpart of
[Marco](https://github.com/mate-desktop/marco). It aims to run a full MATE session with Marco's
look and behaviour: frames drawn from Marco themes, Marco's focus, placement and workspace rules,
and configuration read from the existing `org.mate.Marco.*` gsettings.

It is written in Rust on [Smithay](https://github.com/Smithay/smithay).

> **Status:** early development. Camino v0.1 is being built in nine milestones. The first five
> (M1, a nested compositor, M2, Marco frames, M3, XWayland, M4, on the metal, and M5, the MATE
> session) are done, and M6 (the MATE integrations) is next. Today Camino runs as a window inside
> another desktop or bare on a tty, drawing Marco frames around Wayland and X11 windows on the
> machine's monitors, and _MATE on Camino_ logs in and out with mate-panel, caja's desktop and
> mate-settings-daemon. The session can't lock yet, so it isn't ready for daily use.

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

| #   | Milestone                    | Ends with                                                                                             |
| --- | ---------------------------- | ----------------------------------------------------------------------------------------------------- |
| M1  | Nested compositor            | GTK and Qt apps mapped, focused, stacked, moved and resized on several workspaces, nested under winit |
| M2  | Marco frames                 | SSD windows framed from the Marco theme, matching the goldens                                         |
| M3  | XWayland                     | X11 apps managed and framed like native ones                                                          |
| M4  | On the metal                 | DRM/KMS, libinput, several outputs at fractional scales                                               |
| M5  | MATE session                 | "MATE on Camino" logs in and out with mate-panel, caja's desktop and msd                              |
| M6  | MATE integrations            | Alt+F1/F2, pager layout, Show Desktop, mate-control-center dialogs, window screenshots                |
| M7  | Lock, idle and accessibility | the lock fails closed; idle, blanking, DPMS, Orca and AccessX work                                    |
| M8  | Marco parity                 | every Marco key and action accounted for                                                              |
| M9  | Flatpak                      | a Flatpak app runs in its sandbox, offered no privileged global; `v0.1.0` is tagged                   |

## Building

Camino targets Arch Linux and builds with stable Rust (see [rust-toolchain.toml](rust-toolchain.toml)).

```sh
sudo pacman -S --needed rustup gcc libxkbcommon pixman dbus
cargo build --workspace
```

To build and install everything from nothing, run `tools/bootstrap`. It installs the build
tools, builds the patched MATE packages and `camino-git` in order, and ends with
`camino-desktop`, a meta package that depends on `camino-git` and on the rest of Arch's `mate`
and `mate-extra` groups. Its arguments go to `makepkg`:

```sh
tools/bootstrap              # or: tools/bootstrap --noconfirm --nocheck
```

To install only the compositor as a package, build `camino-git`'s PKGBUILD. It packages the
checkout's committed `HEAD`, not uncommitted work:

```sh
cd packaging/arch/camino-git
makepkg -si
```

### Patched MATE packages

Some MATE components need carried patches to work in the session
([ADR 0011](docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-camino.md)). Each is a
`<pkgname>-camino` package under [packaging/arch/](packaging/arch/), made from Arch's own
PKGBUILD, that replaces the stock package. Its patches do nothing unless Camino is running, or
are correct on any compositor, or only let the baseline build against Arch's libraries, so X11
MATE stays as it was. Build them before `camino-git`, which depends on them:

```sh
for package in mate-session-manager-camino mate-panel-camino mate-settings-daemon-camino \
  mate-control-center-camino caja-camino mate-utils-camino camino-git; do
  (cd packaging/arch/$package && makepkg -si)
done
```

`mate-panel-camino` is built from mate-panel's master, where the applet library has a new
soname (`libmate-panel-applet-4.so.2`). Arch's applets in other packages, such as `mate-applets`,
are linked to the old one, so the package also builds the 1.28 library and installs it beside
master's: each of those applets is a program of its own, which talks to the panel over D-Bus.
The applet of `mate-notification-daemon` is loaded into the panel's process instead, where the two
libraries can't work together, and doesn't work until that package is built again.

`mate-settings-daemon-camino` is msd 1.29.1, which asks for mate-desktop 1.29.0. Arch has 1.28.2,
so one of the package's patches takes out the one upstream commit that needs the newer library (a
change to the X11 display plugin for virtual machines that resize the screen).

`caja-camino` is Arch's caja 1.28.0 with one patch, which puts the desktop on Camino's primary
output.

`mate-utils-camino` is mate-utils 1.29.1, the first whose mate-screenshot captures on Wayland,
with one patch: `mate-screenshot --window`, which `<Alt>Print` runs, captures the window Camino
names as the active one. Its dictionary applet is built into the panel's process, as a Wayland
panel needs it, so it's linked to `mate-panel-camino`'s applet library.

The patches are `git format-patch` files beside each PKGBUILD, and they are the only copy. Each
series applies to a baseline: an upstream tag, or a pinned commit where the component has no
tag. To move a package to a new baseline, rebase its series:

1. In a checkout of the component, at the old baseline, apply the series:

   ```sh
   git checkout -b camino <old baseline>
   git am /path/to/camino/packaging/arch/<pkgname>-camino/*.patch
   ```

2. Move it to the new baseline, and correct what breaks:

   ```sh
   git rebase --onto <new baseline> <old baseline> camino
   ```

3. Export the series again, then update the PKGBUILD's source, `pkgver` and checksums
   (`updpkgsums`). Commit the patches and the PKGBUILD together, one commit for each move:

   ```sh
   rm /path/to/camino/packaging/arch/<pkgname>-camino/*.patch
   git format-patch --zero-commit --no-signature -N \
     -o /path/to/camino/packaging/arch/<pkgname>-camino <new baseline>
   ```

A change to the patches alone, on the same baseline, increases `pkgrel` instead.

## Running

A bare `camino` chooses where it runs from the environment. With `WAYLAND_DISPLAY` or `DISPLAY`
set it runs nested, as a winit window inside that Wayland or X11 desktop. With neither, as on a
spare tty, it drives the machine's monitors itself.

To run it nested, use `tools/dev-nested`. It runs the installed `camino` on a private session bus
with a scratch `XDG_CONFIG_HOME`, so it doesn't touch your desktop's bus or dconf settings.
`--bin` builds and runs this checkout's debug build instead, and any other arguments go to
`camino`:

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

| Keys                  | Action           |
| --------------------- | ---------------- |
| Ctrl+Alt+Left / Right | Switch workspace |
| Alt+Tab               | Switch windows   |
| Alt+F4                | Close window     |
| Alt+F10               | Toggle maximised |

### The MATE session

`camino --session` runs mate-session as Camino's child, and Camino exits with it and with its
status. `camino --session=CMD` runs `CMD` instead, for example `--session=foot`. The `camino-git`
package installs the session entry, _MATE on Camino_
(`/usr/share/wayland-sessions/mate-camino.desktop`), which a display manager lists, and depends
on the patched MATE packages the session needs.

The session logs in from a display manager, as _MATE on Camino_. It can't lock the screen until
M7, so don't leave it unattended. Log out of your other sessions first: every login of one user
shares one systemd user manager and one session bus, and the session changes the environment of
both, so it would disturb a desktop you are logged in to. To try it beside your own desktop, use
a second user, which has its own:

```sh
sudo useradd -m caminotest
sudo passwd caminotest
```

Switch to a spare tty (Ctrl+Alt+F4, for example), log in as `caminotest`, and run:

```sh
camino --session
```

Logging out from the panel's menu ends the session and gives the tty back.

When Camino's stderr isn't a terminal, as from a display manager, Camino logs to the journal,
and so do its helpers and Xwayland, each under its own name:

```sh
journalctl --user -b -t camino               # Camino
journalctl --user -b -t Xwayland             # Xwayland
journalctl --user -b -t camino-gtk-palette   # a helper
```

mate-session and the programs it starts write to Camino's stderr itself, so they are where the
display manager sends that.

On a tty Camino's stderr is the terminal, so all of it goes there. To have Camino's logs in the
journal, and those of mate-session's programs in a file, send stderr to the file:
`camino --session 2>~/session.log`.

## Known limitations

- **An Xwayland crash logs you out of a MATE on Camino session.** Camino survives it, drops the
  XWayland windows and starts Xwayland again on the same `DISPLAY`, backing off and giving up after
  repeated crashes. But mate-session is an X11 client, and it exits when it loses its display,
  which ends the session. Outside a session, new X clients just work again.
- **On NVIDIA, turning one monitor off or on can blank another for a moment.** Seen on an RTX 3060
  with driver 615.71.09: a monitor on DisplayPort at 144 Hz goes black briefly when one on HDMI is
  unplugged, plugged in, or turned off or on. The other way round, nothing happens. Camino changes
  only the monitor that comes or goes, and wayfire does the same on that machine, so the cause is
  the driver's or the hardware's. No window is lost.
- **One keyboard layout for every window.** Camino switches layouts globally, so
  `org.mate.peripherals-keyboard-xkb.general`'s `group-per-window` (on by default) and
  `default-group` have no effect. The layout indicator applet runs on Xwayland, so it may not
  show the layout Camino has switched to.
- **Some programs keep the cursor theme you logged in with.** Camino's own cursors, the ones it
  draws for clients that ask for a cursor by name (GTK and foot among them), and
  XWayland's default cursor follow `org.mate.peripherals-mouse`'s `cursor-theme` and
  `cursor-size` at once. Programs that load cursors themselves from `XCURSOR_THEME` and
  `XCURSOR_SIZE` don't: Camino exports those once, at start-up, so such a program shows the theme
  of the login until you log in again, even if it's started after the change. That includes X11
  programs that leave the theme to libXcursor, which reads the environment before the
  `Xcursor.theme` resource Camino keeps up to date.

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
push, and builds each PKGBUILD with `makepkg` and `namcap` when `packaging/arch/` changes. A
weekly job builds against Smithay master, since Smithay is pinned by commit and bumped one
commit at a time. Another weekly job builds each patched package's series on its component's
upstream head. A failure of either opens an issue.

Some checks need a real desktop and are run by hand before a milestone closes. They're listed in
[docs/testing/checklist.md](docs/testing/checklist.md).

## Layout

| Crate                                     | What it is                                                                                                                           |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| [camino](camino/)                         | The compositor: backends, Wayland protocol handlers, input and rendering                                                             |
| [camino-policy](camino-policy/)           | Window-management policy (focus, stacking, placement, workspaces, transients), free of protocol types                                |
| [camino-layout](camino-layout/)           | Docking and healing a layout of outputs, with no dependencies, for Camino and camino-displays                                        |
| [camino-displays](camino-displays/)       | The display settings program, in GTK4: a library with the page's state and its client of output management, and the window over them |
| [camino-theme](camino-theme/)             | Marco's theme engine, with no Smithay or GTK (from M2)                                                                               |
| [camino-test-client](camino-test-client/) | The Wayland client the integration tests drive                                                                                       |

## Design documents

- [CONTEXT.md](CONTEXT.md): the project's vocabulary. Camino is a _compositor_, never a "window
  manager".
- [docs/adr/](docs/adr/): architecture decision records, from the choice of Rust and Smithay to how
  MATE packages are patched.

## License

GPL-2.0-or-later. See [COPYING](COPYING).
