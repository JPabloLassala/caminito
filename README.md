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

### Marco parity

Parity means that every key of Marco's five `org.mate.Marco.*` schemas, and each of its 103
keybinding actions, has one of four answers in [docs/parity.md](docs/parity.md), with the test
or the checklist item that covers it:

- **honoured:** Caminito does what Marco does with it, or what its row says. Every keybinding
  action is, and the control center's _System Shortcuts_ lists them.
- **never:** Caminito will not implement it, for the reason beside it: what only an X11 window
  manager can do, and what Marco itself never finished.
- **left to another component:** another program of the Session does it.
- **unsupported:** not done in v0.1, and on the roadmap after it. Caminito logs such a key once
  when it's set away from its default.

It doesn't mean pixel-for-pixel sameness everywhere. Where Caminito differs on purpose, as in
the Window switcher's grid of thumbnails in place of Marco's Alt+Tab popup, the row says how.

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
tools, builds the patched MATE packages, `caminito-git` and the global menu's packages in order,
and ends with `caminito-desktop`, a meta package that depends on `caminito-git` and on the rest
of Arch's `mate` and `mate-extra` groups. When it rebuilds `caminito-git` inside a running
Session, it restarts the Session's helpers (`caminito-notifications`, `caminito-wallpaperd` and
`caminito-gtk-palette`) so they run the new binaries; the compositor needs a new login. Its
arguments go to `makepkg`:

```sh
tools/bootstrap              # or: tools/bootstrap --noconfirm --nocheck
```

To install only the compositor as a package, build `caminito-git`'s PKGBUILD. It packages the
GitHub repo's default branch, not the checkout, as its AUR package does:

```sh
cd packaging/arch/caminito-git
makepkg -si
```

### Fedora

The Fedora packages are for the developer's machine: Fedora Asahi Remix 45 (aarch64, KDE Plasma
edition). Plasma Login Manager stays the display manager there, and Plasma stays the fallback
session. The specs are in [packaging/fedora/](packaging/fedora/).

`tools/bootstrap-fedora` is the equivalent of `tools/bootstrap`. Run it on that machine, as your own
user. It uses `sudo` where it needs to. It installs the build tools and Fedora's MATE
(`dnf group install mate-desktop`), and it keeps LightDM disabled. Then it builds the patched and
backported MATE packages and `caminito` in order, and installs the RPMs of each one before the next
build. Its arguments go to `rpmbuild`:

```sh
tools/bootstrap-fedora       # or: tools/bootstrap-fedora --nocheck
```

It holds each package that it installs with `dnf versionlock`. Fedora's `caja` and
`mate-power-manager` have the same version as ours and a higher release, so without the hold a
`dnf upgrade` replaces them with Fedora's. To see the hold, run `dnf versionlock list`.

You can run the script again. It builds every package again. It removes the hold of a package and
adds it again, so a package never has two entries. `dnf` skips a package that is installed at the
same version. If you change a patch, raise the `Release` of the spec.

To go back to Fedora's MATE, remove the hold and `caminito`, then sync to Fedora's versions. Remove
`caminito` first: it is not in Fedora, and it needs the patched packages.

```sh
names=$(rpm -qa --qf '%{NAME} %{RELEASE}\n' | sed -n 's/ 0\.caminito.*//p')
sudo dnf versionlock delete caminito $names
sudo dnf remove caminito
sudo dnf distro-sync
```

### Patched MATE packages

Some MATE components need carried patches to work in the session
([ADR 0011](docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-caminito.md)). Each is a
`<pkgname>-caminito` package under [packaging/arch/](packaging/arch/), made from Arch's own
PKGBUILD, that replaces the stock package. Its patches do nothing unless Caminito is running, or
are correct on any compositor, or only let the baseline build against Arch's libraries, so X11
MATE stays as it was. Build them before `caminito-git`, which depends on them. A patched package
builds through `tools/bootstrap`, not a plain `makepkg`: its PKGBUILD lists its patches by bare
filename, as its AUR package has them, and `tools/bootstrap` copies them beside it first
([ADR 0052](docs/adr/0052-the-aur-packages-are-published-from-the-monorepo.md)).

