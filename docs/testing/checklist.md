# Manual checklist

The checks CI can't run: anything that needs a real desktop, real logind, real hardware, msd or
Orca. Before a milestone closes, run every item tagged with that milestone, and every item from
an earlier milestone that still applies, on the installed package. Record the results in the
milestone's `Close M<n> <name>` commit message.

Each item has:

- **Milestone:** the milestone that introduces it, `M1` to `M8`. An item that needs something from
  a later milestone (msd, logind, Orca, two monitors) is tagged with that milestone, not the one
  that wrote the code.
- **Steps:** what to do.
- **Expected:** what counts as passing.
- **Debug tool:** what shows the result.

Several items read the nested Camino's environment. With it running under `tools/dev-nested`:

```sh
nested_env() {
  tr '\0' '\n' </proc/"$(pgrep -n -x camino)"/environ |
    grep -E '^(DBUS_SESSION_BUS_ADDRESS|XDG_CONFIG_HOME)='
}
```

The items on the tty run bare `camino` as `caminotest`, the second user (14). Log in as
`caminotest` on a spare tty and run `camino 2>~/camino.log`. Run the commands of each item from a
second login of `caminotest` (another tty, or `ssh`), which shares the first one's session bus,
with `WAYLAND_DISPLAY` set to the socket in the log's `listening on WAYLAND_DISPLAY=` line, and
`DISPLAY` to the one in its `Xwayland is ready on DISPLAY=` line for X11 apps. Ctrl+Alt+F<n>
switches between the ttys.

The items in the session run `camino --session` instead, the same way or from the display
manager, and their commands run in a terminal inside the session or from a second login. A
second login gets the session's displays from systemd, where Camino exports them, as in
`env $(session_env) wlr-randr`:

```sh
session_env() {
  systemctl --user show-environment | grep -E '^(WAYLAND_DISPLAY|DISPLAY)='
}
```

## Nested runs

### A nested run uses its private bus

- **Milestone:** M1
- **Steps:**
  1. Run `tools/dev-nested`.
  2. In another terminal, run `nested_env` and `echo "$DBUS_SESSION_BUS_ADDRESS"`.
- **Expected:** Camino's `DBUS_SESSION_BUS_ADDRESS` is `dbus-run-session`'s own bus, not the
  host's (`unix:path=/run/user/<uid>/bus`).
- **Debug tool:** `/proc/<pid>/environ`.

### dconf goes to the scratch config

- **Milestone:** M1
- **Steps:**
  1. Run `tools/dev-nested`. It prints its scratch `XDG_CONFIG_HOME`.
  2. In another terminal, write a key on the nested bus and read it back on both buses:

     ```sh
     env $(nested_env) sh -c 'dconf write /org/camino/checklist true; dconf read /org/camino/checklist; ls "$XDG_CONFIG_HOME/dconf"'
     dconf read /org/camino/checklist
     ```

  3. Quit Camino.
- **Expected:** the nested read prints `true` and `$XDG_CONFIG_HOME/dconf/user` exists in the
  scratch directory. The host read prints nothing. The scratch directory is gone after Camino
  exits.
- **Debug tool:** `dconf read`, and the scratch directory's `dconf/user`.

### The host desktop's systemd environment is untouched

- **Milestone:** M1
- **Steps:**
  1. `systemctl --user show-environment | sort >/tmp/env-before`
  2. Run `tools/dev-nested`, map a client in it, and quit.
  3. `systemctl --user show-environment | sort >/tmp/env-after`
  4. `diff /tmp/env-before /tmp/env-after`
- **Expected:** no difference.
- **Debug tool:** `systemctl --user show-environment`.

## Debugging

### The debug interface answers on the nested bus

- **Milestone:** M2
- **Steps:**
  1. Run `tools/dev-nested`, and a GTK3 app such as pluma in it
     (`WAYLAND_DISPLAY=<camino's socket> GDK_BACKEND=wayland pluma`).
  2. With `DBUS_SESSION_BUS_ADDRESS` from `nested_env`, run
     `busctl --user call org.mate.Camino.Debug /org/mate/Camino/Debug org.mate.Camino.Debug DumpState`.
  3. Run the same with `ToggleFrameDebug` instead of `DumpState`, then again.
