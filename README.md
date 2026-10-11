<p align="center">
  <img src="data/icons/caminito.svg" width="128" height="128" alt="The Caminito logo">
</p>

# Caminito

Caminito is a Wayland compositor for the MATE desktop, built to be the Wayland counterpart of
[Marco](https://github.com/mate-desktop/marco). It aims to run a full MATE session with Marco's
look and behaviour: frames drawn from Marco themes, Marco's focus, placement and workspace rules,
and configuration read from the existing `org.mate.Marco.*` gsettings.

It is written in Rust on [Smithay](https://github.com/Smithay/smithay).

The name is Argentine: Caminito, "little way", is the street in La Boca and the tango, and it
keeps the pun on Wayland.

> **Status:** early development. Caminito v0.1 is being built in nine milestones. The first six
> (M1, a nested compositor, M2, Marco frames, M3, XWayland, M4, on the metal, M5, the MATE
> session, and M6, the MATE integrations) are done, and M7 (lock, idle and accessibility) is next.
> Today Caminito runs as a window inside another desktop or bare on a tty, drawing Marco frames
> around Wayland and X11 windows on the machine's monitors, and _MATE on Caminito_ logs in and out
> with mate-panel, caja's desktop and mate-settings-daemon, with the panel's shortcuts, the
> workspace switcher, the control center's dialogs and screenshots working in it. The session
> can't lock yet, so it isn't ready for daily use.

## Goals for v0.1

- Full Marco parity: every `org.mate.Marco.*` key and keybinding action honoured, except the
  X11-only ones.
- Server-side frames drawn from Marco/Metacity themes by a Rust port of Marco's theme engine,
  tested against reference renderings from Marco itself.
- XWayland, with X11 apps managed and framed like native ones.
- A "MATE on Caminito" session entry that runs mate-session, mate-panel (window list and workspace
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
| M5  | MATE session                 | "MATE on Caminito" logs in and out with mate-panel, caja's desktop and msd                              |
| M6  | MATE integrations            | Alt+F1/F2, pager layout, Show Desktop, mate-control-center dialogs, window screenshots                |
| M7  | Lock, idle and accessibility | the lock fails closed; idle, blanking, DPMS, Orca and AccessX work                                    |
| M8  | Marco parity                 | every Marco key and action accounted for                                                              |
| M9  | Flatpak                      | a Flatpak app runs in its sandbox, offered no privileged global; `v0.1.0` is tagged                   |

## Building

Caminito targets Arch Linux and builds with stable Rust (see [rust-toolchain.toml](rust-toolchain.toml)).

```sh
sudo pacman -S --needed rustup gcc libxkbcommon pixman dbus
cargo build --workspace
```

To build and install everything from nothing, run `tools/bootstrap`. It installs the build
tools, builds the patched MATE packages and `caminito-git` in order, and ends with
`caminito-desktop`, a meta package that depends on `caminito-git` and on the rest of Arch's `mate`
and `mate-extra` groups. Its arguments go to `makepkg`:

```sh
tools/bootstrap              # or: tools/bootstrap --noconfirm --nocheck
```

To install only the compositor as a package, build `caminito-git`'s PKGBUILD. It packages the
checkout's committed `HEAD`, not uncommitted work:

```sh
cd packaging/arch/caminito-git
makepkg -si
```

### Patched MATE packages

Some MATE components need carried patches to work in the session
([ADR 0011](docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-caminito.md)). Each is a
`<pkgname>-caminito` package under [packaging/arch/](packaging/arch/), made from Arch's own
PKGBUILD, that replaces the stock package. Its patches do nothing unless Caminito is running, or
are correct on any compositor, or only let the baseline build against Arch's libraries, so X11
MATE stays as it was. Build them before `caminito-git`, which depends on them:

```sh
for package in mate-session-manager-caminito mate-panel-caminito mate-settings-daemon-caminito \
  mate-control-center-caminito caja-caminito mate-utils-caminito mate-power-manager-caminito \
  caminito-git; do
  (cd packaging/arch/$package && makepkg -si)
done
```

`mate-panel-caminito` is built from mate-panel's master, where the applet library has a new
soname (`libmate-panel-applet-4.so.2`). Arch's applets in other packages, such as `mate-applets`,
are linked to the old one, so the package also builds the 1.28 library and installs it beside
master's: each of those applets is a program of its own, which talks to the panel over D-Bus.
The applet of `mate-notification-daemon` is loaded into the panel's process instead, where the two
libraries can't work together, and doesn't work until that package is built again.

`mate-settings-daemon-caminito` is msd 1.29.1, which asks for mate-desktop 1.29.0. Arch has 1.28.2,
so one of the package's patches takes out the one upstream commit that needs the newer library (a
change to the X11 display plugin for virtual machines that resize the screen).

`caja-caminito` is Arch's caja 1.28.0 with one patch, which puts the desktop on Caminito's primary
output.

`mate-utils-caminito` is mate-utils 1.29.1, the first whose mate-screenshot captures on Wayland,
with one patch: `mate-screenshot --window`, which `<Alt>Print` runs, captures the window Caminito
names as the active one. Its dictionary applet is built into the panel's process, as a Wayland
panel needs it, so it's linked to `mate-panel-caminito`'s applet library.

`mate-power-manager-caminito` is Arch's mate-power-manager 1.28.1 with one patch, which grabs
the brightness, keyboard backlight, power, sleep, hibernate, screensaver and battery keys
through Caminito. mate-power-manager runs on XWayland in the session, where its own grabs see a
key only while an XWayland window has the keyboard.

`vala-panel-appmenu-mate-caminito` is the one patched package that isn't a MATE component, and
nothing depends on it: build it for a global menu in the panel
([ADR 0017](docs/adr/0017-caminito-brokers-the-global-menu-for-a-patched-appmenu-applet.md)).
It's the AUR's `vala-panel-appmenu-mate` 25.04, with four patches: under Caminito the *Global
Application Menu* applet draws the menu Caminito publishes for the focused window, and while
the applet is on a panel apps leave their menubars to it. On any desktop, a menu Firefox
remakes stays open, and a shortcut of two presses, as VS Code's Ctrl+K Ctrl+O, is drawn whole
where the applet drew only its first. So far that's the menu of a
`GtkApplication`, as GTK 4 apps and some GTK 3 apps have, of a Qt 6 app, and of Firefox and
VS Code once they're [set up for it](#the-global-menu). A dialog with no menu shows its
parent's. The application's name is in bold before the menus, and each applet's
*Preferences* can turn it off. GTK apps get their menubars back about ten seconds after the
last applet is removed, so that a restart of the panel changes nothing. It's linked to
`mate-panel-caminito`'s applet library, so build it after that package.

`appmenu-gtk-module-caminito` gives the applet the menus of the GTK 3 apps that build a classic
menubar, as most of MATE's do. Nothing depends on it either. It's Arch's `appmenu-gtk-module`
25.04 with one fix of an upstream bug: on Wayland the module said nothing of a menubar in a
window that isn't a `GtkApplicationWindow`, such as mate-terminal's. The session adds the
module to its `GTK_MODULES` when this package is installed, and not for the stock one, with
which those apps would have no menu at all while the applet is on a panel.

The patches are `git format-patch` files beside each PKGBUILD, and they are the only copy. Each
series applies to a baseline: an upstream tag, or a pinned commit where the component has no
tag. To move a package to a new baseline, rebase its series:

1. In a checkout of the component, at the old baseline, apply the series:

   ```sh
   git checkout -b caminito <old baseline>
   git am /path/to/caminito/packaging/arch/<pkgname>-caminito/*.patch
   ```

2. Move it to the new baseline, and correct what breaks:

   ```sh
   git rebase --onto <new baseline> <old baseline> caminito
   ```

3. Export the series again, then update the PKGBUILD's source, `pkgver` and checksums
   (`updpkgsums`). Commit the patches and the PKGBUILD together, one commit for each move:

   ```sh
   rm /path/to/caminito/packaging/arch/<pkgname>-caminito/*.patch
   git format-patch --zero-commit --no-signature -N \
     -o /path/to/caminito/packaging/arch/<pkgname>-caminito <new baseline>
   ```

A change to the patches alone, on the same baseline, increases `pkgrel` instead.

## Running

A bare `caminito` chooses where it runs from the environment. With `WAYLAND_DISPLAY` or `DISPLAY`
set it runs nested, as a winit window inside that Wayland or X11 desktop. With neither, as on a
spare tty, it drives the machine's monitors itself.

To run it nested, use `tools/dev-nested`. It runs the installed `caminito` on a private session bus
with a scratch `XDG_CONFIG_HOME`, so it doesn't touch your desktop's bus or dconf settings.
`--bin` builds and runs this checkout's debug build instead, and any other arguments go to
`caminito`:

```sh
tools/dev-nested        # the installed caminito
tools/dev-nested --bin  # target/debug/caminito
```

Once Xwayland is ready it prints the two displays, `dev-nested: WAYLAND_DISPLAY=wayland-N
DISPLAY=:M`. Point clients at them:

```sh
WAYLAND_DISPLAY=wayland-1 gtk4-demo
DISPLAY=:1 xterm
```

Logs go to stderr at `info`; set `RUST_LOG` (for example `RUST_LOG=caminito=debug`) for more.

The keybindings are read from Marco's `org.mate.Marco.global-keybindings` and
`org.mate.Marco.window-keybindings`, and a change applies at once. Marco's defaults bind:

| Keys                        | Action                                   |
| --------------------------- | ---------------------------------------- |
| Ctrl+Alt+Left / Right       | Switch workspace                         |
| Ctrl+Shift+Alt+Left / Right | Move the window to the next workspace    |
| Alt+Tab                     | Switch windows, with the Window switcher |
| Alt+\`                      | Switch the focused application's windows |
| Ctrl+Alt+Tab                | Switch panels and the desktop            |
| Alt+F4                      | Close window                             |
| Alt+F10                     | Toggle maximised                         |
| Alt+F5                      | Unmaximise                               |
| Alt+F9                      | Minimise                                 |

Shift with a switch binding steps backward. While the Window switcher is open, Tab and the
arrow keys move the selection, releasing the modifier or Enter activates the selected window,
and Escape cancels. While the modifiers of a switch or a move to the workspace beside are held,
the Workspace popup shows the workspaces in their grid, and the arrow keys go on from there.

### The MATE session

`caminito --session` runs mate-session as Caminito's child, and Caminito exits with it and with its
status. `caminito --session=CMD` runs `CMD` instead, for example `--session=foot`. The `caminito-git`
package installs the session entry, _MATE on Caminito_
(`/usr/share/wayland-sessions/mate-caminito.desktop`), which a display manager lists, and depends
on the patched MATE packages the session needs.

The session logs in from a display manager, as _MATE on Caminito_. It can't lock the screen until
M7, so don't leave it unattended. Log out of your other sessions first: every login of one user
shares one systemd user manager and one session bus, and the session changes the environment of
both, so it would disturb a desktop you are logged in to. To try it beside your own desktop, use
a second user, which has its own:

```sh
sudo useradd -m caminitotest
sudo passwd caminitotest
```

Switch to a spare tty (Ctrl+Alt+F4, for example), log in as `caminitotest`, and run:

```sh
caminito --session
```

Logging out from the panel's menu ends the session and gives the tty back.

When Caminito's stderr isn't a terminal, as from a display manager, Caminito logs to the journal,
and so do its helpers and Xwayland, each under its own name:

```sh
journalctl --user -b -t caminito               # Caminito
journalctl --user -b -t Xwayland             # Xwayland
journalctl --user -b -t caminito-gtk-palette   # a helper
```

mate-session and the programs it starts write to Caminito's stderr itself, so they are where the
display manager sends that.

On a tty Caminito's stderr is the terminal, so all of it goes there. To have Caminito's logs in the
journal, and those of mate-session's programs in a file, send stderr to the file:
`caminito --session 2>~/session.log`.

### The global menu

With `vala-panel-appmenu-mate-caminito` installed, add the *Global Application Menu* applet to a
panel, and the focused window's menubar is drawn there instead of in the window.

- **A `GtkApplication`'s window** follows the applet at once: its menubar moves to the panel
  when the applet is added, and back when it's removed.
- **A Qt 6 app** needs no setting up, but looks for the global menu once, when it starts. One
  that was already running when the applet was added keeps its menubar in its window until it's
  started again, and one that was running when the applet was removed has no menubar until
  it's started again.
- **Firefox** has it off by default. Set both `widget.gtk.global-menu.enabled` and
  `widget.gtk.global-menu.wayland.enabled` to `true` in `about:config`, and install
  `libdbusmenu-glib` and `libdbusmenu-gtk3`, which Firefox loads if they're there. Like a Qt
  app, a window follows the applet only from Firefox's next start.
- **VS Code, VSCodium, Code - OSS and Cursor** have their menus in the panel on native Wayland
  with the custom title bar, the default, once `caminito-vscode-git` is installed (build it
  from `packaging/arch/caminito-vscode-git` as `caminito-git` is built). The package links its
  extension, *Caminito Global Menu*, into each of these editors that's installed, for every
  profile, and pacman updates and removes it with the package or the editor
  ([ADR 0022](docs/adr/0022-vs-code-s-extension-is-a-built-in-linked-by-an-alpm-hook.md)).
  Nothing is set up in the editor, and each window, local or attached to SSH or a container,
  follows the applet while the editor runs. The first time a window's menu goes to the panel
  while its menubar shows, the editor asks once: *The panel now shows this window's menus.
  Hide the menu bar in the window? Pressing Alt shows it again.* **Hide Menu Bar** sets
  `window.menuBarVisibility` to `toggle`, which hides the menubar until Alt is pressed, and
  **Keep Menu Bar** changes nothing. Either way the question isn't asked again:
  - `caminito.globalMenu.offerToHideMenuBar` turned back on asks it again, the next time a
    window's menu goes to the panel. It's kept for this machine, and Settings Sync leaves it
    out.
  - With Settings Sync on, the choice goes to your other machines. To hide the menubar on this
    one only, add `window.menuBarVisibility` to `settingsSync.ignoredSettings`.
  - *View > Appearance > Menu Bar* in the editor's menu never sets `toggle` again, so once the
    menubar is back through it, set `window.menuBarVisibility` to `toggle` by hand to hide it.

  An editor the package doesn't reach, such as one unpacked in your home directory, gets the
  extension from one link into its `resources/app/extensions`:

  ```sh
  ln -s /usr/share/caminito-vscode/global-menu ~/VSCode-linux-x64/resources/app/extensions/caminito.global-menu
  ```

  To turn it off in one profile, find *Caminito Global Menu* under `@builtin` in the editor's
  Extensions view and disable it there. `--disable-extension caminito.global-menu` on the
  editor's command line turns it off for that run.

  The menu is VS Code's own, with your key bindings. It doesn't reach a Flatpak
  editor, never greys an item out, shows a checkmark only for a setting it can read, leaves
  out other extensions' items, and isn't translated. A chord's shortcut, such as Ctrl+K
  Ctrl+O, is drawn whole from the applet's package 25.04-5, and `caminito-vscode-git`
  conflicts with older ones. Alt+F doesn't open the panel's menu.

  Without the package, VS Code has a global menu only as an XWayland window, since an Electron
  app says where its menu is by an X window's id alone. Put `--ozone-platform=x11` in
  `~/.config/code-flags.conf`, and set `window.titleBarStyle` to `native` in its settings. It's
  then drawn at the X11 scale
  ([ADR 0008](docs/adr/0008-fractional-default-scale-integer-x11-scale.md)), so it's soft on
  an output with a fractional scale. Like a Qt app, it follows the applet only from its next
  start.

An app that starts with no applet on a panel keeps its own menubar.

## Known limitations

- **An Xwayland crash logs you out of a MATE on Caminito session.** Caminito survives it, drops the
  XWayland windows and starts Xwayland again on the same `DISPLAY`, backing off and giving up after
  repeated crashes. But mate-session is an X11 client, and it exits when it loses its display,
  which ends the session. Outside a session, new X clients just work again.
- **On NVIDIA, turning one monitor off or on can blank another for a moment.** Seen on an RTX 3060
  with driver 615.71.09: a monitor on DisplayPort at 144 Hz goes black briefly when one on HDMI is
  unplugged, plugged in, or turned off or on. The other way round, nothing happens. Caminito changes
  only the monitor that comes or goes, and wayfire does the same on that machine, so the cause is
  the driver's or the hardware's. No window is lost.
- **One keyboard layout for every window.** Caminito switches layouts globally, so
  `org.mate.peripherals-keyboard-xkb.general`'s `group-per-window` (on by default) and
  `default-group` have no effect. The layout indicator applet runs on Xwayland, so it may not
  show the layout Caminito has switched to.
- **Some programs keep the cursor theme you logged in with.** Caminito's own cursors, the ones it
  draws for clients that ask for a cursor by name (GTK and foot among them), and
  XWayland's default cursor follow `org.mate.peripherals-mouse`'s `cursor-theme` and
  `cursor-size` at once. Programs that load cursors themselves from `XCURSOR_THEME` and
  `XCURSOR_SIZE` don't: Caminito exports those once, at start-up, so such a program shows the theme
  of the login until you log in again, even if it's started after the change. That includes X11
  programs that leave the theme to libXcursor, which reads the environment before the
  `Xcursor.theme` resource Caminito keeps up to date.

## Testing

```sh
cargo test --workspace --exclude caminito
dbus-run-session -- cargo test -p caminito
```

The tests are layered:

- **Policy:** window-management decisions are tested through a fake window type, with proptest
  invariants.
- **Headless:** `caminito`'s integration tests run the compositor on a pixman headless backend and
  drive it with a real `wayland-client` test client, asserting with [insta](https://insta.rs)
  snapshots.

`tools/check` runs rustfmt, clippy (`-D warnings`), cargo-deny, the tests and the extension's
checks, on the developer's Arch machine; `tools/check --deps` installs what they need first.
There's no hosted CI: GitHub Actions' metered minutes ran out, so every check runs locally.

Some checks need a real desktop and are run by hand before a milestone closes. They're listed in
[docs/testing/checklist.md](docs/testing/checklist.md).

## Layout

| Crate                                     | What it is                                                                                                                           |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| [caminito](caminito/)                         | The compositor: backends, Wayland protocol handlers, input and rendering                                                             |
| [caminito-policy](caminito-policy/)           | Window-management policy (focus, stacking, placement, workspaces, transients), free of protocol types                                |
| [caminito-layout](caminito-layout/)           | Docking and healing a layout of outputs, with no dependencies, for Caminito and caminito-displays                                        |
| [caminito-displays](caminito-displays/)       | The display settings program, in GTK4: a library with the page's state and its client of output management, and the window over them |
| [caminito-theme](caminito-theme/)             | Marco's theme engine, with no Smithay or GTK (from M2)                                                                               |
| [caminito-test-client](caminito-test-client/) | The Wayland client the integration tests drive                                                                                       |

## Design documents

- [CONTEXT.md](CONTEXT.md): the project's vocabulary. Caminito is a _compositor_, never a "window
  manager".
- [docs/adr/](docs/adr/): architecture decision records, from the choice of Rust and Smithay to how
  MATE packages are patched.

## License

GPL-2.0-or-later. See [COPYING](COPYING).