`mate-panel-caminito` is built from mate-panel's master, where the applet library has a new
soname (`libmate-panel-applet-4.so.2`). Arch's applets in other packages, such as
`mate-netbook`'s, are linked to the old one, so the package also builds the 1.28 library and
installs it beside master's: each of those applets is a program of its own, which talks to the
panel over D-Bus. An applet loaded into the panel's process can't use the 1.28 library there.
Arch builds `mate-applets` that way, and its Trash applet aborts the panel, so
`mate-applets-caminito` is Arch's `mate-applets` built again against master's library, with no
patch of Caminito's. The applet of `mate-notification-daemon` is loaded into the panel's process
too, and doesn't work until that package is built again. A Session doesn't run `mate-notification-daemon` at all: `caminito-notifications` is its notification daemon
(ADR 0051).

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

The patches are `git format-patch` files in [packaging/patches/](packaging/patches/), one
directory for each component under upstream's name, as `mate-panel` for `mate-panel-caminito` and
`vala-panel-appmenu` for `vala-panel-appmenu-mate-caminito`. They are the only copy, for every
distribution ([ADR 0030](docs/adr/0030-carried-patches-are-kept-once-for-every-distribution.md)).
A build patch that only Arch needs, such as msd's revert, is in the component's `arch/`, and
applies after the rest. The PKGBUILDs list the patches by bare filename, as each AUR package
has them beside its PKGBUILD. `tools/bootstrap` removes the old copies beside each PKGBUILD and
copies the series and the component's `arch/` there, where git ignores them, before `makepkg`
([ADR 0052](docs/adr/0052-the-aur-packages-are-published-from-the-monorepo.md)). To run
`makepkg` or `updpkgsums` without `tools/bootstrap`, copy them yourself:

```sh
cd /path/to/caminito/packaging/arch/<pkgname>
rm -f ./*.patch
cp ../../patches/<component>/*.patch .
cp ../../patches/<component>/arch/*.patch .   # if the component has an arch/
```

For one machine with Fedora Asahi Remix 45, the same components, and marco 1.29.1, have
Fedora specs in [packaging/fedora/](packaging/fedora/). Fedora isn't a supported distribution.
Each spec is Fedora's own, with the smallest diff: a package keeps Fedora's names and
subpackages, its `Version` is the baseline, and its `Release` is `0.caminito<n>`. The specs
take the same shared patches from packaging/patches/, and a build patch that only Fedora needs
is in the component's `fedora/`, as msd's revert is. `packaging/fedora/build <component>`, run
on the Fedora machine after `sudo dnf builddep` on the spec, copies the patches beside the
spec's other sources, fetches and checks the tarballs, and runs `rpmbuild -ba`:

```sh
sudo dnf builddep packaging/fedora/mate-panel/mate-panel.spec
packaging/fedora/build mate-panel
```

Build marco, mate-panel and mate-settings-daemon first, and install their RPMs: the others
build against them. Fedora's mate-panel is 1.28, so the panel's `mate-panel-libs` has the
1.28.4 applet library too, as on Arch.

Each series applies to a baseline: an upstream tag, or a pinned commit where the component has
no tag. To move a component to a new baseline, rebase its series:

1. In a checkout of the component, at the old baseline, apply the series, without `arch/`:

   ```sh
   git checkout -b caminito <old baseline>
   git am /path/to/caminito/packaging/patches/<component>/*.patch
   ```

2. Move it to the new baseline, and correct what breaks:

   ```sh
   git rebase --onto <new baseline> <old baseline> caminito
   ```

3. Export the series again:

   ```sh
   rm /path/to/caminito/packaging/patches/<component>/*.patch
   git format-patch --zero-commit --no-signature -N \
     -o /path/to/caminito/packaging/patches/<component> <new baseline>
   ```

4. If the component has a build patch in `arch/`, apply it on top (`git am`), and export it
   again into `arch/` if it needed a change. It goes once Arch's libraries don't need it.

5. Copy the new patches beside the PKGBUILD, as above, then update its source, `pkgver` and
   checksums (`updpkgsums`). Commit the patches and the PKGBUILD together, one commit for each
   move.