- **Expected:** the dump lists the app's window as `ssd`, with its frame extents and strip scale.
  The first toggle answers `true` and outlines the frame's strips in red, its invisible resize
  border in blue and its title in green, labelled with the strip scale (`×1`). The second answers
  `false` and the outlines go.
- **Debug tool:** `busctl`.

## Frames

### Frames at an output scale of 2 in a nested run

- **Milestone:** M2
- **Steps:**
  1. Run `tools/dev-nested --scale 2`, and pluma in it as above.
  2. With `DBUS_SESSION_BUS_ADDRESS` from `nested_env`, run `DumpState` as above, then
     `ToggleFrameDebug`.
  3. Hover, press and release each titlebar button, drag the titlebar, and resize from an edge
     and a corner.
- **Expected:** the dump lists the output at `scale` 2 and pluma's window as `ssd` with
  `strip_scale` 2, and the same `frame_extents` as at scale 1. Frame debug labels the frame
  `×2`, and its outlines sit on the frame's edges. The frame is as sharp as pluma's own content:
  the title and the button icons aren't blurred or doubled in size. The buttons prelight, press
  and act, the titlebar moves the window, and the edges resize it, where the outlines say they
  should.
- **Debug tool:** frame debug and `busctl`.

### A theme switch while windows are open

- **Milestone:** M2
- **Steps:**
  1. Run `tools/dev-nested`, and pluma, caja (`caja --no-desktop`) and gtk3-demo in it. Maximise
     pluma, and leave caja at its size. gtk3-demo is CSD, so pluma and caja carry the frames. A
     second pluma won't do: pluma hands a new window to the running instance.
  2. On the nested bus, switch the theme a few times, then to one that doesn't exist, then back,
     dumping the state after each switch:

     ```sh
     for theme in BlackMATE HighContrast TraditionalOk NoSuchTheme Menta; do
       env $(nested_env) gsettings set org.mate.Marco.general theme "$theme"
       sleep 2
       env $(nested_env) busctl --user call org.mate.Camino.Debug /org/mate/Camino/Debug \
         org.mate.Camino.Debug DumpState
     done
     ```

- **Expected:** each frame changes to the new theme as soon as it's set, with no app restarted
  and no window unmapped. The unmaximised window's client area keeps its size, the maximised
  one and its frame still fill the output, and `frame_extents` follow the theme. `NoSuchTheme` gives the built-in
  theme and one `can't load the Marco theme NoSuchTheme` line in Camino's log. Back on Menta,
  the frames are Menta's again, and the buttons still prelight, press and act.
- **Debug tool:** `busctl` and Camino's stderr.

## XWayland

### An X11 app's copy and paste to and from a native app

- **Milestone:** M3
- **Steps:**
  1. Run `tools/dev-nested`. Once Xwayland is ready it prints
     `dev-nested: WAYLAND_DISPLAY=wayland-N DISPLAY=:M`.
  2. In other terminals, run foot on Wayland (`WAYLAND_DISPLAY=wayland-N foot`), and xterm on X
     with foot's clipboard keys. xterm's own selection and middle-click stay on the primary
     selection:

     ```sh
     DISPLAY=:M xterm -xrm 'XTerm*VT100.translations: #override Ctrl Shift <Key>C: copy-selection(CLIPBOARD)\nCtrl Shift <Key>V: insert-selection(CLIPBOARD)'
     ```

  3. Clipboard: select text in one, press Ctrl+Shift+C, then click the other and press
     Ctrl+Shift+V. Both ways.
  4. Primary selection: select text in one, and middle-click in the other, both ways.
  5. Copy something new in foot and leave foot focused. From a host terminal, run
     `DISPLAY=:M xclip -o -selection clipboard`.
- **Expected:** the text pastes unchanged both ways, through both the clipboard and the primary
  selection. X only gets a Wayland selection once an X window is focused (37), so step 5 prints
  what xterm last copied, not foot's new text.
- **Debug tool:** the pasted text, and `xclip` on the nested `DISPLAY`.

### `DISPLAY` only exported after the root properties exist

- **Milestone:** M5
- **Steps:**
  1. Log into *MATE on Camino*. Before `camino --session`, nothing Camino spawns receives its
     environment, so in M3 the X11 tier's test that `DISPLAY` isn't set until the root properties
     exist (38) stands in.
  2. Read the `DISPLAY` Camino gave mate-session, the first process it spawns with its
     environment:

     ```sh
     tr '\0' '\n' </proc/"$(pgrep -o -x mate-session)"/environ | grep '^DISPLAY='
     ```

  3. `journalctl --user -b | grep -E 'Xwayland is ready|mate-session'` for the order.
  4. `xprop -root _NET_SUPPORTED _NET_NUMBER_OF_DESKTOPS _NET_DESKTOP_NAMES _NET_CURRENT_DESKTOP
     _NET_WORKAREA _GTK_WORKAREAS_D0` on that `DISPLAY`.
- **Expected:** mate-session's `DISPLAY` is the one in `Xwayland is ready on DISPLAY=:N`, which
  is logged before mate-session starts. `xprop -root` shows `_NET_SUPPORTED` with the desktop
  atoms, `_NET_WORKAREA` and `_GTK_WORKAREAS` listed, the desktops as in `org.mate.Marco.general`,
  and a work area per desktop.
- **Debug tool:** `/proc/<pid>/environ`, `xprop -root` and the journal.

## Session

### Logging into MATE on Camino

- **Milestone:** M5
- **Steps:**
  1. On a tty: log in as `caminotest` on a spare tty and run `camino --session 2>~/session.log`.
  2. From the display manager: log out of every session of the user first, since its logins
     share one systemd manager and bus (14). At the greeter, choose *MATE on Camino* in the
     session list and log in.
  3. Each time, look at the desktop, then run:

     ```sh
     systemctl --user show-environment | grep -E '^(WAYLAND_DISPLAY|DISPLAY|MATE_WM_NAME)='
     loginctl show-session "$XDG_SESSION_ID" -p Type -p Desktop -p TTY
     pgrep -a -u "$USER" -x 'camino|mate-session|mate-panel|caja|mate-settings-d' | sort -k2
     journalctl --user -b -t camino -p warning
     grep -aiE 'timed out|time-?out' ~/session.log
     ```

- **Expected:** the greeter lists *MATE on Camino*. Both ways the desktop is up well inside 30
  seconds, with no mate-session phase timing out (08): both panels, with the window list and the
  workspace switcher, caja's desktop icons, and msd's wallpaper under them on every output.
  systemd has `WAYLAND_DISPLAY`, `DISPLAY` and `MATE_WM_NAME=Camino`, and the five processes
  run. From the display manager the logind session's type is `wayland`. Camino's stderr isn't a
  terminal either way, so its lines are in the journal. mate-session and what it starts write to
  that stderr itself (72): `~/session.log` on the tty, and the display manager's log of the
  session otherwise, where the `grep` goes then. Neither has a time-out.
- **Debug tool:** `systemctl --user show-environment`, `loginctl`, the journal, and the
  `outputs` of `DumpState` for the panels' and the desktop's layer surfaces.

### Log Out and Shut Down from the panel

- **Milestone:** M5
- **Steps:**
  1. As `caminotest` on a tty, run `camino --session 2>~/session.log; echo "camino exited with $?"`.
  2. In the panel, choose *System*, *Shut Down…*, look at the dialog, and cancel it.
  3. Choose *System*, *Log Out caminotest…*, then *Log Out*.
  4. From a second login, run `systemctl --user show-environment` and `pgrep -a -u caminotest`.
  5. Log in as yourself, from the display manager, choose *Shut Down…* and then *Shut Down*.
     After the machine is back, read `journalctl --user -b -1 -t camino | tail`.
- **Expected:** *Shut Down…* shows mate-session's dialog (*Shut down this system now?*, with its
  countdown), an XWayland window that Camino frames, not the panel's own *System Shutdown* (70).
  Cancelling it leaves the session as it was. *Log Out* ends the session: the tty is back in text
  mode at the shell prompt, which prints `camino exited with 0`, mate-session's status (61).
  Nothing of the session still runs, and of its variables systemd has only
  `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` (61, 74). *Shut Down* powers the machine off, and
  the journal of that boot ends with `the session command exited`. It may also have `systemd
  didn't unset the environment`, since a systemd that's shutting down may not answer (61).