A change to the patches alone, on the same baseline, increases `pkgrel` instead, and needs the
new copies too.

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
| Alt+Esc                     | Cycle windows, with no popup             |
| Alt+F6                      | Cycle the focused application's windows  |
| Ctrl+Alt+Esc                | Cycle panels and the desktop             |
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
- **Firefox** has its menus in the panel once `caminito-firefox-global-menu` is installed
  (build it from `packaging/arch/caminito-firefox-global-menu` as `caminito-git` is built). For
  every profile, it turns on `widget.gtk.global-menu.enabled` and
  `widget.gtk.global-menu.wayland.enabled`, which `about:config` can turn off again, and pulls in
  `libdbusmenu-glib` and `libdbusmenu-gtk3`, which Firefox loads if they're there. It also keeps
  the autohide Menu Bar toolbar hidden while the menus are in the panel: Firefox would otherwise
  show it, empty, above the tabs when a menu is opened from the panel
  ([ADR 0053](docs/adr/0053-firefox-s-global-menu-is-set-up-by-an-autoconfig-package.md)). It
  does so through Firefox's autoconfig, of which Firefox reads one, so it can't be used beside
  another `mozilla.cfg`. Without the package, set the two prefs to `true` yourself. Like a Qt
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

### Bluetooth

MATE has no Bluetooth front end of its own, so Caminito has two pieces (ADR 0035):

- **The Bluetooth applet** is an icon in mate-panel. Its popup has a switch for Bluetooth, the paired
  devices with their battery levels, a Connect or Disconnect button for each, and _Add New
  Device…_. It hides itself on a machine with no Bluetooth adapter.
- **`caminito-bluetooth`** runs through the whole session, started at login from
  `/etc/xdg/autostart/caminito-bluetooth.desktop`. It is the pairing agent: it answers a PIN, a
  passkey or a confirmation, whether you started the pairing or the device did. It also opens the
  Bluetooth settings window, which is in the control center under Hardware, and from the applet's
  _Configure Bluetooth…_.

Turning Bluetooth off is an rfkill soft block, so it lasts across reboots and agrees with the
airplane-mode key. Both pieces talk to BlueZ alone, so they depend on `bluez` and on nothing of
Caminito, and work under X11 MATE too.