- **Debug tool:** the exit status, `systemctl --user show-environment`, the journal, and
  `dbus-monitor "interface='org.gnome.SessionManager'"` for the panel's `Logout` and `Shutdown`
  calls.

### An X11 app's clipboard contents survive the app quitting

- **Milestone:** M5
- **Steps:**
  1. Log into *MATE on Camino*, with no pluma running: pluma hands a new window to the running
     instance.
  2. Run `GDK_BACKEND=x11 pluma`, type a line, select it, copy it with Ctrl+C, and quit pluma.
  3. Wait five seconds. Run `pluma`, which is native, paste with Ctrl+V, and quit it.
  4. Run `GDK_BACKEND=x11 pluma` again and paste there.
  5. `wayland-info | grep ext_data_control`, and
     `grep -a 'data-control' ~/session.log` or the display manager's log of the session.
- **Expected:** the line pastes in both, though the app that owned it is gone: msd's clipboard
  plugin keeps a copy through `ext_data_control_manager_v1` and takes the selection about 2
  seconds after its owner goes (11, 68, 71). Step 5 lists the global, and msd never logged `The
  compositor does not support the ext-data-control protocol`.
- **Debug tool:** the pasted text, `wayland-info`, and
  `WAYLAND_DEBUG=1 mate-settings-daemon --replace 2>&1 | grep data_control` for msd's offers.

### X11 MATE and the Wayfire session are unchanged with the Patched packages

- **Milestone:** M5
- **Steps:**
  1. As the developer's own user, with the four Patched packages installed
     (`pacman -Q mate-session-manager mate-panel mate-settings-daemon caja` names the `-camino`
     ones),
     log into *MATE*, the X11 session, from the display manager.
  2. Run:

     ```sh
     echo "$XDG_SESSION_TYPE ${MATE_WM_NAME:-unset} ${GDK_BACKEND:-unset}"
     pgrep -a -x marco
     busctl --user status org.mate.Camino
     theme=$(gsettings get org.mate.interface gtk-theme)
     gsettings get org.gnome.desktop.interface gtk-theme
     gsettings set org.mate.interface gtk-theme TraditionalOk
     gsettings get org.gnome.desktop.interface gtk-theme
     gsettings set org.mate.interface gtk-theme "$theme"
     for applet in $(grep -h '^Location=' /usr/share/mate-panel/applets/* | cut -d= -f2); do
       ldd "$applet" | grep 'libmate-panel-applet.*not found' && echo "in $applet"
     done
     ```

  3. Use the panel: a window list button, the workspace switcher, every applet that was on the
     panel before, and *Log Out…* and *Shut Down…*, cancelling both.
  4. Log out, and log into *MATE on Wayfire*. Look at its panel, its desktop icons and its
     wallpaper, and log out.
- **Expected:** the session is as it was with the stock packages (the fall-back rule, ADR 0011).
  It's `x11`, with `MATE_WM_NAME` not Camino's and `GDK_BACKEND` as before, mate-session starts
  Marco, and nothing owns `org.mate.Camino`. msd serves XSETTINGS, so open apps restyle at the
  theme change, and it doesn't mirror into `org.gnome.desktop.interface`, which keeps its value.
  The panel's buttons, switcher and dialogs work, and no applet is missing from the panel: the
  loop prints nothing, since no installed applet is linked to an applet library that's gone
  (70). The Wayfire session logs in and out, with its panel, its desktop icons and its
  wallpaper where they were with the stock packages (81).
- **Debug tool:** `ldd` on the applets, `busctl`, `gsettings`, and `~/.xsession-errors` or the
  journal for what a missing applet logged.

### Camino restores the saved layout from the first frame

- **Milestone:** M6
- **Steps:**
  1. With `mate-settings-daemon-camino` at `pkgrel` 2 or later, log into *MATE on Camino* with
     both monitors connected, and note `wlr-randr`'s scales and positions, which are Camino's
     defaults (50) unless a layout is saved already. Keep a copy of
     `~/.config/camino/layouts.json` if there is one.
  2. Apply a layout that isn't the default, choose the other output as the primary one, and
     save it:

     ```sh
     wlr-randr --output DP-1 --scale 1.25 --pos 0,0 --output HDMI-A-1 --pos 1536,0
     gdbus call --session --dest org.mate.Camino --object-path /org/mate/Camino \
       --method org.freedesktop.DBus.Properties.Set org.mate.Camino PrimaryOutput "<'HDMI-A-1'>"
     gdbus call --session --dest org.mate.Camino --object-path /org/mate/Camino \
       --method org.mate.Camino.SaveLayout
     ```

  3. Apply another scale with `wlr-randr`, and don't save it. Log out and in again, watching
     the monitors as the session starts, and run `wlr-randr` and `DumpState`.
  4. Unplug the second monitor's cable, run `wlr-randr`, plug it back in, and run it again.
  5. Run `pgrep -a mate-settings-daemon` and `gdbus introspect --session --dest
     org.mate.SettingsDaemon --object-path /org/mate/SettingsDaemon/WLRANDR`.
  6. Remove `layouts.json`, log out and in once more, and then put the copy back.
- **Expected:** after step 3 each output has the mode, scale and place of step 2, not Camino's
  default and not the scale of step 3, and the panels are on the output chosen (81). The
  layout is there from the first frame: the outputs don't blank or jump once the session is
  up, as they did while msd applied it (06). In step 4 the monitor that stays is at the
  origin while it's alone, and the saved layout and the primary output are back when the
  other is plugged in. In step 5 msd runs, and the object has no
  `org.mate.SettingsDaemon.WLRANDR` interface: the plugin isn't started under Camino, and `~/.config/monitors-wayland.xml` is neither read nor written (ADR
  0014). After step 6 the defaults are back.
- **Debug tool:** `wlr-randr`, the `outputs` and `primary_output` of `DumpState`,
  `~/.config/camino/layouts.json`, and `can't restore the saved layout` in `~/camino.log`.

## Privileged clients

### A clipboard watcher sees each copy without a window

- **Milestone:** M5
- **Steps:**
  1. Run `tools/dev-nested`, and set `WAYLAND_DISPLAY` to the nested socket in two terminals.
  2. In the first, run `wl-paste --watch cat`, which is a data-control client with no window.
  3. In the second, run `wl-copy first`, then `wl-copy second`.
- **Expected:** the first terminal prints `first`, then `second`, as each is copied (68).
- **Debug tool:** `wl-paste --watch`, and `wayland-info | grep data_control`.

### A Flatpak app's sandbox is offered no privileged global

- **Milestone:** M5
- **Steps:**
  1. Run `tools/dev-nested`, and set `WAYLAND_DISPLAY` to the nested socket.
  2. Run `wayland-info | grep -c interface:`.
  3. Run `wayland-info` in a Flatpak sandbox, from the host's `/usr`, with any installed
     runtime:

     ```sh
     flatpak run --socket=wayland --filesystem=host-os:ro \
       --command=/run/host/usr/bin/wayland-info org.freedesktop.Platform//25.08
     ```

- **Expected:** the sandbox lists nine globals fewer than step 2 counts. It lists none of
  `zwlr_layer_shell_v1`, `zwlr_foreign_toplevel_manager_v1`,
  `xfce_foreign_toplevel_manager_private_v1`, `ext_foreign_toplevel_list_v1`,
  `ext_workspace_manager_v1`, `zwlr_output_manager_v1`, `ext_data_control_manager_v1`,
  `zwlr_data_control_manager_v1` and `wp_security_context_manager_v1`, and it does list
  `xdg_wm_base` and `wl_data_device_manager` (68, 75).
- **Debug tool:** `wayland-info` in and out of the sandbox.

## The panel

### The window list shows the active workspace's windows

- **Milestone:** M6
- **Steps:**
  1. Log into *MATE on Camino*, with `mate-panel-camino` at `pkgrel` 2 or later, and open two
     windows on the first workspace.
  2. Switch to the second workspace with the workspace switcher, look at the window list, and
     switch back.
  3. Move one of the windows to the second workspace from its window menu, and look at the
     window list on both workspaces.
  4. In a terminal, find the window list's settings and show every workspace's windows:

     ```sh
     path=$(dconf list /org/mate/panel/objects/ | grep window-list)
     gsettings set org.mate.panel.applet.window-list:/org/mate/panel/objects/${path}prefs/ \
       display-all-workspaces true
     ```

  5. Set it back to `false`, then open the window list's *Preferences* and choose *Show windows
     from all workspaces* there.
  6. `journalctl --user -b | grep 'Window<->Workspace'`.