[Blueman](https://github.com/blueman-project/blueman) can be installed beside them. BlueZ gives the
default agent to whichever registers last, so the one that started last answers the pairing requests.
Stop the other's agent, or don't start it, to choose.

### Power

The **Power applet** is a battery icon in mate-panel, in a new session's top panel beside the
Bluetooth applet. Its tooltip has the charge and the time left or until full, and where there is
no battery it's a plug. Its popup has:

- The power profiles, Power Saver, Balanced and Performance, as tiles, with why Performance is
  degraded and which application holds a profile. They are there only when power-profiles-daemon,
  or tuned-ppd, runs.
- _Stay awake_, which keeps the computer from sleeping and the screen from locking until it's
  turned off, and the applications that keep the computer awake.
- Every battery: the laptop's, with its health and, where the battery supports it, _Limit
  charging to 80%_ under its ⋮ button (the firmware's own end threshold where it has one), and
  the mouse's, the keyboard's and the headset's.
- _Configure Power Management…_, which opens mate-power-preferences.

It reads UPower, power-profiles-daemon and mate-session directly, so it depends on `upower`, and
optionally on `power-profiles-daemon`, and on nothing of Caminito, and works under X11 MATE too.
There, so that only one battery icon shows, the package turns mate-power-manager's own icon off
by default. In mate-power-preferences, _Only display an icon when a battery is present_, its
setting before the package, turns it on again. A user who had panel settings before keeps their
panel, and adds _Power_ with _Add to Panel_.

### Temperatures and fans

The **Thermal applet** is a thermometer in mate-panel with the CPU's temperature, and the hottest
GPU reading beside it when there is a GPU, over the fastest fan's speed, or _Fans Off_ when every
fan is stopped. A
temperature turns orange from 75 °C, and red from 90 °C, or from 5 °C below the sensor's critical
value when that is lower. The two thresholds can be changed in Preferences. On a panel smaller
than 32 px it's one line, and on a vertical panel it's the CPU's temperature alone. Its tooltip names the temperatures in the panel, and every fan
with its speed.

A click opens its popup: a tile for each of the CPU, the GPU, the storage and the fans, with the
hottest reading of each group and the fastest fan. Under _Show all sensors_ is every reading,
grouped, with a bar from 20 °C to 100 °C in its colour, and every fan with its speed. A fan's bar
is against its maximum, or, when the fan gives none, against the fastest it has spun since the
applet started, after ten readings.

The pin at the end of a temperature's row shows on hover, and puts that temperature in the panel.
The panel has two at most: pinning a third takes out the one pinned before the other. The pin of a
temperature in the panel stays shown, and a click on it takes the temperature out, unless it's
the only one. While the chip of one of the temperatures chosen is gone, as when its module is
unloaded, the panel shows the CPU's and the GPU's again, until the chip comes back.

_Preferences_, in the applet's right-click menu, has the units, °C or °F, for the panel, the
tooltip and the popup, and where warm and hot start. The applet keeps these, the temperatures in
the panel, and whether _Show all sensors_ is open, when mate-panel restarts.

It isn't in a new session's panel: add _Temperatures and Fans_ with _Add to Panel_. It reads every
chip through lm_sensors' libsensors, so the labels and the `ignore` lines in `/etc/sensors3.conf`
and `/etc/sensors.d/` apply to it. It depends on `lm_sensors` and on nothing of Caminito, and works
under X11 MATE too.

It only watches: it never sets a fan's speed.

A SATA drive's temperature shows only when the kernel's `drivetemp` module is loaded, and nothing
loads it automatically. To load it now and at each boot:

```sh
sudo modprobe drivetemp
echo drivetemp | sudo tee /etc/modules-load.d/drivetemp.conf
```

NVIDIA's proprietary driver gives its GPU's temperature only through NVML, not through hwmon, so
the applet doesn't show it.

### Night Light

*Night Light* in the menu's Preferences opens caminito-displays on its Night Light tab
(`caminito-displays --night-light`). There it's turned on and off, follows the sun or holds one
temperature, and pauses until sunrise.

No shortcut toggles it by default. To bind one, open *System Shortcuts* in the control center
(*Keyboard Shortcuts* outside a Session),
choose *Add*, give it a name, such as *Night Light*, and the command:

```sh
caminito-displays --toggle-night-light
```

then click the new shortcut's row and press the keys. The command turns Night Light on if it's
off and off if it's on, and opens no window.

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
| [caminito-about](caminito-about/)             | The "About Caminito" of the control center, in GTK4                                                                                  |
| [caminito-wallpaper](caminito-wallpaper/)     | The wallpaper: `caminito-wallpaperd`, which draws each output's, with no GTK, and the Wallpaper dialog, in GTK4                      |
| [caminito-behaviour](caminito-behaviour/)     | Workspace Behaviour, the settings of how Caminito itself behaves, in GTK4: the Hot corners for now                                  |
| [caminito-version](caminito-version/)         | The version string with the build's revision, for `caminito --version` and caminito-about                                            |
| [caminito-theme](caminito-theme/)             | Marco's theme engine, with no Smithay or GTK (from M2)                                                                               |
| [caminito-test-client](caminito-test-client/) | The Wayland client the integration tests drive                                                                                       |

## Design documents

- [CONTEXT.md](CONTEXT.md): the project's vocabulary. Caminito is a _compositor_, never a "window
  manager".
- [docs/adr/](docs/adr/): architecture decision records, from the choice of Rust and Smithay to how
  MATE packages are patched.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). It tells the licence of a contribution, the sign-off
that each commit must have, and where to report bugs.

## License

GPL-3.0-or-later. See [COPYING](COPYING).

The files from third parties keep their own licences and copyright. [REUSE.toml](REUSE.toml) and
the SPDX headers of those files give each file's licence and copyright, in the
[REUSE](https://reuse.software) layout. [LICENSES/](LICENSES/) has the text of each licence.