- **Expected:** on the second workspace the window list has no button in step 2, and both
  windows' buttons are there again on the first. In step 3 each workspace's list has the button
  of its one window. In step 4 both buttons are there on every workspace, and in step 5 they go
  and come back; the two choices in *Preferences* can be clicked. Step 6 prints nothing:
  libxfce4windowing logs that line only where it can't learn a window's workspace (75).
- **Debug tool:** the window list, and `WAYLAND_DEBUG=1 mate-panel --replace 2>&1 | grep
  xfce_foreign_toplevel` for the `workspace_enter` and `workspace_leave` events.

## Input

### Switching keyboard layouts in native and X11 apps

- **Milestone:** M4
- **Steps:**
  1. Run `tools/dev-nested`, then start `gtk3-demo` (or any GTK3 text field) and `xterm` in it.
  2. On the nested bus, run:

     ```sh
     gsettings set org.mate.peripherals-keyboard-xkb.kbd layouts "['us', 'es']"
     gsettings set org.mate.peripherals-keyboard-xkb.kbd options "['grp\tgrp:alt_shift_toggle']"
     ```

  3. In each app, type `;`, press Alt+Shift, type `;` again, then press Alt+Shift to go back.
- **Expected:** both apps type `;` with the US layout and `ñ` with the Spanish one, and switch
  back. The switch applies to every window at once (54).
- **Debug tool:** the typed text, and `xkbcomp "$DISPLAY" - | grep xkb_symbols` on the nested
  `DISPLAY`.

## Outputs

### The scale picked for each of the developer's monitors

- **Milestone:** M4
- **Steps:**
  1. With both monitors connected, run `camino` on the tty.
  2. Start a terminal on it (`foot`), and move it to each monitor.
  3. Run `wlr-randr`, and
     `busctl --user call org.mate.Camino.Debug /org/mate/Camino/Debug org.mate.Camino.Debug DumpState`.
  4. `gsettings set org.mate.interface window-scaling-factor 2`, read the scales again, then
     set it back to 0 and read them once more.
- **Expected:** each monitor is in its preferred mode at the scale its rule picks (50): the AOC
  (DP-1, 1920×1080) at 1.0 and the LG (HDMI-A-1, 3840×2160) at 1.7. They sit left to right in
  the order they were found, with the second one's `x` the first one's logical width, and the
  pointer crosses between them where they touch. The terminal's text is the same physical size
  on both, and sharp on both. `window-scaling-factor` 2 puts both at 2 and moves the second
  output to the first one's new right edge, and 0 brings the defaults back.
- **Debug tool:** `wlr-randr`, the `outputs` of `DumpState`, and `edid-decode
  /sys/class/drm/card*-*/edid` for the sizes the rule read.

### Hot-plugging a monitor

- **Milestone:** M4
- **Steps:**
  1. With both monitors connected, run `camino` on the tty, and put a GTK3 window (pluma) and
     an X11 one (xterm) on the right-hand monitor. Maximise one of them.
  2. Unplug the right-hand monitor's cable. Run `wlr-randr` and `DumpState`.
  3. Plug it back in, and run them again.
  4. Do the same with the left-hand monitor.
- **Expected:** unplugging removes the output: `wlr-randr` and `DumpState` list one output, at
  0,0, and the windows that were on the unplugged monitor are on the other one, with the
  maximised one filling it. They're moved, not resized, so one larger than the output hangs
  over its edge. The monitor that stays never goes black or freezes. Plugging the
  monitor back lights it in its preferred mode at its default scale, to the right of the other
  one, whichever side it was on before, unless a layout is saved for the two (83), and the pointer and windows can go to it again. The log
  has no `can't light` line.
- **Debug tool:** `wlr-randr`, the `outputs` and windows of `DumpState`, and `~/camino.log`.

### The primary output moves the panel, the desktop icons and RandR's primary

- **Milestone:** M6
- **Steps:**
  1. With both monitors connected, and `mate-panel-camino` at `pkgrel` 3 or later and
     `caja-camino` installed, log into *MATE on Camino*. Read the primary output:

     ```sh
     primary() {
       gdbus call --session --dest org.mate.Camino --object-path /org/mate/Camino \
         --method org.freedesktop.DBus.Properties.Get org.mate.Camino PrimaryOutput
     }
     choose() {
       gdbus call --session --dest org.mate.Camino --object-path /org/mate/Camino \
         --method org.freedesktop.DBus.Properties.Set org.mate.Camino PrimaryOutput "<'$1'>"
     }
     primary; xrandr | grep ' connected'
     ```

  2. Choose the other output, by its name in `wlr-randr`, such as `choose HDMI-A-1`, and run
     the last line again.
  3. With the pointer on the output that isn't primary and no window open, start pluma.
  4. Unplug the primary output's cable, look, and plug it back in.
  5. `choose VGA-9`.
- **Expected:** with no choice made, the primary output is the left-hand monitor, or a
  laptop's own panel, and the panels and the desktop icons are on it. In step 2 they move to
  the output chosen, each at that output's size, the wallpaper stays on both, and `xrandr`
  marks the output chosen as `primary` both times. Pluma opens on the primary output. In step
  4 the panels and the icons go to the output that's left, and come back with the monitor.
  Step 5 is refused with `isn't an enabled output`, and changes nothing (81).
- **Debug tool:** `primary_output` and each output's `layer_surfaces` in `DumpState`,
  `wlr-randr`, `xrandr`, and `gdbus monitor --session --dest org.mate.Camino` for the property's
  signal.

### Turning an output off leaves the others touching

- **Milestone:** M6
- **Steps:**
  1. With three monitors connected, run `camino` on the tty, and run `wlr-randr`.
  2. `wlr-randr --output A --off`, with A the middle one, and run `wlr-randr` again.
  3. `wlr-randr --output A --on`, then `wlr-randr --output B --pos 5000,300`, with B another
     one, and run `wlr-randr` again.
  4. With two monitors only: `wlr-randr --output A --scale 1`, with A the left one at a scale
     above 1.
- **Expected:** after step 2 the two outputs left touch, with the leftmost at 0,0, and the
  pointer crosses between them. After step 3 A is back, right of the others, and B is against
  the others, not at 5000,300, with no gap and nothing overlapping. After step 4 the right
  output's `x` is the left one's new logical width (82).
- **Debug tool:** `wlr-randr`, and the `outputs` of `DumpState`.

### A GTK3 window dragged across a 1.0 and a 2.0 output

- **Milestone:** M4
- **Steps:**
  1. With both monitors connected, run `camino` on the tty. Set the LG to 2.0, so the AOC stays
     at 1.0: `wlr-randr --output HDMI-A-1 --scale 2`.
  2. Start pluma on the 1.0 output, type a line of text, and run `ToggleFrameDebug`, as in *The
     debug interface answers on the nested bus*.
  3. Drag pluma slowly by its titlebar onto the 2.0 output, stop while it spans both, go on
     until it's wholly on the 2.0 output, then drag it back the same way.
  4. Do the same with an X11 window (xterm).
- **Expected:** pluma's content and its frame stay the same logical size all the way across, so
  the window never jumps, doubles or halves. While the window spans both outputs, its frame is
  drawn at ×2 and is sharp on both. GTK3 gets no preferred scale (52): it draws at the largest
  scale of the outputs it has entered. So pluma's content switches to scale 2 as soon as the
  window enters the 2.0 output, and back to scale 1 only once it has wholly left it. Wholly on
  either output, the title, the button icons and pluma's text are as sharp as
  that output's other windows, and frame debug's label is that output's scale. xterm is drawn
  at the X11 scale, 2, on both outputs (41), and its frame behaves as pluma's does.
- **Debug tool:** frame debug's `×<n>` label, `strip_scale` in `DumpState`, and
  `WAYLAND_DEBUG=1 pluma 2>&1 | grep -E 'wl_surface.*(enter|leave|set_buffer_scale)'` for the
  outputs pluma is told of and the scale it draws at.

## camino-displays

### The page with MATE's theme, at 100% and at a fractional scale

- **Milestone:** M6
- **Steps:**
  1. Run `tools/dev-nested --bin`, and in another terminal, with `WAYLAND_DISPLAY` set to the
     nested socket, `target/debug/camino-displays --demo`. The demo has three made-up monitors
     and changes nothing outside its window.
  2. Drag a display, choose each resolution of one display and then each of its refresh
     rates, change a scale, turn a display off, press Apply, and let the countdown run out
     once and press Keep once.
  3. Run both again with `tools/dev-nested --bin --scale 1.5`.
  4. In the session, open it from the control center with Menta, with BlackMATE and with a
     theme that has a `gtk-4.0` directory, such as Breeze.
- **Expected:** the page is the prototype's (84): the arrangement on top, a row with one entry
  per display, the selected display's settings below, and Identify, Reset and Apply at the
  bottom. After each resolution and each refresh rate the window is still up and shows the
  choice (85). At 1.5 the text and the tiles' borders are sharp. mate-themes ship no `gtk-4.0`, so
  with Menta and with BlackMATE the page is in GTK4's own light theme, with MATE's font, and
  with Breeze it's in Breeze (ADR 0014).
- **Debug tool:** `GTK_DEBUG=interactive`, and `gsettings get org.gnome.desktop.interface
  gtk-theme`, which msd mirrors from MATE's (ADR 0005).

### camino-displays applies, reverts and keeps a configuration

- **Milestone:** M6
- **Steps:**
  1. On the tty with two monitors, log into *MATE on Camino*, keep a copy of
     `~/.config/camino/layouts.json` if there is one, and run `wlr-randr`.
  2. Open *Displays* from the control center. Press Identify.
  3. Drag the second display to the other side of the first, set its scale to another step,
     and press Apply. Let the countdown run out, and run `wlr-randr`.
  4. Do the same again and press Revert, and once more and press Keep. Run `wlr-randr`, and
     read `layouts.json`.
  5. Change a resolution and a refresh rate, and keep them. Turn a display off, keep it, turn
     it on again, and keep it.
  6. Tick Primary on the display that isn't, and press Apply.
  7. Close the window during a countdown.
  8. With the window open and a scale changed but not applied, unplug the second monitor's
     cable, and plug it back in.
- **Expected:** in step 2 the entry opens camino-displays, not the Display capplet, and each
  monitor shows its own name for two seconds. Nothing changes until Apply. After step 3 and
  after Revert, `wlr-randr` lists the scales and places of step 1. After Keep it lists what
  the window showed, with no gap and no overlap, and `layouts.json` has them (83). In step 5
  each change is what `wlr-randr` lists, and the display that comes back is on the right of
  the other. In step 6 there's no countdown, the panel and the desktop icons move (81), and
  `layouts.json` has the new primary output. In step 7 the old configuration is back. In step
  8 the window shows one display, then two again, and the scale that wasn't applied is gone.
- **Debug tool:** `wlr-randr`, the `outputs` and `primary_output` of `DumpState`,
  `~/.config/camino/layouts.json`, and `camino-displays` run from a terminal for GTK's and
  its own warnings.

### The control center's Displays is camino-displays, under Camino only

- **Milestone:** M6
- **Steps:**
  1. In the session, run `tr '\0' '\n' </proc/"$(pgrep -o -x mate-session)"/environ | grep
     '^XDG_DATA_DIRS='`, and `systemctl --user show-environment | grep XDG_DATA_DIRS`.
  2. Open the control center and the panel's *System ▸ Preferences ▸ Hardware* menu, and open
     *Displays* from each.
  3. Log out, log into *MATE*, the X11 session, and open *Displays* there.
- **Expected:** mate-session's `XDG_DATA_DIRS` starts with `/usr/share/camino/xdg:`, and
  systemd's, if it has one, doesn't name it. Each place has one *Displays*, which opens
  camino-displays. In X11 MATE it opens the Display capplet (84).
- **Debug tool:** `gtk-launch mate-display-properties`, and `ls
  /usr/share/camino/xdg/applications`.
