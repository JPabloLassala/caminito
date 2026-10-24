# Manual checklist

The checks `tools/check` can't run: anything that needs a real desktop, real logind, real hardware, msd or
Orca. Before a milestone closes, run every item tagged with that milestone, and every item from
an earlier milestone that still applies, on the installed package. Record the results in the
milestone's `Close M<n> <name>` commit message.

Each item has:

- **Milestone:** the milestone that introduces it, `M1` to `M9`. An item that needs something from
  a later milestone (msd, logind, Orca, two monitors) is tagged with that milestone, not the one
  that wrote the code.
- **Steps:** what to do.
- **Expected:** what counts as passing.
- **Debug tool:** what shows the result.

Several items read the nested Caminito's environment. With it running under `tools/dev-nested`:

```sh
nested_env() {
  tr '\0' '\n' </proc/"$(pgrep -n -x caminito)"/environ |
    grep -E '^(DBUS_SESSION_BUS_ADDRESS|XDG_CONFIG_HOME)='
}
```

The items on the tty run bare `caminito` as `caminitotest`, the second user (14). Log in as
`caminitotest` on a spare tty and run `caminito 2>~/caminito.log`. Run the commands of each item from a
second login of `caminitotest` (another tty, or `ssh`), which shares the first one's session bus,
with `WAYLAND_DISPLAY` set to the socket in the log's `listening on WAYLAND_DISPLAY=` line, and
`DISPLAY` to the one in its `Xwayland is ready on DISPLAY=` line for X11 apps. Ctrl+Alt+F<n>
switches between the ttys.

The items in the session run `caminito --session` instead, the same way or from the display
manager, and their commands run in a terminal inside the session or from a second login. A
second login gets the session's displays from systemd, where Caminito exports them, as in
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
- **Expected:** Caminito's `DBUS_SESSION_BUS_ADDRESS` is `dbus-run-session`'s own bus, not the
  host's (`unix:path=/run/user/<uid>/bus`).
- **Debug tool:** `/proc/<pid>/environ`.

### dconf goes to the scratch config

- **Milestone:** M1
- **Steps:**
  1. Run `tools/dev-nested`. It prints its scratch `XDG_CONFIG_HOME`.
  2. In another terminal, write a key on the nested bus and read it back on both buses:

     ```sh
     env $(nested_env) sh -c 'dconf write /org/caminito/checklist true; dconf read /org/caminito/checklist; ls "$XDG_CONFIG_HOME/dconf"'
     dconf read /org/caminito/checklist
     ```

  3. Quit Caminito.
- **Expected:** the nested read prints `true` and `$XDG_CONFIG_HOME/dconf/user` exists in the
  scratch directory. The host read prints nothing. The scratch directory is gone after Caminito
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
     (`WAYLAND_DISPLAY=<caminito's socket> GDK_BACKEND=wayland pluma`).
  2. With `DBUS_SESSION_BUS_ADDRESS` from `nested_env`, run
     `busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug org.mate.Caminito.Debug DumpState`.
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
       env $(nested_env) busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug \
         org.mate.Caminito.Debug DumpState
     done
     ```

- **Expected:** each frame changes to the new theme as soon as it's set, with no app restarted
  and no window unmapped. The unmaximised window's client area keeps its size, the maximised
  one and its frame still fill the output, and `frame_extents` follow the theme. `NoSuchTheme` gives the built-in
  theme and one `can't load the Marco theme NoSuchTheme` line in Caminito's log. Back on Menta,
  the frames are Menta's again, and the buttons still prelight, press and act.
- **Debug tool:** `busctl` and Caminito's stderr.

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
  1. Log into *MATE on Caminito*. Before `caminito --session`, nothing Caminito spawns receives its
     environment, so in M3 the X11 tier's test that `DISPLAY` isn't set until the root properties
     exist (38) stands in.
  2. Read the `DISPLAY` Caminito gave mate-session, the first process it spawns with its
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

### Logging into MATE on Caminito

- **Milestone:** M5
- **Steps:**
  1. On a tty: log in as `caminitotest` on a spare tty and run `caminito --session 2>~/session.log`.
  2. From the display manager: log out of every session of the user first, since its logins
     share one systemd manager and bus (14). At the greeter, choose *MATE on Caminito* in the
     session list and log in.
  3. Each time, look at the desktop, then run:

     ```sh
     systemctl --user show-environment | grep -E '^(WAYLAND_DISPLAY|DISPLAY|MATE_WM_NAME)='
     loginctl show-session "$XDG_SESSION_ID" -p Type -p Desktop -p TTY
     pgrep -a -u "$USER" -x 'caminito|mate-session|mate-panel|caja|mate-settings-d' | sort -k2
     journalctl --user -b -t caminito -p warning
     grep -aiE 'timed out|time-?out' ~/session.log
     ```

- **Expected:** the greeter lists *MATE on Caminito*. Both ways the desktop is up well inside 30
  seconds, with no mate-session phase timing out (08): both panels, with the window list and the
  workspace switcher, caja's desktop icons, and msd's wallpaper under them on every output.
  systemd has `WAYLAND_DISPLAY`, `DISPLAY` and `MATE_WM_NAME=Caminito`, and the five processes
  run. From the display manager the logind session's type is `wayland`. Caminito's stderr isn't a
  terminal either way, so its lines are in the journal. mate-session and what it starts write to
  that stderr itself (72): `~/session.log` on the tty, and the display manager's log of the
  session otherwise, where the `grep` goes then. Neither has a time-out.
- **Debug tool:** `systemctl --user show-environment`, `loginctl`, the journal, and the
  `outputs` of `DumpState` for the panels' and the desktop's layer surfaces.

### A link opens in the browser after a second login

- **Milestone:** M5
- **Steps:**
  1. Log into *MATE on Caminito*, log out, and log in again, with the user's systemd manager
     left running between the two: a second login of the user elsewhere, or lingering.
  2. Run:

     ```sh
     pid=$(systemctl --user show -p MainPID --value xdg-desktop-portal.service)
     tr '\0' '\n' </proc/$pid/environ | grep -E '^(WAYLAND_DISPLAY|DISPLAY)='
     ```

  3. Open *About Caminito* and press one of its links, with Firefox closed and again with it
     open.
- **Expected:** in step 2 the portal has both variables, with this Session's values, or has no
  process yet. In step 3 the link opens in the browser each time (232). The journal has no
  `no DISPLAY environment variable specified` from the browser, and no `can't restart the
  portal` from Caminito.
- **Debug tool:** `journalctl --user -b -t caminito`, `journalctl --user -u xdg-desktop-portal`,
  and `systemctl --user restart xdg-desktop-portal.service`, which is what Caminito asks for.

### Log Out and Shut Down from the panel

- **Milestone:** M5
- **Steps:**
  1. As `caminitotest` on a tty, run `caminito --session 2>~/session.log; echo "caminito exited with $?"`.
  2. In the panel, choose *System*, *Shut Down…*, look at the dialog, and cancel it.
  3. Choose *System*, *Log Out caminitotest…*, then *Log Out*.
  4. From a second login, run `systemctl --user show-environment` and `pgrep -a -u caminitotest`.
  5. Log in as yourself, from the display manager, choose *Shut Down…* and then *Shut Down*.
     After the machine is back, read `journalctl --user -b -1 -t caminito | tail`.
- **Expected:** *Shut Down…* shows mate-session's dialog (*Shut down this system now?*, with its
  countdown), an XWayland window that Caminito frames, not the panel's own *System Shutdown* (70).
  Cancelling it leaves the session as it was. *Log Out* ends the session: the tty is back in text
  mode at the shell prompt, which prints `caminito exited with 0`, mate-session's status (61).
  Nothing of the session still runs, and of its variables systemd has only
  `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` (61, 74). *Shut Down* powers the machine off: the
  journal of that boot has logind's `requested from client PID … ('mate-session')`, and
  Caminito's last lines are `stopping the session command` or `the session command exited`,
  since mate-session asks logind before it exits and the machine may go first. It may also
  have `systemd didn't unset the environment`, since a systemd that's shutting down may not
  answer (61).
- **Debug tool:** the exit status, `systemctl --user show-environment`, the journal, and
  `dbus-monitor "interface='org.gnome.SessionManager'"` for the panel's `Logout` and `Shutdown`
  calls.

### An X11 app's clipboard contents survive the app quitting

- **Milestone:** M5
- **Steps:**
  1. Log into *MATE on Caminito*, with no pluma running: pluma hands a new window to the running
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

### Firefox's page content is dark with a dark GTK theme

- **Milestone:** M5
- **Steps:**
  1. Log into *MATE on Caminito*. In *Appearance*, choose BlackMATE.
  2. In Firefox, open a page that follows the colour scheme, such as
     `data:text/html,<style>@media (prefers-color-scheme: dark) { body { background: black; color: white } }</style>dark?`,
     and, in a terminal, run `gsettings get org.gnome.desktop.interface color-scheme`.
  3. With Firefox open, choose Menta, and look at the page and the key again.
- **Expected:** with BlackMATE the page is white on black, Firefox's own toolbars and
  `about:preferences` are dark, and the key is `prefer-dark`. With Menta the page goes black on
  white within a second, with Firefox not restarted, and the key is `default` (180).
- **Debug tool:** the page, `gsettings`, and `gdbus call --session --dest
  org.freedesktop.portal.Desktop --object-path /org/freedesktop/portal/desktop --method
  org.freedesktop.portal.Settings.ReadOne org.freedesktop.appearance color-scheme`, which gives
  `1` for dark and `0` for none.

### X11 MATE and the Wayfire session are unchanged with the Patched packages

- **Milestone:** M5
- **Steps:**
  1. As the developer's own user, with the four Patched packages installed
     (`pacman -Q mate-session-manager mate-panel mate-settings-daemon caja` names the `-caminito`
     ones),
     log into *MATE*, the X11 session, from the display manager.
  2. Run:

     ```sh
     echo "$XDG_SESSION_TYPE ${MATE_WM_NAME:-unset} ${GDK_BACKEND:-unset}"
     pgrep -a -x marco
     busctl --user status org.mate.Caminito
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
     wallpaper, and leave it with Super+Escape (`wlogout`), or `pkill wayfire` from a terminal.
- **Expected:** the session is as it was with the stock packages (the fall-back rule, ADR 0011).
  It's `x11`, with `MATE_WM_NAME` not Caminito's and `GDK_BACKEND` as before, mate-session starts
  Marco, and nothing owns `org.mate.Caminito`. msd serves XSETTINGS, so open apps restyle at the
  theme change, and it doesn't mirror into `org.gnome.desktop.interface`, which keeps its value.
  The panel's buttons, switcher and dialogs work, and no applet is missing from the panel: the
  loop prints nothing, since no installed applet is linked to an applet library that's gone:
  `mate-panel-caminito` installs the 1.28 library beside master's (70, 73). An applet of another
  package that was on the panel shows and works, unless it's one the panel loads into its own
  process (`InProcess=true`, as `mate-notification-daemon`'s), where the two libraries can't
  work together. The Wayfire session logs in, with its panel, its desktop icons and its
  wallpaper where they were with the stock packages (81). The panel's *Log Out* does nothing
  there, with the stock panel too: it asks the session manager, and that session has no
  mate-session.
- **Debug tool:** `ldd` on the applets, `busctl`, `gsettings`, and `~/.xsession-errors` or the
  journal for what a missing applet logged.

### Caminito restores the saved layout from the first frame

- **Milestone:** M6
- **Steps:**
  1. With `mate-settings-daemon-caminito` at `pkgrel` 3 or later, log into *MATE on Caminito* with
     both monitors connected, and note `wlr-randr`'s scales and positions, which are Caminito's
     defaults (50) unless a layout is saved already. Keep a copy of
     `~/.config/caminito/layouts.json` if there is one.
  2. Apply a layout that isn't the default, choose the other output as the primary one, and
     save it:

     ```sh
     wlr-randr --output DP-1 --scale 1.25 --pos 0,0 --output HDMI-A-1 --pos 1536,0
     gdbus call --session --dest org.mate.Caminito --object-path /org/mate/Caminito \
       --method org.freedesktop.DBus.Properties.Set org.mate.Caminito PrimaryOutput "<'HDMI-A-1'>"
     gdbus call --session --dest org.mate.Caminito --object-path /org/mate/Caminito \
       --method org.mate.Caminito.SaveLayout
     ```

  3. Apply another scale with `wlr-randr`, and don't save it. Log out and in again, watching
     the monitors as the session starts, and run `wlr-randr` and `DumpState`.
  4. Unplug the second monitor's cable, run `wlr-randr`, plug it back in, and run it again,
     and `DumpState`.
  5. Run `pgrep -a mate-settings-daemon` and `gdbus introspect --session --dest
     org.mate.SettingsDaemon --object-path /org/mate/SettingsDaemon/WLRANDR`.
  6. Remove `layouts.json`, log out and in once more, and then put the copy back.
- **Expected:** after step 3 each output has the mode, scale and place of step 2, not Caminito's
  default and not the scale of step 3, and the panels are on the output chosen (81). The
  layout is there from the first frame: the outputs don't blank or jump once the session is
  up, as they did while msd applied it (06). In step 4 the monitor that stays is at the
  origin while it's alone, and the saved layout and the primary output are back when the
  other is plugged in, with one `mate-background` layer surface on each output, as before the unplug (88). In step 5 msd runs, and the object has no
  `org.mate.SettingsDaemon.WLRANDR` interface: the plugin isn't started under Caminito, and `~/.config/monitors-wayland.xml` is neither read nor written (ADR
  0014). After step 6 the defaults are back.
- **Debug tool:** `wlr-randr`, the `outputs` and `primary_output` of `DumpState`,
  `~/.config/caminito/layouts.json`, and `can't restore the saved layout` in `~/caminito.log`.

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

- **Milestone:** M9
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
  1. Log into *MATE on Caminito*, with `mate-panel-caminito` at `pkgrel` 2 or later, and open two
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

### The window list sends each button's rectangle

- **Milestone:** M6
- **Steps:**
  1. Log into *MATE on Caminito*, with a pluma window open.
  2. In a terminal, restart the panel with its Wayland requests printed:

     ```sh
     WAYLAND_DEBUG=1 mate-panel --replace 2>&1 | grep set_rectangle
     ```

  3. Open a second window, and then a third.
  4. Stop the command with Ctrl+C, and start the panel again from Alt+F2 (`mate-panel`) if the
     session hasn't.
- **Expected:** step 2 prints a `zwlr_foreign_toplevel_handle_v1` `set_rectangle` request for
  pluma and one for the terminal, each with the panel's `wl_surface` and the place and size of
  that window's button in the window list (06). Step 3 prints one for each new window, and again
  for the others when their buttons move or shrink. No request is a protocol error: the panel
  stays up.
- **Debug tool:** the requests are the check. Caminito keeps the rectangle with the window, and
  nothing reads it until M8's minimise animation, so the debug dump has no member for it.

### Alt+F1 opens the main menu, and Alt+F2 the Run dialog

- **Milestone:** M6
- **Steps:**
  1. Log into *MATE on Caminito*, with `mate-panel-caminito` at `pkgrel` 8 or later, and run
     `busctl --user status org.mate.Panel | grep -E '^(PID|Comm)='`.
  2. With a window focused and the pointer over it, press Alt+F1, move through the menu and
     into a submenu with the arrow keys, and press Escape until it has closed. Type in the
     window.
  3. Press Alt+F1 again, move the pointer along the Menu Bar from *Applications* to
     *Places* and into a submenu there, and click outside the menu.
  4. Right-click in the window for its context menu, and press Alt+F1 with it open.
  5. Press Alt+F2, type `mate-calc` and press Enter.
  6. Remove the Menu Bar from the panel and add a *Main Menu* button in its place. Press
     Alt+F1. Remove the button too, and press Alt+F1 with the pointer over the panel, and
     then with it over a window. Put the Menu Bar back.
  7. In a terminal, stop the panel answering, press Alt+F2 and type in the window, and then
     let the panel go on:

     ```sh
     pkill -STOP -x mate-panel; sleep 10; pkill -CONT -x mate-panel
     ```

- **Expected:** in step 1 the name's owner is `mate-panel`. In step 2 the menu drops from the
  Menu Bar's *Applications*, and it has the keyboard: the arrow keys move through it, a
  submenu opens, and Escape closes it one level at a time, after which the window has the
  keyboard back. In step 3 *Places* and its submenu open under the pointer, and the click
  closes the menu. In step 4 the context menu stays, no menu of the panel's opens, and the
  panel is still there. In step 5 the Run dialog opens with the keyboard in its entry, and
  the calculator starts. In step 6 the menu drops from the button, and without one it opens
  at the pointer over the panel, and from the first panel's corner with the pointer over a
  window, each time with the keyboard. In step 7 nothing opens while the panel is stopped,
  and Caminito neither hangs nor loses a key: the window gets what's typed. The Run dialog
  opens once the panel goes on (95).
- **Debug tool:** `busctl --user status org.mate.Panel`, and
  `busctl --user call org.mate.Panel /org/mate/Panel org.mate.Panel RunDialog u 0`, which is
  Caminito's call without the key. `can't open the panel's main menu` in `~/caminito.log` says
  the name had no owner or the call failed.

### The Show Desktop button and Ctrl+Alt+D show the same desktop

- **Milestone:** M6
- **Steps:**
  1. Log into *MATE on Caminito*, with `mate-panel-caminito` at `pkgrel` 7 or later, and open
     three windows on the first workspace, one of them an X11 app such as `xterm`. In a
     terminal on the second workspace, follow the property:

     ```sh
     gdbus monitor --session --dest org.mate.Caminito --object-path /org/mate/Caminito
     ```

  2. On the first workspace, click the panel's Show Desktop button, then press Ctrl+Alt+D.
  3. Press Ctrl+Alt+D, then click the button.
  4. Click the button, switch to the second workspace and back with the workspace switcher.
  5. With the desktop still shown, click one window's button in the window list.
  6. Show the desktop again with either, and start pluma from the panel's menu.
- **Expected:** in steps 2 and 3 the windows go and come back as they were stacked, with the
  focus on the one that had it, whichever of the two hid them and whichever brought them back,
  and the button is pressed in exactly while they're hidden. The window list keeps their
  buttons, none drawn as minimised. In step 4 the button is out on the second workspace, whose
  windows stay, and in again back on the first. In step 5 that window comes back alone, and in
  step 6 pluma opens alone: the button comes out, and the other windows are minimised in the
  window list, as in Marco. `gdbus monitor` prints one `ShowingDesktop` change for
  each, and none for a switch between workspaces that agree (94).
- **Debug tool:** `showing_desktop` in `DumpState`, the `gdbus monitor` of step 1, and to tell
  the button's side from Caminito's:

  ```sh
  gdbus call --session --dest org.mate.Caminito --object-path /org/mate/Caminito \
    --method org.freedesktop.DBus.Properties.Set org.mate.Caminito ShowingDesktop '<true>'
  ```

### The workspace switcher's preferences set the count and the names

- **Milestone:** M6
- **Steps:**
  1. Log into *MATE on Caminito*, with `mate-panel-caminito` at `pkgrel` 5 or later, and four
     workspaces (`gsettings get org.mate.Marco.general num-workspaces`).
  2. Open the workspace switcher's *Preferences*, and set *Number of workspaces* to 6, then
     to 2. After each, look at the switcher and run the `gsettings get` of step 1.
  3. Tick *Show workspace names in switcher*. In *Workspace names*, change the second name to
     `Mail` and press Enter, then run
     `gsettings get org.mate.Marco.workspace-names name-2`.
  4. Empty that name again, and set the count back to 4.
  5. Log into *MATE*, the X11 session, and do steps 2 to 4 there. Then log into *MATE on
     Wayfire* and open the switcher's *Preferences*.
- **Expected:** under Caminito the preferences have *Number of workspaces* and *Workspace
  names*. The switcher, which draws what Caminito sends, has six buttons, then two, `gsettings`
  prints the same count, and the spin button stays at the number that was set. The second
  button and the second row say `Mail`, and `gsettings` prints `'Mail'`. Emptied, both say
  `Workspace 2` again. On X11 the same steps do the same through Marco, as with the stock
  panel, and on Wayfire the preferences have neither control (the gate, ADR 0011).
- **Debug tool:** the switcher, `gsettings monitor org.mate.Marco.general`, and
  `WAYLAND_DEBUG=1 mate-panel --replace 2>&1 | grep ext_workspace` for the handles and names
  Caminito sends.

### The workspace switcher's rows are Caminito's grid

- **Milestone:** M6
- **Steps:**
  1. Log into *MATE on Caminito*, with `mate-panel-caminito` at `pkgrel` 6 or later and four
     workspaces, the switcher on a horizontal panel, and note
     `gsettings list-recursively org.mate.Caminito`.
  2. Open the workspace switcher's *Preferences*, set the rows to 2, and read the keys again.
  3. On the first workspace, press Ctrl+Alt+Right twice, then click the third workspace in the
     switcher and press Ctrl+Alt+Left.
  4. `gsettings set org.mate.Marco.general num-workspaces 6`, look at the switcher, and set
     it back to 4.
  5. Set the rows back to 1 in *Preferences*.
  6. Log into X11 MATE, set the switcher's rows to 2 and back, and read
     `gsettings list-recursively org.mate.Caminito` before and after.
- **Expected:** in step 1 the switcher is one row, and the keys are `horizontal`, 1 and 0. In
  step 2 it shows two rows of two, with the first and second workspaces above the third and
  fourth, and `workspace-layout-rows` is 2. In step 3 Caminito stops at the second workspace, the
  end of the first row, and stays on the third, the start of the second. In step 4 the switcher
  is two rows of three. In step 5 it's one row again. In step 6 the switcher's rows change as
  they always did on X11, and Caminito's keys don't (ADR 0011).
- **Debug tool:** `gsettings monitor org.mate.Caminito`, and `WAYLAND_DEBUG=1 mate-panel --replace
  2>&1 | grep coordinates` for the `ext_workspace_handle_v1.coordinates` events.

### Dynamic workspaces grow and shrink the switcher

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito*, with no window open and the workspace switcher on the panel.
     Run `gsettings range org.mate.Marco.general dynamic-workspaces`. If it prints `No such
     key`, the installed Marco is 1.29.0 or older, which has no key for it: skip the item and
     say so in the closing run's results.
  2. `gsettings set org.mate.Marco.general dynamic-workspaces true`, and run
     `gsettings get org.mate.Marco.general num-workspaces`.
  3. Open a terminal, then a second one on the second workspace, by clicking it in the
     switcher first. Run the `gsettings get` of step 2 after each.
  4. Close the terminal on the first workspace, then the other one.
  5. `gsettings set org.mate.Marco.general dynamic-workspaces false`, open and close a
     terminal, and set `num-workspaces` back to 4.
- **Expected:** in step 2 the switcher shrinks to one workspace and `num-workspaces` is 1. In
  step 3 it grows by one after each terminal, to three, the last always empty, and
  `num-workspaces` follows. In step 4 the second terminal moves to the first workspace with
  the view and the focus, the switcher has two workspaces, then one. In step 5 the count
  stays as it is while windows come and go, then goes back to 4.
- **Debug tool:** `gsettings monitor org.mate.Marco.general`, and `WAYLAND_DEBUG=1 mate-panel
  --replace 2>&1 | grep ext_workspace` for the workspaces Caminito adds and removes.

### The four directions in a 2 × 2 grid, with each wrap style

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito* with four workspaces, and set the workspace switcher's rows
     to 2 in its *Preferences*, so that the first and second workspaces are above the third
     and fourth.
  2. With `gsettings set org.mate.Marco.general wrap-style 'no wrap'`, on the first
     workspace, press Ctrl+Alt+Up, Ctrl+Alt+Left, then Ctrl+Alt+Right, Down, Left and Up.
  3. `gsettings set org.mate.Marco.general wrap-style classic`, then on the first workspace
     press Ctrl+Alt+Up, and on the first again Ctrl+Alt+Left; on the fourth, Ctrl+Alt+Right,
     and on the fourth again Ctrl+Alt+Down.
  4. `gsettings set org.mate.Marco.general wrap-style toroidal`, and press the keys of step 3
     from the same workspaces.
  5. Open a terminal on the first workspace and press Ctrl+Shift+Alt+Down, then
     Ctrl+Shift+Alt+Up.
  6. `gsettings set org.mate.Marco.global-keybindings switch-to-workspace-prev
     '<Super>p'`, click the fourth workspace in the switcher, then press Super+P twice.
  7. Reset `wrap-style` and `switch-to-workspace-prev`, and set the rows back to 1.
- **Expected:** in step 2 the first two keys do nothing, and the four after go to the second,
  fourth, third and first workspaces. In step 3 classic goes to the fourth, the fourth, the
  first and the first. In step 4 toroidal goes to the third, the second, the third and the
  second. In step 5 the terminal goes to the third workspace and back, the view with it, and it
  keeps the focus. In step 6 Super+P goes back to the workspace the click left, then to the
  fourth again. The switcher shows each switch as it happens.
- **Debug tool:** `gsettings monitor org.mate.Marco.general`, and the debug dump's `bindings`
  for the combinations in force.

### The Workspace popup shows the grid while Ctrl+Alt is held

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito* with four workspaces and two monitors. Set the workspace
     switcher's rows to 2 in its *Preferences*, and name the workspaces there, the last one
     with a name longer than the others. Open a terminal on the first workspace of the
     first monitor.
  2. With the terminal focused, hold Ctrl+Alt and press Right, then Down, Left and Up,
     slowly, and release Ctrl+Alt.
  3. Hold Ctrl+Alt, press Left, and release Alt alone.
  4. Hold Ctrl+Alt, press Right, and press A. Hold Ctrl+Alt, press Left, and click a window.
  5. Tap Ctrl+Alt+Right, then Ctrl+Alt+Left, quickly.
  6. With the terminal focused, hold Ctrl+Shift+Alt and press Right, then Down, and release.
  7. Open a terminal on the second monitor, move the pointer back to the first monitor and
     do step 2 again. Run `gsettings set org.mate.Caminito workspaces-per-output false`, do
     step 2 once more, and set the key back to `true`.
  8. Hold Alt, start to move a window with the left button, and press Ctrl+Right with them
     held. Lock the screen and press Ctrl+Alt+Right.
  9. Set the rows back to 1. Run as part of M8's closing run (173).
- **Expected:** in step 2 the popup appears at the first press, in the middle of the first
  monitor, in the GTK theme's colours and font: four cells in two rows, each a small picture
  of its workspace with its name under it, the long name ending in an ellipsis, and the
  terminal outlined in the first. The cell of the workspace switched to is marked at each
  press, and the workspace behind the popup changes with it. The popup goes at the release.
  In step 3 the popup shows the first workspace marked, though the switch went nowhere, and
  goes when Alt is released. In step 4 the A closes the popup and types nothing, and the
  click closes it and doesn't reach the window. In step 5 the workspace changes and back, and
  the popup at most flashes. In step 6 the terminal goes to the second workspace and then
  the fourth, its outline with it, and keeps the focus. In step 7 the popup is on the second
  monitor, the Active output, and shows that monitor's workspaces and windows; with the key
  off it is on the first monitor, where the pointer is (231). In step 8 the workspace changes under the move and no popup shows, and under the
  lock nothing happens (152).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, whose `workspace_popup` member has
  the popup's output, its mode, the grid's shape, the workspace marked and each cell's
  rectangles while it shows.

### The Workspace slide on one monitor

- **Milestone:** M9
- **Steps:**
  1. Log into *MATE on Caminito* on the tty backend, with one monitor, four workspaces in one
     row and the workspace switcher on the panel. Run `gsettings get org.mate.Caminito
     workspace-slide`, which prints `true`. Open a terminal on each of the first three
     workspaces, each at a different place and size, and maximise the third.
  2. On the first workspace press Ctrl+Alt+Right three times, with a second between, then
     Ctrl+Alt+Left three times. Type a letter as soon as each key is pressed.
  3. Click the third workspace in the switcher, then the first.
  4. Set the switcher's rows to 2 in its *Preferences*. From the first workspace press
     Ctrl+Alt+Down, then Ctrl+Alt+Up, then click the fourth workspace in the switcher, and
     the first. Set the rows back to 1.
  5. On the first workspace hold Ctrl+Alt+Right until the fourth workspace shows. Hold
     Ctrl+Alt+Left until the first shows. Then press Ctrl+Alt+Right and, before the slide
     ends, Ctrl+Alt+Left.
  6. `gsettings set org.mate.Marco.general wrap-style toroidal`, then on the fourth workspace
     press Ctrl+Alt+Right. Go to the fourth again and hold Ctrl+Alt+Right until the workspaces
     have gone round two times. Reset `wrap-style`.
  7. Focus the first workspace's terminal and press Ctrl+Shift+Alt+Right, then
     Ctrl+Shift+Alt+Left.
  8. Pick *Always on Visible Workspace* in that terminal's window menu, and press
     Ctrl+Alt+Right and Ctrl+Alt+Left. Pick *Only on This Workspace* after.
  9. Hold Alt, start to move the terminal with the left button, and press Ctrl+Right with
     them held. Release, and move the terminal back to the first workspace.
  10. `gsettings set org.mate.Caminito workspace-slide false`, and press Ctrl+Alt+Right and
      Ctrl+Alt+Left. Set it back to `true`, press Ctrl+Alt+Right and, before the slide ends,
      set it to `false` from a second login. Set it back to `true`.
  11. With `workspace-slide` on, turn each of these against the slide in turn, switch, and set
      it back: `org.mate.Marco.general compositing-manager false`, `org.mate.Marco.general
      reduced-resources true` and `org.mate.interface enable-animations false`.
  12. Lock the screen during a slide: run `sleep 3; mate-screensaver-command --lock` in a
      terminal, and press Ctrl+Alt+Right as the three seconds end. Unlock.
- **Expected:** in step 2 the old workspace's windows go out to the left as the new one's come
  in from the right, with their Frames and shadows and with no gap between the two, and the
  other way for Left. The panel, the desktop icons and the wallpaper stay still, and the
  maximised terminal comes in over them. The letter goes to the new workspace's terminal,
  which has the focus from the first frame. In step 3 the slide to the third workspace is one
  movement, two monitor widths long, and the second workspace's terminal isn't drawn on the
  way. In step 4 Down and Up slide down and up, and the click on the fourth is one diagonal
  line. In step 5 the movement is continuous across the workspaces, with no stop and no jump at
  each repeat of the key, and the workspaces passed over are drawn as they go by. The switch
  back returns the windows from where they are. In step 6 the wrap slides one cell to the
  left, in the key's direction, and not back across the grid. With the key held no window
  jumps from one side to the other: a workspace that comes back is drawn where it leaves and
  where it arrives (239). In step 7 the terminal stays under the eyes while the others move
  beneath it, and keeps the focus. In step 8 the sticky terminal stays still, the others
  sliding beneath it. In step 9 the terminal stays under the pointer through the slide, and the
  move goes on. In step 10 each switch is a cut with the key off, and the slide that runs when
  the key goes off finishes. In step 11 each switch is a cut. In step 12 the locker shows no
  window that slides, and after the unlock the windows are in their places.

  In every step the slide is smooth at the monitor's refresh rate, with no frame that stutters
  or tears. It takes a quarter of a second, starts fast and slows down into its place. After
  its end no frame shows a window of the old workspace, and no window is a pixel off its place.
  The developer judges if the length and the curve feel right. If not, the developer changes
  them and records it in the [spec](../../.scratch/workspace-slide/spec.md)'s "Timing" (237).
- **Debug tool:** `DumpState`'s `workspace_slides`, there while a slide runs: each has its
  `output`, `from`, `to` and `offset`, the `windows` it moves with their drawn rectangles, and
  the `still` ones. `wlr-randr` for the monitor's refresh rate. A phone's slow-motion video of
  the monitor shows a frame of the old workspace after the end.

### The Workspace slide on two monitors

- **Milestone:** M9
- **Steps:**
  1. Log into *MATE on Caminito* on the tty backend, with two monitors side by side and four
     workspaces in one row. Run `gsettings get org.mate.Caminito workspaces-per-output`,
     which prints `true`. Open a terminal on the first and second workspaces of each monitor,
     and a `pluma` on the first monitor's first workspace, moved to straddle the two
     monitors with its larger part on the first.
  2. Click a terminal on the first monitor, and press Ctrl+Alt+Right, then Ctrl+Alt+Left. Do
     the same after a click on a terminal on the second monitor.
  3. Click another workspace in the workspace switcher of each monitor's panel, if each has
     one.
  4. Click a terminal on the first monitor, and hold Ctrl+Alt+Right across three workspaces,
     and back.
  5. Focus a terminal on the first monitor and press Ctrl+Shift+Alt+Right, then
     Ctrl+Shift+Alt+Left.
  6. `gsettings set org.mate.Caminito workspaces-per-output false`, and press Ctrl+Alt+Right
     and Ctrl+Alt+Left. Set it back to `true`.
  7. If the monitors differ in size or scale, do step 6 again and watch each.
  8. `gsettings set org.mate.Marco.general dynamic-workspaces true`, open a terminal on the
     second workspace of the first monitor, and close it there. Set the key back to `false`
     and `num-workspaces` to 4.
- **Expected:** in step 2 only the monitor of the terminal that was clicked slides, wherever
  the pointer is, and the other is as it was, to the pixel. A sliding window is cut at the edge of its monitor and no part of it
  shows on the other. `pluma` slides on the first monitor alone: on the second its part isn't
  drawn as it leaves, and is at its place from the first frame as it arrives. When the second
  monitor switches, `pluma` stays at its place on both. In step 3 the pager's switch slides
  its own monitor. In step 4 the movement is continuous, on the first monitor alone. In step 5
  the terminal stays still and the others of its monitor move beneath it. In step 6 both
  monitors slide at the same time, and the workspace changes on both. In step 7 each slides by
  its own width, and both end together. In step 8 the first monitor slides to the first
  workspace when the terminal closes, and the second monitor doesn't move. In every step the
  slide is smooth on each monitor at that monitor's refresh rate, with no frame of the old
  workspace after its end (237).
- **Debug tool:** `DumpState`'s `workspace_slides`, with one slide for each monitor that
  switches. `wlr-randr` for the refresh rates.

### The Overview on one monitor

- **Milestone:** `overview`, the developer's to run (247).
- **Steps:**
  1. Log into *MATE on Caminito* on the tty backend, with one monitor, four workspaces in one
     row and the panel's window list showing. Run `gsettings get org.mate.Caminito overview`,
     which prints `'<Super>w'`, and `gsettings get org.mate.Caminito overview-animation`,
     which prints `true`. On the first workspace open a terminal, pluma, `xterm` (X11) and a
     video playing in Firefox or `mpv`, each at a different place and size, and minimise
     `xterm`. On the second workspace open `glxgears` or a second video.
  2. Run `gsettings list-recursively | grep -i -E '<(super|mod4)>w'` from the terminal. Open
     *System Shortcuts* in the control center and look for Super+W. Focus pluma and press
     Super+W, then press it again. Do the same with caja's desktop focused, with nothing
     focused, and with pluma fullscreen (F11).
  3. Open the Overview and close it with Esc. Open it and close it with a click on the empty
     area between the windows. Open it, press Alt+Tab and a volume key, and close it. Go to
     the empty fourth workspace, open it and close it, and go back to the first.
  4. Open it and click pluma. Open it and choose `xterm`, the minimised one, with a click.
     Minimise `xterm` again.
  5. Open it, move the selection with each arrow key, and press Enter. Open it with no window
     focused (click the desktop first) and press Right, then Enter.
  6. Open it, and middle-click `xterm`. Then run `sleep 2; mate-calc &` in the terminal, open
     the Overview at once, wait for `mate-calc` to open in it, and press Esc.
  7. Open it and look at the video in the layout and at the bar's cells for half a minute.
  8. Open it and click the second workspace's cell, then the first's. Press Ctrl+Alt+Right
     and Ctrl+Alt+Left. If a mouse button is bound to `switch-to-workspace-left` or `-right`,
     press it over a window, over a cell and over the empty area.
  9. Open it and drag the terminal onto the third workspace's cell, holding it over the second
     cell and then the third for a moment before the release. Drag pluma onto the first
     cell, the active one. Drag pluma well past the drag threshold and release it on the empty area. Start a
     drag of pluma and press Esc during it. Close the Overview, and go to the third
     workspace to see the terminal. Bring it back to the first.
  10. Press Super+W and, before the opening ends, press it again. Then open it fully and
      press Esc and Super+W one after the other, quickly.
  11. Press Ctrl+Alt+Right and, before the slide ends, Super+W.
  12. `gsettings set org.mate.Caminito overview-animation false`, and open and close it, and
      drag a window onto a cell. Set it back to `true`. With it on, turn each of these
      against the transition in turn, open and close, and set it back:
      `org.mate.Marco.general compositing-manager false`, `org.mate.Marco.general
      reduced-resources true` and `org.mate.interface enable-animations false`.
  13. Lock the screen with the Overview open: run `sleep 3; mate-screensaver-command --lock`
      and press Super+W at once. Unlock. Do it again, with a drag in progress as the lock
      comes.
  14. `gsettings set org.mate.Caminito overview ''`, and press Super+W. Reset the key.
- **Expected:** in step 2 the grep prints the `overview` key alone, *System Shortcuts* lists
  *Show the windows of the workspace side by side*, under *Window Management*, on Super+W and
  no other shortcut on it, and each press opens or closes
  the Overview and reaches no window: pluma types no `w`, and caja's desktop does nothing. The
  fullscreen pluma is laid out like any other window. In every step the Overview opens on the
  first workspace with each window moving and shrinking from its place into rows, none over
  another, in the order of their places, each with its icon and title beneath it. The
  minimised `xterm` fades in at its place in the layout. The bar fades in along the top, one cell for each
  workspace, with the first marked. The wallpaper dims, with no blur, and the panel and the
  desktop icons fade out. Closing, the windows all go back to their places together, and a
  minimised window that wasn't chosen fades out.

  In step 3 nothing changes after the closing. Alt+Tab opens no Window switcher, and the
  volume key changes the volume and shows its OSD. On the empty workspace the Overview shows
  the bar alone. In step 4 the chosen window is raised as the
  windows go back, and has the focus; `xterm` comes back unminimised. In step 5 the focused
  window is marked at opening, each arrow selects the nearest window that way, and Enter
  chooses it as a click does. With no window focused, nothing is marked until Right, which
  marks the top row's left window. In step 6 `xterm` closes and the Overview stays open, the
  others easing to their new places. `mate-calc` joins the layout, the others easing to make
  room. The developer notes which window has the focus after Esc: `mate-calc` can take it
  (242), and the spec doesn't say if it should. In step 7 the video keeps playing in the layout, as smooth as on the desktop. The
  second workspace's cell shows its window moving, at about 10 frames a second: the developer
  judges if the bar's size and that rate look right. If not, the developer changes them and
  records it in the [spec](https://linear.app/jplassala/document/spec-overview-spec-eec483ad739c)'s
  "The bar" and "Live content". In step 8 each switch slides the laid-out windows, the old
  layout out and the new in, the Overview staying open, and no Workspace popup shows. A button
  does what its key does, wherever the pointer is, and chooses or closes no window. In step 9
  the dragged window follows the pointer over the layout and the bar, shrinks to the cell's
  scale over a cell, about the point it was held by, and the cell under it is marked. Dropped
  on the third cell it eases into that cell's picture, at its place there, and leaves the
  layout; the others ease to their new places. On the active cell, on the empty area and after
  Esc, it eases back to its slot. The Overview stays open after each. On the third workspace
  the terminal has the geometry it had. In step 10 each second press reverses the transition
  from where it is, with no jump. In step 11 the slide is cut to its end and the Overview
  opens. In step 12 each opening and closing, and the drop, is at once, with no movement, and
  the Overview works as before. In step 13 the lock ends the Overview and the drag, the locker
  shows nothing of it, and after the unlock the windows are in their places, on their
  workspaces. In step 14 Super+W opens no Overview, and the key goes to the focused window.

  In every step the transition is smooth at the monitor's refresh rate, with no frame that
  stutters or tears. It takes a quarter of a second, starts fast and slows into place.
- **Debug tool:** `DumpState`'s `overview`, there while it's open: its `output`, `workspace`,
  `windows` with their `rect` and `label`, `selected`, the `bar`'s cells, `drag`, `drop`,
  `sliding` and `reflow`. `overview_transition`, there while it runs: its `direction`,
  `part`, `dim`, `layer_surfaces` and the drawn `windows`. `wlr-randr` for the refresh rate.

### The Overview on two monitors

- **Milestone:** `overview`, the developer's to run (247).
- **Steps:**
  1. Log into *MATE on Caminito* on the tty backend, with two monitors side by side and four
     workspaces in one row. Run `gsettings get org.mate.Caminito workspaces-per-output`,
     which prints `true`. Open a terminal and pluma on the first workspace of the first
     monitor, a terminal on the first workspace of the second, and `xterm` (X11) on the
     second workspace of the second. Maximise pluma. Open a `mate-calc` on the first monitor
     and move it to straddle the two, with its larger part on the first.
  2. Focus the first monitor's terminal and press Super+W. Press Esc. Focus the second
     monitor's terminal and press Super+W. Press Esc.
  3. Open the Overview from the first monitor's terminal, and click the second workspace's
     cell in the second monitor's bar. Press Right, then Ctrl+Alt+Left. Close it.
  4. Open it and drag the first monitor's terminal onto the second monitor's layout. Drag it
     back onto the first monitor's third cell. Drag the maximised pluma onto the second
     monitor's layout. Close the Overview, and look at where each window is. Put them back.
  5. Open it and unplug the second monitor. Plug it in again, with the Overview still open,
     and close it.
  6. `gsettings set org.mate.Caminito workspaces-per-output false`, open the Overview, click
     the second cell of one monitor's bar, then the first's, and press Ctrl+Alt+Right and
     Ctrl+Alt+Left. Drag a terminal onto the other monitor's layout. Close it, and set the key
     back to `true`.
- **Expected:** in step 2 the Overview opens on both monitors at once, each with its own
  active workspace's windows and its own bar, both dimmed, with each panel and the desktop
  icons fading. The transitions run together, smooth at each monitor's refresh rate.
  `mate-calc` is in the first monitor's layout alone. The selection starts on the focused
  terminal's monitor. In step 3 the click switches the second monitor alone, which then has
  the selection with nothing selected, so Right marks its top-left window. The focus stays
  on the first monitor's terminal. Ctrl+Alt+Left switches the second monitor back. In step 4 the terminal leaves the first monitor's layout
  and joins the second's, at the same place relative to its work area, shrunk if it must be,
  the second's windows easing to make room. Over the first monitor's bar it takes that bar's
  scale, and then goes to the first monitor's third workspace. pluma is maximised on the
  second monitor. The Overview stays open after each. When the focused terminal is dropped
  on the second monitor, the selection stays on the first (248): the developer notes if that
  matters. In step 5 the Overview stays open on the first
  monitor, with the second's windows in its layout if they are on its workspace, and opens on
  the second monitor again when it comes back. In step 6 both bars show the same workspaces, a
  click on one switches both, and each binding switches both. The terminal dropped on the
  other monitor is on the same workspace.
- **Debug tool:** `DumpState`'s `overview`, whose `outputs` has each monitor's part, and its
  `drag`'s `onto`. `overview_transition`'s `outputs`.

### The Application overview

- **Milestone:** `overview`, the developer's to run (CMN-185).
- **Steps:**
  1. Log into *MATE on Caminito* on the tty backend, with two monitors and four workspaces.
     Run `gsettings get org.mate.Caminito application-overview`, which prints
     `'disabled'`, and look for *Show the windows of the current application side by side*
     in *System Shortcuts*. Set it to Super+A there. On the first workspace of the first
     monitor open two caja windows and a terminal, and on the second monitor a third caja
     window and pluma. On the second workspace of the first monitor open a fourth caja
     window.
  2. Focus a caja window and press Super+A. Click the second workspace's cell in the first
     monitor's bar, then the first workspace's. Close it, focus pluma, and press Super+A.
  3. Focus a caja window and press Super+A, then Super+W, then Super+A, then Super+A again.
  4. Minimise every window, and press Super+A.
  5. Set the top-right corner to *Application overview* in *Workspace Behaviour*, focus a
     caja window, and rest the pointer in the second monitor's top-right corner; rest it
     there again once it has opened.
- **Expected:** in step 2 each monitor shows only caja's windows of its active workspace,
  the first monitor's two and the second's one, and each bar's cells draw every window:
  the terminal and pluma are in the cells. The switch shows the fourth caja window, and the
  first workspace's two again. With pluma focused, only pluma shows, on the second monitor,
  and the first monitor shows an empty layout under its bar. In step 3 Super+W changes the
  view to the Overview without closing it: caja's windows ease to their new places and the
  terminal and pluma fade in. Super+A takes it back to caja alone, the others fading out,
  and the second Super+A closes it. In step 4 nothing opens. In step 5 it opens with the
  selection on the second monitor, and the second rest closes it.
- **Debug tool:** `DumpState`'s `overview`, whose `scope` is `application`, and each
  monitor's `fading`.

### A Wayland menu slides out of what it's attached to

- **Milestone:** M9
- **Steps:**
  1. Log into *MATE on Caminito* on the tty backend, with the panel at the top. Run
     `gsettings get org.mate.Caminito popup-slide`, which prints `true`.
  2. Open the panel's main menu, then a submenu of it. Open pluma's *File* menu from its
     menubar, a submenu of its *View* menu, and a combo box's list in its *Preferences*.
  3. Move the panel to the bottom of the screen and open its main menu again. Move it back.
  4. Open pluma's *File* menu and click *New* at once, before the menu has finished sliding.
  5. Hold the pointer over a toolbar button of pluma until its tooltip shows.
  6. `gsettings set org.mate.Caminito popup-slide false`, and open the menus of step 2 again.
     Set it back to `true`. Turn each of these against the slide in turn, open a menu, and set
     it back: `org.mate.Marco.general compositing-manager false`, `org.mate.Marco.general
     reduced-resources true` and `org.mate.interface enable-animations false`.
  7. Open the Overview with Super+W, right-click caja's desktop through it if it shows, and
     close the Overview.
  8. Open the panel's main menu and a submenu of it, and close them with a click on the
     desktop. Open pluma's *File* menu and close it with Escape. Open it again and click
     outside it at once, before it has finished sliding out.
  9. Open pluma's *File* menu and close pluma with Ctrl+Q from the keyboard while it's open.
  10. Press Alt+Space over pluma, then *Move to Another Workspace* with the arrows, then Left,
      then Escape. Move pluma to the bottom of the screen, right-click its titlebar, and press
      Alt+Space and choose *Maximize* at once, before the menu has finished sliding. Press
      Alt+Tab, and Ctrl+Alt+Right then Ctrl+Alt+Left.
- **Expected:** in step 2 the panel's menu comes down out of the panel, and its submenu out of
  the menu's side; pluma's menus come down out of the menubar, its submenu out of its menu's
  side, and the combo box's list comes down over its button. Nothing of a menu is drawn over
  the panel, the menubar or the parent menu while it slides. A short menu only slides; a long
  one fades in as well. In step 3 the menu comes up out of the bottom panel. In step 4 the
  new document opens: the click reaches the row at its real place. In step 5 the tooltip
  slides out too. In step 6 each menu is there at once. In step 7 nothing slides in the
  Overview. In step 8 the menu and its submenu slide back together, each into the edge it came
  out of, and the click on the desktop reaches it; pluma's menu slides back up into the
  menubar; the one clicked away part-way turns round from where it is, with no jump. In step 9
  the menu doesn't slide on its own: it leaves inside pluma's closing picture. In step 10 the
  Window menu comes down from pluma's corner, its submenu out of its right side and back into
  it on Left, and the menu slides back up on Escape; the one opened near the bottom, turned up
  to fit, comes up instead; *Maximize* chosen part-way maximises pluma at once and the menu
  turns round; the Window switcher and the Workspace popup are there at once.

  Each slide takes a fifth of a second, starts fast and slows into its place, or starts slow
  and speeds into its edge as it closes, with no frame that stutters. An X11 app's menus have
  an item of their own.
- **Debug tool:** `DumpState`'s `popup_slides`, there while a popup slides out, and
  `closing_popups`, there while a closed popup's picture slides back: each has its `rect` at
  its place, its `edge`, and the `offset` and `alpha` it's drawn at. The Window menu's and its
  submenu's come after the other popups'.

### An X11 app's menu slides away from the pointer

- **Milestone:** M9
- **Steps:**
  1. Log into *MATE on Caminito* on the tty backend, with the panel at the top. Run
     `gsettings get org.mate.Caminito popup-slide`, which prints `true`. Start pluma on
     Xwayland with `GDK_BACKEND=x11 pluma`.
  2. Open its *File* menu from its menubar, then a submenu of its *View* menu. Right-click
     its text for its context menu.
  3. Move pluma down until its menubar is near the bottom of the screen, so the *File* menu
     opens above it, and open it. Move pluma back.
  4. Open the *File* menu and press Down and Return at once, before it has finished sliding.
     Open it again and click *New* at once.
  5. Hold the pointer over a toolbar button until its tooltip shows.
  6. Open the *File* menu and close it with Escape. Open it again and press Escape at once,
     before it has finished sliding out.
  7. Drag a file from caja onto pluma's text, if caja runs on Xwayland too.
  8. `gsettings set org.mate.Caminito popup-slide false`, and open and close the menus of step
     2 again. Set it back to `true`.
- **Expected:** in step 2 the *File* menu comes down out of the menubar, the submenu out of
  its menu's side, away from where the pointer is, and the context menu comes down from the
  pointer. Nothing of a menu is drawn outside its own place while it slides. In step 3 the
  menu comes up from the menubar beneath it. In step 4 the keys reach the menu and a new
  document opens, and the click reaches the row at its real place. In step 5 the tooltip
  slides out too. In step 6 the menu slides back into the edge it came out of; the one closed
  at once turns round from where it is, with no jump. In step 7 the drag icon follows the
  pointer and doesn't slide. In step 8 each menu is there at once and gone at once.

  Each slide takes a fifth of a second, as a Wayland menu's does, with no frame that
  stutters.
- **Debug tool:** `DumpState`'s `popup_slides` and `closing_popups`, as for a Wayland menu: an
  X11 menu's `rect` is its X window's place on the screen.

### The workspace bindings switch the Active output

- **Milestone:** M9
- **Steps:**
  1. Log into *MATE on Caminito* with two monitors side by side and four workspaces in one
     row. Run `gsettings get org.mate.Caminito workspaces-per-output`, which prints `true`.
     Open a terminal on the first workspace of each monitor.
  2. Click the first monitor's terminal, and press Ctrl+Alt+Right, then Ctrl+Alt+Left. Click
     the second monitor's terminal, leave the pointer on the first monitor, and press the
     same keys.
  3. With the second monitor's terminal focused and the pointer on the first monitor, press
     Ctrl+Alt+Right two times, with a second between, and type a letter after each. Then
     press Ctrl+Alt+Left two times.
  4. With the second monitor's terminal focused, press the binding of
     `switch-to-workspace-3`, then that of `switch-to-workspace-prev` two times, if they
     have bindings.
  5. Close the second monitor's terminal with its close button, and type a letter. Click the
     first monitor's terminal and press Ctrl+Alt+Right, then Ctrl+Alt+Left.
  6. Press Ctrl+Alt+Right on the second monitor first, as in step 2, if it shows its first
     workspace. Then drag the first monitor's terminal across to the second monitor, and
     press Ctrl+Alt+Right, then Ctrl+Alt+Left.
  7. In that terminal, on the second monitor, switch the first monitor to its third
     workspace with the `SwitchOutputWorkspace` call below, and run `gsettings set
     org.mate.Caminito workspaces-per-output false`. Press Ctrl+Alt+Right and
     Ctrl+Alt+Left. Set the key back to `true`, and press Ctrl+Alt+Right.
- **Expected:** in step 2 each press switches the monitor of the terminal that was clicked,
  and the other monitor is as it was, wherever the pointer is. The Workspace popup is on the
  monitor that switches, and the switch sound goes the way that monitor went. In step 3 the
  second monitor goes to its second workspace and then its third, both empty: no window has
  the focus, the first monitor's terminal gets no letter, and the second press acts on the
  second monitor still. The way back gives the focus to the second monitor's terminal. In
  step 4 the second monitor goes to its third workspace, back to the one before, and to the
  third again, and the first monitor doesn't change. In step 5 nothing has the focus after
  the close and the first monitor's terminal gets no letter; after the click, the first
  monitor switches. In step 6 the terminal stays in view on the second monitor with the
  focus, though that monitor shows another workspace than the one the terminal left, and
  the presses switch the second monitor alone. In step 7 the first monitor alone goes to
  its third workspace at the call. When the key goes off, the first monitor shows the
  workspace the second one shows, with the terminal still in view, and the two presses
  switch both monitors together. With the key on again both keep what they show, and the
  last press switches the second monitor alone. The terminal's workspace number doesn't
  change in step 7 (231).
- **Debug tool:** `DumpState`'s `active_output`, the name of the seat's Active output, and
  each of its `outputs`' `active_workspace`. `busctl --user call org.mate.Caminito.Debug
  /org/mate/Caminito/Debug org.mate.Caminito.Debug SwitchOutputWorkspace su HDMI-A-1 2`
  switches the monitor of that name, as `wlr-randr` gives it, to its third workspace with no
  binding.

### A click says the Active output, and a new window opens on it

- **Milestone:** M9
- **Steps:**
  1. Log into *MATE on Caminito* with two monitors side by side, four workspaces in one row
     and `workspaces-per-output` at `true`. The panel is on the first monitor. Open a
     terminal on the first workspace of each monitor.
  2. Click the second monitor's terminal, and press Ctrl+Alt+Right: the second monitor is on
     an empty workspace. Leave the pointer on the first monitor, and open a terminal with
     its binding (`run-command-terminal`).
  3. Click an empty part of the first monitor's panel, and open a terminal from the panel's
     menu.
  4. Move the pointer to the second monitor, with no click, and press Ctrl+Alt+Right, then
     Ctrl+Alt+Left.
  5. Click the second monitor's wallpaper, or caja's desktop if it is there, and press
     Ctrl+Alt+Right, then Ctrl+Alt+Left. Then press the binding of `rename-workspace`, if
     it has one, and cancel the dialog.
  6. Run `gsettings set org.mate.Marco.general placement-mode pointer`. Click the first
     monitor's panel, put the pointer on the second monitor and open a terminal with its
     binding. Set the key back with `gsettings reset`.
  7. Click the first monitor's terminal, leave the pointer on the second monitor, and hold
     Alt and press Tab.
- **Expected:** in step 2 the new terminal is on the second monitor, on the empty workspace
  that monitor shows, with the focus. In step 3 the click on the panel leaves the focus
  where it was, and the terminal from the menu is on the first monitor, on the workspace it
  shows. In step 4 the presses switch the first monitor, where the last terminal has the
  focus, and not the monitor the pointer is on. In step 5 the presses switch the second
  monitor, and the dialog offers the name of the workspace the second monitor shows. In
  step 6 the terminal opens under the pointer on the second monitor, on the workspace that
  monitor shows. In step 7 the Window switcher's popup is on the second monitor, where the
  pointer is (222).
- **Debug tool:** `DumpState`'s `active_output`, and its `active_workspace`, which is that
  output's.

### A dialog follows its window's workspace, and a sticky window stays, on two monitors

- **Milestone:** M9
- **Steps:**
  1. Log into *MATE on Caminito* with two monitors side by side, four workspaces in one row
     and `workspaces-per-output` at `true`. The panel is on the first monitor, and its
     window list shows the windows of all workspaces. Open a `pluma` on the first workspace
     of the first monitor and a terminal on the first workspace of the second.
  2. In `pluma`, open *Search > Find…* and drag the Find dialog to the second monitor. Click
     the `pluma` window and press Ctrl+Alt+Right, then Ctrl+Alt+Left. Click the Find dialog
     and press the same keys. Close the dialog.
  3. In the terminal's Window menu (Alt+Space), choose *Always on Visible Workspace*. Press
     Ctrl+Alt+Right two times, then Ctrl+Alt+Left two times. Drag the terminal to the first
     monitor and press the same keys. Press Ctrl+Alt+Right, and choose *Only on This
     Workspace* in its Window menu. Press Ctrl+Alt+Left, then Ctrl+Alt+Right.
  4. With the terminal focused on the first monitor's second workspace, press
     Ctrl+Shift+Alt+Right, then Ctrl+Shift+Alt+Left.
  5. Drag the terminal back to the second monitor. Click it, and press Ctrl+Alt+Right until
     the second monitor shows its last workspace, where the terminal isn't. Open a second
     terminal there, and open its Window menu.
  6. Press the binding of `move-to-monitor-w` with the second terminal focused, if it has
     one, and that of `move-to-monitor-e` after it.
  7. Click the first monitor's panel, press Ctrl+Alt+Left until the first monitor shows
     `pluma`, and click the `pluma` window. In the window list, click the button of the
     first terminal, which no monitor shows.
- **Expected:** in step 2, with `pluma` clicked, the first monitor switches and the Find
  dialog leaves the second monitor with `pluma`, and comes back with it; the second monitor
  shows its first workspace all the time. With the dialog clicked, the second monitor
  switches, and the dialog stays in view with the focus, as `pluma` does on the first. In
  step 3 the terminal stays in view through each switch of the monitor it's on, and, made
  not sticky, it's on the first monitor's second workspace alone. In step 4 the terminal
  goes to the first monitor's third workspace and back, the first monitor goes with it, and
  the second monitor doesn't change. In step 5 the menu has *Move to Workspace Left* and no
  *Move to Workspace Right*, though the first monitor shows another workspace. In step 6 the
  terminal goes to the first monitor, in view on the workspace that monitor shows, and back
  to the second one's last workspace. In step 7 the second monitor alone switches to the
  first terminal's workspace, the terminal has the focus, and the first monitor still shows
  `pluma` (223).
- **Debug tool:** `DumpState`'s `outputs`, each with its `active_workspace`, and each
  window's `workspace`.

### Alt+Tab and Show Desktop on two monitors

- **Milestone:** M9
- **Steps:**
  1. Log into *MATE on Caminito* with two monitors side by side, four workspaces in one row
     and `workspaces-per-output` at `true`. The panel is on the first monitor, with a Show
     Desktop button. Open a `pluma` on the first workspace of the first monitor and a
     terminal on the first workspace of the second.
  2. Click the terminal, press Ctrl+Alt+Right, and open a second terminal: the second
     monitor shows its second workspace. Hold Alt and press Tab, and look at the popup.
     Release Alt on `pluma`.
  3. Hold the binding of `switch-windows-all`, if it has one, look at the popup, and cancel
     with Escape.
  4. Click the panel's Show Desktop button.
  5. Click the second monitor's wallpaper and press Ctrl+Alt+Left, then Ctrl+Alt+Right.
  6. Click `pluma`'s button in the window list.
  7. Click the Show Desktop button two times.
  8. Run `gsettings set org.mate.Caminito workspaces-per-output false`, and do steps 4 and 6
     again. Set the key back with `gsettings reset`.
- **Expected:** in step 2 the popup has `pluma` and the second terminal, and not the first
  terminal, which no monitor shows; `pluma` takes the focus on the first monitor and no
  monitor switches. In step 3 the popup has all three windows. In step 4 both monitors show
  the bare desktop, and the button stays pressed. In step 5 Ctrl+Alt+Left shows the first
  terminal on the second monitor and the button is no longer pressed; Ctrl+Alt+Right shows
  the bare desktop there again and the button is pressed again. The first monitor shows the
  bare desktop all the time. In step 6 `pluma` comes back on the first monitor with the
  focus, the second monitor still shows the bare desktop with the second terminal hidden
  and not minimised, and the button is not pressed. In step 7 the first click clears both
  monitors and the second brings the windows of both back. In step 8, with the key off,
  step 4 clears both monitors and step 6 brings `pluma` back and leaves every other window
  of the workspace minimised, as Marco does (224).
- **Debug tool:** `DumpState`'s `showing_desktop`, which is `true` only while every monitor
  shows its desktop, and `outputs`, each with its `active_workspace`.

### Unplugging a monitor keeps its windows on their workspaces

- **Milestone:** M9
- **Steps:**
  1. Log into *MATE on Caminito* with two monitors side by side, four workspaces in one row
     and `workspaces-per-output` at `true`. The first monitor is the primary one, with the
     panel. Open a `pluma` on the first workspace of the first monitor.
  2. Click the second monitor's wallpaper. Open a terminal there, press Ctrl+Alt+Right and
     open a second one, and press Ctrl+Alt+Right again and open a third. Press
     Ctrl+Alt+Left, so that the second monitor shows its second workspace.
  3. Unplug the second monitor's cable. Look at the first monitor, then press Ctrl+Alt+Right
     two times, and Ctrl+Alt+Left two times.
  4. Plug the cable back in. Click the second monitor's wallpaper and press Ctrl+Alt+Right.
  5. Run `gsettings set org.mate.Marco.general dynamic-workspaces true`. Close the second
     terminal, on the first monitor's second workspace. Set the key back with `gsettings
     reset`.
  6. Run `gsettings set org.mate.Caminito workspaces-per-output false`, drag `pluma` to the
     second monitor, press Ctrl+Alt+Right, and do steps 3 and 4 again. Set the key back with
     `gsettings reset`.
- **Expected:** in step 3 the first monitor shows its first workspace still, with `pluma`
  and the first terminal, and the focus on one of them. The two presses of Ctrl+Alt+Right
  show the second terminal alone and then the third alone, each on the workspace of the
  number it had on the second monitor, and the workspace switcher shows them there. In step
  4 the second monitor comes back on its first workspace with no window, every window stays
  on the first monitor, and the binding switches the second monitor to an empty second
  workspace. In step 5 there are three workspaces on both monitors, the third terminal is on
  the second, and `num-workspaces` is 3. In step 6, with the key off, `pluma` goes to the
  first workspace of the first monitor, which shows the second, and the monitor that comes
  back shows the second workspace too, as the first does (225).
- **Debug tool:** `DumpState`'s `outputs`, each with its `active_workspace`, `active_output`,
  and each window's `workspace`. `gsettings get org.mate.Marco.general num-workspaces`.

### Each monitor's workspace switcher is its own, and the window list shows both monitors

- **Milestone:** M9
- **Steps:**
  1. Log into *MATE on Caminito* with two monitors side by side, `mate-panel-caminito` at
     `pkgrel` 13 or later, four workspaces in one row and `workspaces-per-output` at `true`.
     The first monitor is the primary one, with the bottom panel, its window list and its
     workspace switcher.
  2. Right-click the top panel, choose *New Panel*, and in the new panel's properties choose
     the second display. Right-click the new panel, choose *Add to Panel* and add a
     *Workspace Switcher*.
  3. Open a `pluma` on the first monitor and a terminal on the second. Click the third button
     of the second monitor's switcher, then the second button of the first monitor's.
  4. Click the first button of each switcher. Click the second monitor's wallpaper, press
     Ctrl+Alt+Right, and roll the wheel over the first monitor's switcher.
  5. Open the *Preferences* of the second monitor's switcher, read the number of workspaces
     and the names, and change the name of the second workspace.
  6. Put each monitor on its first workspace. Look at the window list. Switch the second
     monitor to its second workspace, look again, and click the terminal's button if there
     is one. In the window list's *Preferences* choose *Show windows from all workspaces*,
     look, click the terminal's button, and choose *Show windows from current workspace*
     again.
  7. In the second panel's properties choose the first display, look at its switcher, and
     choose the second display again.
  8. Unplug the second monitor's cable, look at the switchers, and plug it back in.
  9. Run `gsettings set org.mate.Caminito workspaces-per-output false`, look at the two
     switchers and the window list, click a button of each switcher, and set the key back
     with `gsettings reset`.
- **Expected:** each switcher has four buttons. In step 3 the second monitor's switcher shows
  its third button pressed and the terminal goes, while the first monitor and its switcher
  stay; then the first monitor's shows its second pressed and `pluma` goes, while the second
  monitor stays. In step 4 each switcher shows its first button pressed and no other, and
  each binding and each roll of the wheel changes one monitor and that monitor's switcher
  alone, one workspace for each notch (240). In step 5 the dialog shows four workspaces and the one set
  of names, and the new name shows in both switchers with *Workspace names* shown. In step 6
  the list has a button for `pluma` and one for the terminal, on the first monitor's panel,
  though the terminal is on the second monitor; with the second monitor on its second
  workspace the terminal's button is gone and `pluma`'s stays. With all workspaces shown the
  terminal's button is back, and a click on it switches the second monitor to its first
  workspace and no other. In step 7 the moved panel's switcher shows the first monitor's
  active workspace, with no restart of the panel, and the second monitor's again when it is
  back. In step 8 the one monitor's switchers both show its workspaces, and the second
  panel's shows the second monitor's first workspace when the monitor returns. In step 9
  both switchers have four buttons with the same one pressed, a click on either switches
  both monitors, and the window list shows the windows of the one active workspace on both
  monitors, as before the key (226, 227).
- **Debug tool:** `DumpState`'s `outputs`, each with its `active_workspace`.
  `WAYLAND_DEBUG=1 mate-panel --replace 2>&1 | grep ext_workspace` for the groups, each with
  its `output_enter`, and for the `activate` a click sends.

## Screenshots

### Print, Shift+Print and Alt+Print

- **Milestone:** M6
- **Steps:**
  1. Log into *MATE on Caminito*, with `mate-utils-caminito` installed. Open a pluma window and a
     terminal, with pluma focused, and drag pluma down until the bottom panel covers the lower
     part of its window.
  2. Press `Print`, and save the screenshot from the dialog.
  3. Press `<Shift>Print`, drag over an area that has part of both windows, and save it.
  4. Press `<Alt>Print`, and save it.
  5. Focus the terminal, run `mate-screenshot --interactive` from Alt+F2, choose *Grab the
     current window*, and click *Take Screenshot*.
  6. Log into the Wayfire session, with the same package, and run `mate-screenshot --window`
     in a terminal.
- **Expected:** step 2's image is the whole screen, every monitor. Step 3's is the area that
  was dragged. Step 4's is pluma's window alone, with its Frame and all of its contents, the
  part under the panel too, and nothing of the panel or the desktop around it. Step 5's is
  the terminal, not mate-screenshot's own dialog. Step 6 prints `Error: window capture is not
  supported on Wayland.`, as stock mate-screenshot does (ADR 0011).
- **Debug tool:** the saved images. `gdbus call --session --dest org.mate.Caminito --object-path
  /org/mate/Caminito --method org.mate.Caminito.GetActiveToplevel` prints the identifier
  mate-screenshot is given, and `WAYLAND_DEBUG=1 mate-screenshot --window 2>&1 | grep
  ext_foreign_toplevel` shows the toplevel it's matched to.

## Media keys

### The volume keys change the volume and show the OSD

- **Milestone:** M6
- **Steps:**
  1. With `mate-settings-daemon-caminito` 1.29.1-4 or later installed, and a `caminito-git` whose
     `org.mate.Caminito` has `AcceleratorRepeated` (`busctl --user introspect org.mate.Caminito
     /org/mate/Caminito | grep Accelerator`), log into *MATE on Caminito*, with sound playing.
  2. Press the volume up, volume down and mute keys, once each, and mute again. Then hold
     volume down for two seconds, and hold mute for two seconds.
  3. Make a window fullscreen, such as a browser's with F11, and click in a text field in
     it. Press volume up, and type while the OSD shows.
  4. Run the first command, press Ctrl+Alt+M, then press the mute key, then run the second:

     ```sh
     gsettings set org.mate.SettingsDaemon.plugins.media-keys volume-mute '<Control><Alt>m'
     gsettings reset org.mate.SettingsDaemon.plugins.media-keys volume-mute
     ```

     With `mate-control-center-caminito` installed, change *Volume mute* in *Keyboard
     Shortcuts* instead.
  5. On a laptop with a touchpad toggle key, press it twice.
  6. Log out, log into *MATE*, the X11 session, and repeat step 2. Then log into the Wayfire
     session and run this in a terminal, since the daemon isn't on `PATH`:

     ```sh
     /usr/lib/mate-settings-daemon/mate-settings-daemon --debug --replace 2>&1 | grep -i media
     ```
- **Expected:** in step 2 each press changes the volume or the mute, and shows the OSD in the
  lower half of the monitor. The held volume key goes on lowering the volume, and the held mute
  key toggles once. In step 3 the OSD shows above the fullscreen window, and what's typed goes
  to the text field. In step 4 Ctrl+Alt+M mutes with no log out, the mute key does nothing
  while it's set, and works again after the reset. In step 5 the OSD says *Touchpad disabled*,
  the touchpad stops moving the pointer, and the second press brings it back. In step 6 X11
  MATE's keys and OSD are as before. On Wayfire the daemon run from the terminal is on the
  Wayland display and doesn't load the plugin (ADR 0011): it prints `Failed to call
  "GrabMediaPlayerKeys"`, with `Object does not exist at path
  "/org/mate/SettingsDaemon/MediaKeys"`, which is the stock mpris plugin finding no media-keys
  plugin. It doesn't print the daemon's own `requires X11; not loading on this display`. That
  session's own daemon is another matter: `mate-wayland-components.sh` starts it with
  `GDK_BACKEND=x11` and starts it again when it ends, so it's on XWayland and loads the plugin
  through its X11 code, with the stock package too.
- **Debug tool:** `DumpState`'s `accelerator_grabs`, which lists each key msd grabbed with
  msd's unique name, and its `outputs`' `layer_surfaces`, where the OSD is `mate-osd` on the
  `overlay` layer with `keyboard_interactivity` `none` while it shows. `dbus-monitor
  "interface='org.mate.Caminito'"` can't see the signals, which go to msd alone: run
  `/usr/lib/mate-settings-daemon/mate-settings-daemon --debug --replace` for msd's side.

### A custom shortcut runs its command

- **Milestone:** M6
- **Steps:**
  1. With `mate-settings-daemon-caminito` 1.29.1-6 or later installed, and a `caminito-git` with
     ticket 100's change (a grab named by the keysym its press types), log into *MATE on
     Caminito*.
  2. Add a shortcut for `mate-calc` on Ctrl+Alt+K. With `mate-control-center-caminito`
     installed, use *Add* in *System Shortcuts* and press the keys there. Without it:

     ```sh
     d=/org/mate/desktop/keybindings/custom-check
     dconf write $d/name "'Calculator'"
     dconf write $d/action "'mate-calc'"
     dconf write $d/binding "'<Primary><Alt>k'"
     ```

  3. Click in a text editor's window, and press Ctrl+Alt+K. Close the calculator, then hold
     Ctrl+Alt+K for two seconds.
  4. Change the shortcut's keys to Shift+Super+1, in *System Shortcuts* or with `dconf
     write $d/binding "'<Shift><Mod4>exclam'"`. Press Ctrl+Alt+K, then Shift+Super+1.
  5. Remove the shortcut, in *System Shortcuts* or with `dconf reset -f $d/`. Press
     Shift+Super+1 in a terminal running `wev`.
  6. Add the shortcut again as in step 2, with `XF86AudioMute` for its keys, which the mute
     key has, and press the mute key. Then remove the shortcut.
  7. Log out, log into *MATE*, the X11 session, and repeat steps 2 to 5 with `xev` for `wev`,
     and Super+1 (`<Mod4>1`) for Shift+Super+1.
- **Expected:** in step 3 the calculator opens with no log out, the editor gets no `k`, and
  the held keys open one calculator, where X11's open one at every repeat. In step 4
  Ctrl+Alt+K opens nothing and Shift+Super+1 opens the calculator. In step 5 `wev` shows the
  press of `1`: the keys are the focused window's again. In step 6 the mute key mutes and no
  calculator opens, and msd's log has `Grab failed for the key binding (XF86AudioMute)`: on
  X11 both would happen. In step 7 X11 MATE's shortcuts are as before (ADR 0011). Shift+Super+1
  isn't tried there: msd's X11 code grabs nothing for a shortcut whose keysym needs Shift, with
  the stock package too (100).
- **Debug tool:** `DumpState`'s `accelerator_grabs`, where the shortcut's keys are listed under
  a unique name of msd's that isn't the one the media keys have: the plugin has a connection
  of its own. `/usr/lib/mate-settings-daemon/mate-settings-daemon --debug --replace` for msd's
  side.

### A mouse button runs a command

- **Milestone:** M8
- **Steps:**
  1. With `mate-settings-daemon-caminito` 1.29.1-6 or later and `mate-control-center-caminito`
     1.29.1-6 or later installed, and a `caminito-git` with ticket 185's change (a grab of a
     mouse button), log into *MATE on Caminito* with a mouse that has back and forward
     buttons. Open Firefox, and open two pages one after the other in a tab.
  2. In *System Shortcuts*, click *Add*, name the shortcut *Calculator* with the command
     `mate-calc`, click its shortcut, and with Super held click the back button over the
     dialog. Then run `dconf dump /org/mate/desktop/keybindings/` to find its directory, and
     set `d` to it, as in `d=/org/mate/desktop/keybindings/custom0`.
  3. With Super held, click the back button over Firefox's page, and then over the panel.
     Close the calculators. Then click the back button over the page without Super.
  4. Go forward to the second page again. In *System Shortcuts*, click the shortcut again and
     click the forward button alone over the dialog. With Super held, click the back button
     over a terminal running `wev`, and then click the forward button over Firefox's page.
  5. Remove the shortcut with *Remove* in *System Shortcuts*, go back a page in Firefox, and
     click the forward button over the page.
  6. In *System Shortcuts*, click *Volume mute*'s shortcut and click the forward button over
     the dialog. Click the forward button twice, then click *Volume mute*'s shortcut again and
     press the mute key, or Backspace and then `gsettings reset
     org.mate.SettingsDaemon.plugins.media-keys volume-mute`.
  7. Add the shortcut again as in step 2, log out, log into *MATE*, the X11 session, and
     click the back button with Super held. Change the shortcut to `<Primary><Alt>k` and
     press Ctrl+Alt+K. Then remove it.
- **Expected:** in step 3 each click with Super opens a calculator, wherever the pointer is,
  and Firefox doesn't go back; without Super it goes back. In step 4 `wev` shows the back
  button's press, which is the focused window's again, and the forward button opens a
  calculator while Firefox stays on its page. In step 2 the row reads *Super+Back Button*
  and `dconf read $d/binding` is `'<Super>Button8'`. In step 5 Firefox goes forward. In step
  6 the two clicks mute and unmute, with the OSD. In step 7 the button opens nothing, as with the
  stock package, which can't read a button (ADR 0028), and Ctrl+Alt+K opens the calculator
  (ADR 0011).
- **Debug tool:** `DumpState`'s `accelerator_grabs`, where `<Super>Button8` is listed under
  msd's unique name; Caminito's log, which names a button it won't grab, such as `Button1`
  with no modifier, or `Button4`, the wheel's; `wev` or `xev` for the button's number.
  `/usr/lib/mate-settings-daemon/mate-settings-daemon --debug --replace` for msd's side.

### The brightness keys change the backlight with a Wayland window focused

- **Milestone:** M7
- **Steps:**
  1. On a laptop, with `mate-power-manager-caminito` installed, log into *MATE on Caminito*.
     Open pluma, a native Wayland window, and click in it.
  2. Press brightness down and brightness up, once each. Then hold brightness down for two
     seconds.
  3. If the laptop has keyboard backlight keys, press them, and the backlight toggle key twice.
  4. Open an xterm, an XWayland window, click in it, and repeat step 2.
  5. Log out, log into *MATE*, the X11 session, and repeat step 2.
- **Expected:** in steps 2 and 4 each press changes the backlight by a step and shows
  mate-power-manager's brightness OSD, and the held key goes on dimming. In step 3 the keyboard
  backlight changes and the toggle turns it off and on again. The key reaches neither pluma nor
  xterm. In step 5 X11 MATE's keys and OSD are as before.
- **Debug tool:** `DumpState`'s `accelerator_grabs`, which lists the eleven keys
  mate-power-manager grabbed (`XF86MonBrightnessUp`, `XF86PowerOff` and the rest) with its unique
  name. For mate-power-manager's side, end it with `pkill -x mate-power-manager` and run it
  in a terminal as mate-session starts it, `GDK_BACKEND=x11 mate-power-manager --verbose`
  (78): it logs `emitting button-pressed : brightness-up` for each press it acts on.

## The lock

### swaylock locks the session, over everything, and its password unlocks

- **Milestone:** M7
- **Steps:**
  1. Run `tools/dev-nested`, then start a GTK3 app and `xterm` in it. Start something on the
     `overlay` layer too, such as `wob` or `fuzzel`, and leave it open.
  2. Open a menu of the GTK3 app and leave it open. From a terminal outside the nested
     Caminito, with its `WAYLAND_DISPLAY`, run `sleep 3; swaylock`. From a second one, with
     the same `WAYLAND_DISPLAY`, run `sleep 6; grim /tmp/locked.png` at the same time (grim
     1.5 or later, which has ext-image-copy-capture). Or instead, within the 3 seconds before
     swaylock starts, run `mate-screenshot --delay=6` from the nested `xterm`.
  3. Once it locks, move the pointer, click and type a few letters. Press no binding: in a
     nested run the host's compositor has them, and its Alt+F4 closes the nested Caminito.
  4. From the outside terminal run `swaylock` again, with the same `WAYLAND_DISPLAY`.
  5. Type the password and press Enter.
  6. Lock again, and from the outside terminal run `pkill -x swaylock`. Then `swaylock` again.
  7. On the tty, run bare `caminito`, start a terminal in it, and run `swaylock` from the
     second login. Press Alt+F4, Alt+Tab, Ctrl+Alt+Right and a volume key, then unlock.
- **Expected:** in step 2 the output goes dark, then shows swaylock, and neither the menu,
  the overlay surface nor any window shows over it. The screenshot taken in step 2 is what
  swaylock shows, with nothing of the session in it: grim's file at once, mate-screenshot's
  in its save dialog after step 5. In step 3 nothing of the session reacts, and swaylock's
  indicator shows the letters. In step 4 the second swaylock exits at once, as its lock is
  refused. In step 5 the session is back with the window that had the keyboard
  having it, and the menu closed. In step 6 the output stays dark with the session locked,
  and the second swaylock is refused: until 126's locker and 130's `loginctl unlock-session`,
  the nested Caminito is ended from outside. In step 7 no window closes, no switcher opens,
  the workspace doesn't change and no volume OSD shows, and after the unlock the terminal is
  there as it was.
- **Debug tool:** `DumpState`'s `lock`: `unlocked`, `locked`, and `locker-lost` after the
  `pkill`. Caminito logs `can't give a client the session lock` for each refused `lock`.

### caminito-locker shows MATE's unlock dialog, and the password unlocks

- **Milestone:** M7
- **Steps:**
  1. With `caminito-git` installed, run `tools/dev-nested` and start a GTK3 app in it. From a
     terminal outside the nested Caminito, with its `WAYLAND_DISPLAY`, run
     `caminito-locker --verbose`.
  2. Type a wrong password and press Enter. Then type the right one and press Enter.
  3. Make a theme of the default one with red labels, and lock with it, then with a theme
     that isn't installed:

     ```sh
     cd /usr/share/mate-screensaver
     sudo cp lock-dialog-default.ui lock-dialog-red.ui
     echo 'label { color: #c00; }' | sudo tee lock-dialog-red.css
     gsettings set org.mate.screensaver lock-dialog-theme red
     ```

     Run `caminito-locker` and unlock. Then `gsettings set org.mate.screensaver
     lock-dialog-theme nonexistent`, run `caminito-locker` and unlock. Then `gsettings reset
     org.mate.screensaver lock-dialog-theme` and `sudo rm lock-dialog-red.*`.
  4. Log into *MATE on Caminito* with both monitors connected, and lock with *System* ▸
     *Lock Screen*: Caminito spawns `caminito-locker`, its default `locker-command` (126).
  5. Move the pointer from one monitor to the other and back, and type a letter on each.
     Unlock.
  6. In *Appearance*, pick another GTK theme, such as *TraditionalOk*, and lock again. Unlock,
     and set the theme back.
- **Expected:** in step 1 the session is behind the lock, and the dialog is mate-screensaver's
  as X11 MATE shows it: the clock and date, the user's face (`~/.face`, if there is one) and
  real name, *user on host*, the *Password:* prompt, and *Cancel* and *Unlock*, in the GTK
  theme's colours, on the desktop's background (132). No tooltip shows over the
  prompt's eye icon. In step 2 the dialog says *Incorrect password.* and asks again, and the
  right password unlocks: `caminito-locker` exits and the app has the keyboard again. In
  step 3 the first lock's labels are red, and the second's dialog is the default one again,
  from caminito-locker's own copy of it. In
  step 5 the dialog goes to the monitor the pointer is on, and the letters typed there
  reach its prompt. In step 6 the dialog is in the other theme's colours.
- **Debug tool:** `DumpState`'s `lock` and each output's `lock_surface`. `--verbose` logs the
  PAM conversation (`Got message style 1: 'Password: '`), `No theme at …` for step 3, and
  `Moving the dialog` for step 5.

### caminito-locker's background, status message, Switch User and Log Out

- **Milestone:** M7
- **Steps:**
  1. Log into *MATE on Caminito* with both monitors connected and a picture as the desktop
     background, and on the session bus run `gsettings set org.mate.Caminito locker-command
     "['caminito-locker', '--verbose']"`. Lock with *System* ▸ *Lock Screen*, and unlock.
  2. Run `gsettings set org.mate.screensaver picture-filename` with the path of another
     image, and lock. Unlock, and do it again with a path that doesn't exist. Unlock, and
     `gsettings reset org.mate.screensaver picture-filename`.
  3. Set a status message, as mate-panel's status applets do, and lock:

     ```sh
     dbus-send --session --dest=org.gnome.SessionManager --type=method_call \
       /org/gnome/SessionManager/Presence \
       org.gnome.SessionManager.Presence.SetStatusText string:'Back at three'
     ```

     While locked, from another tty, set it to `'Back at four'` in the same way, with the
     session's `DBUS_SESSION_BUS_ADDRESS`. Unlock, run `gsettings set org.mate.screensaver
     status-message-enabled false` and lock. Unlock, and reset the key.
  4. Lock, and click *Switch User*. In the display manager's greeter, go back to the
     session: pick it in the greeter, or press Ctrl+Alt+F<n> for its tty. Then unlock.
  5. Run `gsettings set org.mate.lockdown disable-user-switching true` and lock. Unlock, and
     reset the key.
  6. Run `gsettings set org.mate.screensaver logout-enabled true`, `gsettings set
     org.mate.screensaver logout-delay 1` and `gsettings set org.mate.screensaver
     logout-command 'mate-session-save --logout'`, and lock. Wait a minute and a half, then
     click *Log Out*. Log in again, reset the three keys and `locker-command`.
- **Expected:** in step 1 both monitors show the desktop background, as X11 MATE's lock
  screen does, and the dialog has the GTK theme's background, on the monitor under the
  pointer. In step 2 both show the other image, then the desktop background again. In step 3
  the dialog shows *Back at three* above its buttons, then *Back at four*, and no message
  with the key off. In step 4 the display manager's greeter shows, and the session is still
  locked when it's back. With sddm, mate-screensaver's LightDM path reaches it: sddm sets
  `XDG_SEAT_PATH` and has `SwitchToGreeter` on its seat. In step 5 there's no *Switch User*.
  In step 6 *Log Out* appears a minute after the lock, and the session logs out.
- **Debug tool:** the journal's `caminito-locker` lines: where each monitor's wallpaper came
  from (218, below), then `Creating background w:… h:…` for it, `Status message: …`, `Switch
  User: enabled` or `disabled` (and `Warning: Unknown DM for switch button` where no display
  manager's path applies), and `Log Out: after its delay` then `Log Out: shown`. `busctl --system get-property org.freedesktop.DisplayManager
  "$XDG_SEAT_PATH" org.freedesktop.DisplayManager.Seat CanSwitch` says whether the greeter can
  be reached that way.

### How the give-up state looks

- **Milestone:** M7
- **Steps:**
  1. Run `tools/dev-nested`, then start a GTK3 app in it. On the nested bus, run
     `gsettings set org.mate.Caminito locker-command "['false']"`.
  2. Lock with `mate-screensaver-command --lock` on the nested bus, which reaches Caminito's
     Screensaver shim (127).
  3. Wait ten seconds, and read Caminito's log and `DumpState`.
  4. Set the window's scale with `--scale 2` on another run, and repeat steps 1 to 3.
- **Expected:** in step 2 the output goes dark at once, and nothing of the session shows from
  then on. Within ten seconds a line in the interface font is in the middle of the dark output:
  *To unlock, switch to a text console and run: loginctl unlock-session* and the ID of the
  session the nested Caminito is in, the host's `$XDG_SESSION_ID`. It's sharp at scale 2 too.
  Nothing of the session reacts to the pointer or the keys.
- **Debug tool:** the log has five `exited` lines for `false`, then `can't start the locker, so
  it's given up on`. `DumpState`'s `lock` is `pending`, and its `locker` has `state` `gave-up`
  and the `message`.

### The suspend race with a slow locker

- **Milestone:** M7
- **Steps:**
  1. Log into MATE on Caminito (a Session, on the tty), with mate-power-manager's
     `lock-suspend` on. Run `systemd-inhibit --list` and find Caminito's `sleep` delay.
  2. Make the locker slow: `gsettings set org.mate.Caminito locker-command "['sh', '-c', 'sleep
     5; exec caminito-locker']"`. Suspend from the panel's Shut Down dialog, and wake the
     machine with a key once it sleeps.
  3. Set `locker-command` to `"['sleep', 'infinity']"`, a locker that never draws, and repeat
     step 2.
  4. Reset `locker-command`, and repeat step 2 with the default locker.
- **Expected:** after each resume the lock is what shows, never the desktop, not even for a
  frame. In step 2 the machine sleeps about 2 s after the request with the dark lock on screen,
  and in step 3 likewise; caminito-locker's dialog is there on resume in step 4. Caminito's
  `sleep` delay is in `systemd-inhibit --list` again after each resume.
- **Debug tool:** `journalctl --user -b -t caminito -p warning` has `the lock isn't on screen 2 s
  after logind asked to sleep` for steps 2 and 3, and not for 4. `journalctl -b -u
  systemd-logind` has the suspend and how long it was delayed.

### `loginctl unlock-session` from a console

- **Milestone:** M7
- **Steps:**
  1. In a Session, run `loginctl lock-session` from a terminal, then
     `loginctl show-session $XDG_SESSION_ID -p LockedHint` from another tty.
  2. Unlock with the password, and read `LockedHint` again.
  3. Set `locker-command` to `"['false']"`, run `loginctl lock-session`, and wait for the
     give-up line. Switch to a text console, log in, and run the command the line names. Switch
     back.
  4. Reset `locker-command`, lock with `loginctl lock-session`, and unlock from a console with
     `loginctl unlock-session` and the session's ID while caminito-locker shows.
- **Expected:** step 1 locks with caminito-locker's dialog, and `LockedHint=yes`; step 2 gives
  `LockedHint=no`. In step 3 the console's command unlocks: back on Caminito's VT the desktop is
  there, and the next lock gets caminito-locker again. In step 4 the dialog goes, and the
  desktop is back.
- **Debug tool:** `loginctl show-session` with `-p LockedHint` after each step.
  `DumpState`'s `lock` is `unlocked` and its `locker` `idle` after each.

### Idle blanks, a key brings it back, a playing video holds it off, and the lock follows

- **Milestone:** M7
- **Steps:**
  1. Log into *MATE on Caminito*, and run:

     ```sh
     gsettings set org.mate.session idle-delay 1
     gsettings set org.mate.screensaver idle-activation-enabled true
     gsettings set org.mate.screensaver lock-enabled true
     gsettings set org.mate.screensaver lock-delay 1
     ```

  2. Leave the keyboard and the mouse alone. Once the screens go black, press Shift.
  3. Play a video in `mpv`, a native Wayland player that holds idle off while it plays, and
     leave everything alone for two minutes.
  4. Minimise mpv's window and leave everything alone for two minutes more, then move the
     mouse and type the password.
  5. Set the four keys back with `gsettings reset`.
- **Expected:** in step 2 every monitor goes black a minute after the last input, the cursor
  too, and Shift brings the desktop back, unlocked, as the lock delay hadn't passed (128). In
  step 3 the screens stay on. In step 4 they go black a minute after the window is minimised,
  and a minute later the session locks under the blank: the mouse brings up
  `caminito-locker`'s dialog, not the desktop, and the password unlocks.
- **Debug tool:** `DumpState`'s `idle`, whose `blanked` follows the screens and whose
  `inhibitors` has mpv's surface with its window and `shown`, and its `lock`. `--verbose`
  logs `the session is idle; blanking the outputs` and `activity; unblanking the outputs`.

### Idle powers both monitors off, a key lights them in their layout, and a video keeps them on

- **Milestone:** M7
- **Steps:**
  1. On the tty, with both of the developer's monitors (the AOC and the LG) plugged in, log
     into *MATE on Caminito*, and run:

     ```sh
     gsettings set org.mate.session idle-delay 1
     gsettings set org.mate.screensaver lock-enabled false
     gsettings set org.mate.power-manager sleep-display-ac 120
     gsettings set org.mate.power-manager sleep-display-battery 120
     ```

  2. Leave the keyboard and the mouse alone for three minutes, then press Shift.
  3. Play a video in `mpv` and leave everything alone for three minutes.
  4. Set the four keys back with `gsettings reset`.
- **Expected:** in step 2 both monitors go black after a minute, and a minute later each
  says it has no signal and goes to standby (129). Shift lights both in the saved layout,
  each in its mode, with the windows where they were: no window moves to the other monitor,
  and the panel stays where it was. Whether the AOC goes black once more as the LG comes
  back, as it does when the LG is turned off or on (87), is written in 129. In step 3 both
  stay on.
- **Debug tool:** `DumpState`'s `idle`, whose `powered_off` follows the monitors, and whose
  `on_battery` says which key applies. `--verbose` logs `the session is idle; powering the
  outputs off`, `powering the monitors off`, and their `on` lines.

### The panel's *Lock Screen* and `mate-screensaver-command`

- **Milestone:** M7
- **Steps:**
  1. Log into *MATE on Caminito*, and choose *Lock Screen* in the panel's *System* menu. Unlock
     with the password.
  2. In a terminal, run `mate-screensaver-command --query`, then
     `mate-screensaver-command --lock`, unlock, and run `mate-screensaver-command --query`
     again.
  3. Run `gsettings set org.mate.screensaver lock-delay 1`, as the default of 0 locks with
     the blank. Run `mate-screensaver-command --activate`, wait for the screens to go black,
     then leave everything alone for ten seconds and press Shift. Run
     `mate-screensaver-command --query`, and `gsettings reset org.mate.screensaver lock-delay`.
  4. Set `org.mate.session idle-delay` to 1, run `mate-screensaver-command --inhibit &`, then
     `mate-screensaver-command --query`, and leave everything alone for two minutes. Kill the
     background command and leave everything alone for a minute more. Reset `idle-delay`.
- **Expected:** step 1 locks with caminito-locker's dialog, and the password unlocks. In step
  2 the queries say "The screensaver is inactive", and `--lock` locks as the panel did. In
  step 3 the screens go black at once, Shift brings the desktop back, unlocked, and the query
  says it's inactive. In step 4 the query lists the inhibitor, the screens stay on while it
  runs, and they go black a minute after it's killed (127).
- **Debug tool:** `DumpState`'s `idle`, whose `inhibitors` has a `screensaver` entry for the
  command while it runs, and its `lock`. `dbus-monitor "interface='org.mate.ScreenSaver'"`
  shows `ActiveChanged` once at each blank or lock, and once as each ends.

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

### Sticky keys from the Keyboard dialog, in both kinds of window and at the lock screen

- **Milestone:** M7
- **Steps:**
  1. With `mate-control-center-caminito` installed, log into *MATE on Caminito*. Open `pluma`,
     a native Wayland window, and `xterm`, an XWayland one.
  2. Open *Keyboard* from the control center. On *Accessibility*, tick *Simulate simultaneous
     keypresses*. Leave the dialog open.
  3. In pluma, press and let go of Shift, then type `aa`. Press and let go of Shift twice, type
     `aa`, and press and let go of Shift once more. Do the same in xterm.
  4. With pluma focused, press and let go of Alt, then press F10.
  5. Lock the screen (*System* ▸ *Lock Screen*). At the unlock dialog, for a password with a
     capital letter in it, press and let go of Shift before that letter instead of holding it.
  6. In the dialog, tick *Accessibility features can be toggled with keyboard shortcuts*. Press
     Shift five times in a row, then press and let go of Shift and type `a` in pluma. Press
     Shift five times again.
  7. Tick *Only accept long keypresses*, and in pluma tap `a` quickly, then hold it for a
     second. Untick it, tick *Ignore fast duplicate keypresses*, and tap `a` twice quickly.
  8. In *Audio Feedback…*, tick *Beep when a modifier key is pressed*, and press Shift.
- **Expected:** in step 3 each app gets `Aa`, then `AA`, and lowercase again after the last
  Shift (134). In step 4 pluma maximises. In step 5 the password is taken. In step 6 a
  notification says *Sticky Keys turned off* after the first five presses, the `a` is
  lowercase, a notification says *Sticky Keys turned on* after the second five, and the
  dialog's tick follows each. In step 7 the quick tap types nothing and the held key types
  `a` after the delay, then the second quick tap of `a` types nothing. In step 8 the press is
  logged as a bell, which flashes as the visual bell says (169) and sounds as the audible
  bell says (170).
- **Debug tool:** `gsettings monitor org.mate.accessibility-keyboard`, `wev` in place of
  pluma and `xev` in place of xterm for the keys and modifiers each gets, and `DumpState`'s
  `bell`, whose `source` is `accessx` for one of these beeps.

### Orca with the Keyboard monitor and on the lock screen

- **Milestone:** M7
- **Steps:**
  1. With `orca` and at-spi2-core 2.55 or later installed, run
     `gsettings set org.mate.applications-at-visual startup true` and log into *MATE on
     Caminito*.
  2. Press Alt+F1, and walk the panel's menus with the arrow keys. Open pluma, a native
     Wayland window, and type a line.
  3. Click the desktop, so that no window has the keyboard. Press Insert+H, Orca's help,
     then Escape. Press Insert+T, which reads the time.
  4. Press Insert twice quickly in pluma.
  5. Lock the screen (*System* ▸ *Lock Screen*), and type the password at
     `caminito-locker`'s dialog, then Enter.
- **Expected:** Orca speaks from step 1 on. In step 2 it reads each menu item and the line
  typed. In step 3 Orca's help opens and closes and the time is read, with the focus on the
  desktop: the keys reach Orca through the Keyboard monitor, not through an app (135). In step
  4 pluma gets one Insert, which toggles overwrite. In step 5 Orca reads the dialog's label
  and field, says *star* or nothing for each letter of the password, never the letter, and
  the session unlocks.
- **Debug tool:** `busctl --user status org.freedesktop.a11y.Manager`, which is Caminito,
  `dbus-monitor --session "interface='org.freedesktop.a11y.KeyboardMonitor'"` for the calls
  Orca makes and the `KeyEvent`s it gets, and `orca --debug` for what it heard.

## Outputs

### The scale picked for each of the developer's monitors

- **Milestone:** M4
- **Steps:**
  1. With both monitors connected, run `caminito` on the tty.
  2. Start a terminal on it (`foot`), and move it to each monitor.
  3. Run `wlr-randr`, and
     `busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug org.mate.Caminito.Debug DumpState`.
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
  1. With both monitors connected, run `caminito` on the tty, and put a GTK3 window (pluma) and
     an X11 one (xterm) on the right-hand monitor. Maximise one of them.
  2. Unplug the right-hand monitor's cable. Run `wlr-randr` and `DumpState`.
  3. Plug it back in, and run them again.
  4. Do the same with the left-hand monitor.
- **Expected:** unplugging removes the output: `wlr-randr` and `DumpState` list one output, at
  0,0, and the windows that were on the unplugged monitor are on the other one, with the
  maximised one filling it. They're moved, not resized, so one larger than the output hangs
  over its edge. The monitor that stays never freezes, and never goes black, but for the README's known limitation on NVIDIA (87). Plugging the
  monitor back lights it in its preferred mode at its default scale, to the right of the other
  one, whichever side it was on before, unless a layout is saved for the two (83), and the pointer and windows can go to it again. The log
  has no `can't light` line.
- **Debug tool:** `wlr-randr`, the `outputs` and windows of `DumpState`, and `~/caminito.log`.

### The primary output moves the panel, the desktop icons and RandR's primary

- **Milestone:** M6
- **Steps:**
  1. With both monitors connected, and `mate-panel-caminito` at `pkgrel` 3 or later and
     `caja-caminito` at `pkgrel` 2 or later, log into *MATE on Caminito*. Read the primary output:

     ```sh
     primary() {
       gdbus call --session --dest org.mate.Caminito --object-path /org/mate/Caminito \
         --method org.freedesktop.DBus.Properties.Get org.mate.Caminito PrimaryOutput
     }
     choose() {
       gdbus call --session --dest org.mate.Caminito --object-path /org/mate/Caminito \
         --method org.freedesktop.DBus.Properties.Set org.mate.Caminito PrimaryOutput "<'$1'>"
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
  `wlr-randr`, `xrandr`, and `gdbus monitor --session --dest org.mate.Caminito` for the property's
  signal.

### The desktop's menu is on every output

- **Milestone:** M6
- **Steps:**
  1. With both monitors connected and `caja-caminito` at `pkgrel` 4 or later, log into *MATE
     on Caminito*.
  2. Right-click the wallpaper of the output that isn't primary, and choose *Create Folder*.
  3. Choose the other output as primary, with `choose` from the item above or in
     caminito-displays, and right-click the wallpaper of each output.
  4. Unplug the cable of the output that isn't primary, plug it back in, and right-click its
     wallpaper.
- **Expected:** the desktop's menu pops up at the pointer, on the output clicked, each time.
  The wallpaper of the output that isn't primary looks as it does with no caja running. The
  new folder appears among the icons, on the primary output (208).
- **Debug tool:** each output's `layer_surfaces` in `DumpState`: `desktop` on the primary
  output and `desktop-menu` on each of the others.

### A panel's properties choose its display

- **Milestone:** M6
- **Steps:**
  1. With both monitors connected and `mate-panel-caminito` at `pkgrel` 11 or later, log into
     *MATE on Caminito*.
  2. Right-click the bottom panel, choose *Properties*, and read the *Display* row.
  3. Choose the other display in it. Then right-click the top panel, choose *New Panel*, and
     in the new panel's properties choose the display that isn't primary.
  4. With both dialogs open, choose the other output as primary, with `choose` from the item
     above or in caminito-displays.
  5. Unplug the cable of the output that isn't primary, look, and plug it back in.
  6. Log out and in again.
- **Expected:** the row is below *Orientation*, and lists both displays as caminito-displays
  names them, the primary one with `(primary)`, and the bottom panel's is chosen. In step 3
  each panel goes to the display chosen, at that display's size, with its applets, and the
  windows of both outputs keep clear of the panels there. In step 4 the panels swap outputs,
  and each dialog's row lists the displays in their new order with its panel's still chosen.
  In step 5 the panels of the unplugged output go to the one that's left, where the row is
  gone, and they return with the monitor. After step 6 each panel is where it was chosen to
  be. A drawer's properties have no row (209).
- **Debug tool:** each output's `layer_surfaces` and `work_area` in `DumpState`, and
  `dconf watch /org/mate/panel/toplevels/` for the `monitor` key.

### Turning an output off leaves the others touching

- **Milestone:** M6
- **Steps:**
  1. With three monitors connected, run `caminito` on the tty, and run `wlr-randr`.
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
  1. With both monitors connected, run `caminito` on the tty. Set the LG to 2.0, so the AOC stays
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

### Edge resistance and snap-move when dragging a window

- **Milestone:** M8
- **Steps:**
  1. With both monitors connected side by side, log into *MATE on Caminito*, with the panel
     along the bottom. Open pluma and a terminal (xterm, for an X11 window), apart.
  2. Drag pluma slowly by its titlebar down to the panel, then on past it. Do the same with
     Alt+drag, and resize pluma from its bottom edge down to the panel.
  3. Drag pluma slowly to the right onto the other monitor, then on across it.
  4. Drag pluma slowly towards the terminal until their sides meet, then on over it.
  5. Repeat steps 2 to 4 holding Shift during the drag, and press and let go of Shift mid-drag.
  6. Repeat steps 2 to 5 dragging the terminal, and compare with Marco under *MATE*, the X11
     session.
- **Expected:** in steps 2 to 4 the window stops where its frame meets the panel, the
  monitors' shared edge (with its right side, then with its left) and the terminal's frame,
  and goes on once the pointer is about 32 pixels past (16 at a window), as in Marco. With
  Shift, it jumps to the nearest edge instead and moves from edge to edge; letting go of Shift
  goes back to following the pointer. The terminal does the same. Nothing resists moving
  away from an edge (157).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, for the windows' geometry and the
  outputs' work areas.

### Edge and corner tiling of a GTK3, a GTK4 CSD and an X11 window

- **Milestone:** M8
- **Steps:**
  1. With both monitors connected side by side, log into *MATE on Caminito*, with the panel
     along the top. Open pluma (native GTK3, Caminito's Frame), `gtk4-demo` (GTK4, drawing
     its own titlebar) and xterm (X11).
  2. Drag pluma by its titlebar to each side, each corner and the top of the first monitor,
     waiting at each before letting go, then once with Shift held. Each time, drag it back
     out by its titlebar.
  3. Drag pluma to the monitors' shared edge, just inside the first, then just past it onto
     the second, and let go there.
  4. Repeat steps 2 and 3 with `gtk4-demo` and xterm, and once with Alt+drag on xterm.
  5. Run `gsettings set org.mate.Marco.general allow-top-tiling false` and drag pluma to the
     top; run `gsettings set org.mate.Marco.general allow-tiling false` and drag it to a side.
     Reset both with `gsettings reset`.
  6. Run as part of M8's closing run (173).
- **Expected:** in step 2, while the pointer is at a side, a corner or the top, a translucent
  rectangle in the theme's selection colour shows the half, the quarter or the whole work
  area, beneath the window; it goes when the pointer moves away, and letting go fills it,
  the panel left clear. A side tile's frame is drawn square where the theme has tiled
  styles. With Shift nothing shows or tiles. Dragging out puts the window back at its size
  before, under the pointer. In step 3 it tiles to the first monitor's right half, then to
  the second's left half. In step 4 `gtk4-demo` drops its rounded corners and shadow while
  tiled and gets them back once untiled, xterm tiles as pluma does, and Alt+drag holds a
  tiled xterm still for a few pixels before it comes loose. In step 5 the top does nothing,
  then the side does nothing, as in Marco under *MATE*, the X11 session (158).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, for `tile_preview` while dragging
  and each window's `tile`, and `WAYLAND_DEBUG=1 gtk4-demo 2>&1 | grep configure` for the
  `tiled_*` states.

### The tiling keybindings and tile cycling, on a native and an X11 window

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito*, with the panel along the top. In mate-control-center's
     *System Shortcuts*, under *Window Management*, set the six *Tile window to …* actions
     to Super with Q (west side), W (east side), E (north-west corner), R (north-east),
     T (south-west) and A (south-east). Open pluma (native GTK3) and xterm (X11).
  2. With pluma focused, press each of the six once.
  3. Press Super+W three times, then four more times.
  4. Run `gsettings set org.mate.Marco.general allow-tile-cycling false`, then press Super+W
     twice. Reset it with `gsettings reset`.
  5. Tile pluma with Super+Q and drag its right edge in by a few hundred pixels.
  6. Repeat steps 2 to 5 with xterm, maximised once with Alt+F10 before a press.
  7. Run as part of M8's closing run (173).
- **Expected:** in step 2 the window fills each half and quarter of the work area in turn,
  the panel left clear. In step 3 the right-hand tile is half, a third, then a quarter of
  the screen wide, and then the whole width, three quarters, two thirds, and the window goes
  back to its size and place from before step 2. In step 4 the first press tiles it to the
  right half and the second puts it back. In step 5 the window stays tiled at its new width,
  its left edge and its height unchanged, and a drag of its left or bottom edge changes
  nothing. In step 6 xterm does the same, and a maximised xterm is tiled at once and goes
  back to its size from before it was maximised, as in Marco under *MATE*, the X11 session
  (159).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, for each window's `tile` and
  geometry, and its bindings in force.

### The bindings that move a window to a corner, a side, the centre or another monitor

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito* with two monitors side by side, at different scales (100%
     and 200%, say, set in *Displays*), and the panel along the top of the first. In
     mate-control-center's *System Shortcuts*, under *Window Management*, set the four
     *Move window to … corner* actions, the four *Move window to … side of screen* actions,
     *Move window to center of screen* and the four *Move window to … monitor* actions to
     keys of your choice. Open pluma (native GTK3) and xterm (X11).
  2. With pluma focused on the first monitor, press each corner's binding, each side's and
     the centre's.
  3. Press the binding to the monitor the second one is towards, each corner's binding
     there, and the binding back. Press a binding to a monitor where there is none.
  4. Maximise pluma with Alt+F10, press a corner's binding, then the binding to the other
     monitor and back, and unmaximise it with Alt+F5.
  5. Repeat steps 2 to 4 with xterm, then open a menu of it after each move.
  6. Run as part of M8's closing run (173).
- **Expected:** in step 2 the window's frame, shadow apart, lies against each corner and
  side of the work area, the panel left clear, and in its middle, and the window keeps its
  size; a side's binding moves it one way only. In step 3 the window goes to the other
  monitor, at the same share of the way across and down it and of the same share of its
  size, wholly inside that monitor's work area and drawn sharp at its scale; the corners
  there are that monitor's; the binding back returns it to the first monitor; and with no
  monitor that way nothing moves. In step 4 the maximised window doesn't move for the
  corner, is maximised on the other monitor and back, and goes back to its earlier size and
  place when unmaximised. In step 5 xterm does the same, and its menus open at the window,
  not where it was before the move (160).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, for each window's geometry and
  outputs, the outputs' work areas, and the bindings in force.

## caminito-displays

### The page with MATE's theme, at 100% and at a fractional scale

- **Milestone:** M6
- **Steps:**
  1. Run `tools/dev-nested --bin`, and in another terminal, with `WAYLAND_DISPLAY` set to the
     nested socket, `target/debug/caminito-displays --demo`. The demo has three made-up monitors
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

### caminito-displays shows its window at once

- **Milestone:** M6
- **Steps:**
  1. In the session, with no caminito-displays running, run `tools/first-frame -n 5
     caminito-displays`. Each run shows the window and ends it.
  2. Open *Displays* from the control center, see how long its window takes, leave it open,
     and run `tools/first-frame caminito-displays` again.
- **Expected:** in step 1 the first frame is within about 0.3 s of the start, as ticket 86
  measured it on an RTX 3060 with GTK's Vulkan renderer (265 to 313 ms). The first run after a
  login or a driver update may take 1.4 s, while the driver compiles the renderer's shaders. In
  step 2 the window is there as soon from the control center, and the run shows no frame of
  its own and exits with status 0 in well under a second: the window that was open is the one
  there is.
- **Debug tool:** `tools/first-frame --log /tmp/start caminito-displays` keeps the stamped
  protocol, to see between which messages the time went. `GSK_RENDERER=cairo` before the
  program takes GTK's GPU renderer out of it. A run that ends after 25 s with `Failed to
  register: Timeout was reached` is waiting for an instance that hangs: `pgrep -a
  caminito-displays`.

### caminito-displays applies, reverts and keeps a configuration

- **Milestone:** M6
- **Steps:**
  1. On the tty with two monitors, log into *MATE on Caminito*, keep a copy of
     `~/.config/caminito/layouts.json` if there is one, and run `wlr-randr`.
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
- **Expected:** in step 2 the entry opens caminito-displays, not the Display capplet, and each
  monitor shows its own name for two seconds. Nothing changes until Apply. After step 3 and
  after Revert, `wlr-randr` lists the scales and places of step 1. After Keep it lists what
  the window showed, with no gap and no overlap, and `layouts.json` has them (83). In step 5
  each change is what `wlr-randr` lists, and the display that comes back is on the right of
  the other. In step 6 there's no countdown, the panel and the desktop icons move (81), and
  `layouts.json` has the new primary output. In step 7 the old configuration is back. In step
  8 the window shows one display, then two again, and the scale that wasn't applied is gone.
- **Debug tool:** `wlr-randr`, the `outputs` and `primary_output` of `DumpState`,
  `~/.config/caminito/layouts.json`, and `caminito-displays` run from a terminal for GTK's and
  its own warnings.

### The control center's Displays is caminito-displays, under Caminito only

- **Milestone:** M6
- **Steps:**
  1. In the session, run `tr '\0' '\n' </proc/"$(pgrep -o -x mate-session)"/environ | grep
     '^XDG_DATA_DIRS='`, and `systemctl --user show-environment | grep XDG_DATA_DIRS`.
  2. Open the control center and the panel's *System ▸ Preferences ▸ Hardware* menu, and open
     *Displays* from each.
  3. Log out, log into *MATE*, the X11 session, and open *Displays* there.
- **Expected:** mate-session's `XDG_DATA_DIRS` starts with `/usr/share/caminito/xdg:`, and
  systemd's, if it has one, doesn't name it. Each place has one *Displays*, which opens
  caminito-displays. In X11 MATE it opens the Display capplet (84).
- **Debug tool:** `gtk-launch mate-display-properties`, and `ls
  /usr/share/caminito/xdg/applications`.

### The Night Light tab fades the screen, opens from Preferences, and toggles from a shortcut

- **Milestone:** M9
- **Steps:**
  1. On the tty, log into *MATE on Caminito*. Open the panel's *System ▸ Preferences ▸
     Hardware* menu, and choose *Night Light*.
  2. Turn the switch on. Choose *Constant*, and drag its slider to the warm end and back.
     Choose *Follow the sun*, and move both sliders.
  3. Press *Pause until sunrise*, then *Resume*.
  4. Close the window. Open *Displays* from the control center, then choose *Night Light* in
     the menu again.
  5. In *System Shortcuts*, add a shortcut named *Night Light* with the command
     `caminito-displays --toggle-night-light`, and give it keys. Press them twice, with the
     window closed and then with it open.
- **Expected:** in step 1 the menu's entry has a night-sky icon and opens caminito-displays on
  its *Night Light* tab. In step 2 each change fades the screen over about two seconds, with no
  Apply, and with *Follow the sun* the screen is at the temperature that `DumpState`'s
  `target_temperature` gives. In step 3 the button reads *Resume* beside *Paused until* and the
  next sunrise's time, the screen fades to neutral, and then back. In step 4 the open window
  switches to the *Night Light* tab, and no second window opens. In step 5 each press fades
  Night Light off or on, opens no window, and the open window's switch follows (142).
- **Debug tool:** the `night_light` of `DumpState`, `gsettings list-recursively
  org.mate.Caminito | grep night-light`, and `caminito-displays --toggle-night-light` run from a
  terminal for its warning.

## caminito-about

### About Caminito is in the control center, under Caminito only, and shows the installed build

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` built from the PKGBUILD and installed, log into *MATE on Caminito*.
     Run `caminito --version` and `pacman -Q caminito-git`.
  2. Open the control center and the panel's *System ▸ Preferences ▸ Hardware* menu, and open
     *About Caminito* from each.
  3. Press each of the four links: *Marco*, *Smithay*, *GPL-2.0-or-later* and the repository.
  4. With the window open, open *About Caminito* again, or run `caminito-about`.
  5. Try to resize the window, and press Escape.
  6. Run `ls /usr/share/applications | grep -i caminito-about`. Log out, log into *MATE*, the
     X11 session, and look for *About Caminito* in its control center.
  7. In a checkout, run `cargo build -p caminito -p caminito-about`, then
     `target/debug/caminito --version` and `target/debug/caminito-about`.
- **Expected:** in step 1 `caminito --version` prints `0.1.0 (r<count>.<hash>)` and
  exits, and the part in brackets is the package's `pkgver`. In step 2 the entry is under
  *Hardware* with Caminito's logo, and opens the prototype's compact card (182): the logo on
  the left, *Caminito* in large bold type with the same version string after it on one line,
  *A Wayland compositor for the MATE desktop* under them, and two small dim lines, *Built on
  Marco and Smithay. GPL-2.0-or-later.* and the repository. There is no Close button. In step
  3 each link opens in the browser: Marco's and Smithay's repositories, the GNU page of the
  GPL 2's text, and `https://github.com/JPabloLassala/caminito`. In step 4 the open window is
  raised and there is still one. In step 5 the window doesn't resize, and Escape closes it. In
  step 6 `ls` prints nothing, and X11 MATE has no *About Caminito*. In step 7 both show the
  checkout's `r$(git rev-list --count HEAD).$(git rev-parse --short=7 HEAD)`.
- **Debug tool:** `ls /usr/share/caminito/xdg/applications`, `gtk-launch caminito-about`, and
  `caminito-about` run from a terminal for GTK's warnings.

## caminito-behaviour

### Workspace Behaviour's Hot corners follow and set the keys, and fire with no restart

- **Milestone:** none (CMN-184)
- **Steps:**
  1. With `caminito-git` built from the PKGBUILD and installed, log into *MATE on Caminito*.
     Open the control center, find *Workspace Behaviour* under *Look and Feel*, and open it.
  2. Run `gsettings list-recursively org.mate.Caminito | grep hot-corner`.
  3. Set the bottom-right drop-down to *Show desktop*, run step 2's command again, and rest the
     pointer in the bottom-right corner of each monitor.
  4. With the window open, run `gsettings set org.mate.Caminito hot-corner-top-left overview`,
     and then rest the pointer in the top-left corner.
  5. Set every drop-down back to *None*, and rest the pointer in each corner.
  6. With the window open, run `caminito-behaviour` again, and press Escape.
  7. Log out, log into *MATE*, the X11 session, and look for *Workspace Behaviour* in its
     control center.
- **Expected:** in step 1 the window has one tab, *Hot corners*, with a picture of a screen and
  a drop-down in each of its corners. In step 2 every key is `'none'`, as each drop-down shows.
  In step 3 the key is `'show-desktop'` with no Apply pressed, and the corner shows the desktop
  on every monitor, with Caminito not restarted. In step 4 the top-left drop-down changes to
  *Overview* by itself, and the corner opens the Overview. In step 5 no corner does anything.
  In step 6 the open window is raised and there is still one, and Escape closes it. In step 7
  X11 MATE has no *Workspace Behaviour*.
- **Debug tool:** `gsettings monitor org.mate.Caminito`, `ls /usr/share/caminito/xdg/applications`,
  and `caminito-behaviour` run from a terminal for GTK's warnings.

### The Hot corners' Activation delay is set on the page and counts at once

- **Milestone:** none (CMN-191)
- **Steps:**
  1. In *MATE on Caminito*, open *Workspace Behaviour* with every corner set to *None*.
  2. Set the top-left drop-down to *Show desktop*, and rest the pointer in the top-left corner,
     then take it away and back, several times.
  3. Set *Activation delay* to 1000, and rest the pointer in the top-left corner, counting.
  4. Run `gsettings set org.mate.Caminito hot-corner-activation-delay 400` from a terminal,
     and rest the pointer in the top-left corner.
  5. Set *Activation delay* to 0, and slide the pointer into the top-left corner and out.
  6. Set the top-left drop-down back to *None*.
- **Expected:** in step 1 *Activation delay* reads 150 ms and is greyed out. In step 2 it can
  be changed, and the corner shows the desktop, or stops, at about the same short rest each
  time. In step 3 the key is `1000` with no Apply pressed, and the corner waits about a second.
  In step 4 the spin button reads 400 by itself, and the corner waits less than in step 3,
  with Caminito not restarted. In step 5 the corner fires as soon as the pointer gets there. In step 6 *Activation
  delay* is greyed out again.
- **Debug tool:** `gsettings monitor org.mate.Caminito`, and Caminito's log for the key's
  problems.

## The wallpaper

### caminito-wallpaperd draws each output's wallpaper, and follows the monitors

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` and `mate-settings-daemon-caminito` at `pkgrel` 8 or later built and
     installed, log into *MATE on Caminito* with two monitors. Run `pgrep -a
     caminito-wallpaper` and `gsettings get org.mate.Caminito.wallpaper outputs`.
  2. Note the picture `gsettings get org.mate.background picture-filename` names: on the first
     run it's on both monitors.
  3. Give each monitor a picture of its own: set `first` and `second` to the two identities
     from step 1, and `a` and `b` to two pictures under `/usr/share/backgrounds/mate/`.

     ```sh
     gsettings set org.mate.Caminito.wallpaper same-on-all-outputs false
     out() { echo "org.mate.Caminito.wallpaper.output:/org/mate/caminito/wallpaper/outputs/$1/"; }
     gsettings set "$(out "$first")" kind picture
     gsettings set "$(out "$first")" picture "$a"
     gsettings set "$(out "$second")" kind picture
     gsettings set "$(out "$second")" picture "$b"
     ```

  4. On the first, set `positioning` to each of `fill`, `fit`, `stretch`, `centre` and `tile`
     in turn, with a picture whose shape isn't the monitor's, and `colour` to `'#c03030'`. Then
     set `kind` to `colour`.
  5. Unplug the second monitor, wait five seconds and plug it back. Then run `killall
     caminito-wallpaperd`, and `killall -9 caminito-wallpaperd`.
  6. Run `gsettings set org.mate.Caminito.wallpaper same-on-all-outputs true`.
- **Expected:** in step 1 one `caminito-wallpaperd` runs, no `mate-settings-daemon` draws a
  background (msd's log has `Not activating background plugin under Caminito` with
  `G_MESSAGES_DEBUG=all`), and `outputs` lists both monitors, by make, model and serial where
  they report one. Caja's desktop icons are over the wallpaper on the primary output. In step 3
  each monitor shows its own picture, and each change cross-fades over about a second rather
  than jumping. In step 4 Fill covers the monitor and crops the picture, Fit shows all of it with
  red bars on the short sides, Stretch distorts it to the monitor's shape, Centre shows it at its
  own size in the middle with red around it if it's smaller, Tile repeats it from the top left,
  and Colour is plain red; the picture is sharp, pixel for pixel at the monitor's scale. In
  step 5 the monitor comes back with its own picture, and `DumpState`'s `layer_surfaces` has one
  `mate-background` on each output (88). After each `killall` the wallpaper is back within a
  second or two, and `pgrep` shows one new drawer. In step 6 both monitors cross-fade to the
  same wallpaper.
- **Debug tool:** `busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug
  org.mate.Caminito.Debug DumpState`, `journalctl --user -t caminito-wallpaperd`, and `dconf
  dump /org/mate/caminito/wallpaper/`.

### The wallpaper shows through caja's desktop, and stays still as the Overview opens

- **Milestone:** M9
- **Steps:**
  1. With two monitors connected and `caja-caminito` at `pkgrel` 5 or later, log into *MATE
     on Caminito*. Put a file on the desktop, and drag it to each corner of the primary output.
     Then read the primary output's `layer_surfaces`:
     `busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug org.mate.Caminito.Debug DumpState`.
  2. On the primary output, open the Overview with Super+W and close it, three times. Do the
     same on the other output.
  3. Give each monitor a picture of its own, as in "caminito-wallpaperd draws each output's
     wallpaper, and follows the monitors". Then open the Wallpaper dialog on the primary output
     and click *cosmos*, a dynamic wallpaper.
  4. Log out, log into *MATE* on X11, and change the desktop's picture in *Appearance*. Then
     log into *MATE on Wayfire* and look at the desktop.
- **Expected:** in step 1 the icons are over the wallpaper, and an icon in each corner is all
  on screen, clear of both panels. In the dump, `desktop` starts below the top panel and is
  the output's height less the panels' exclusive zones: for example `y=32, 2560×1384` on a
  1440-row output with a 32 px top panel and a 24 px bottom one. In step 2 the wallpaper
  doesn't move on either output as the Overview opens or closes. Only the icons go. In step 3
  the primary output shows its own picture under the icons, then the dynamic one. Only
  Caminito's cross-fade shows, and no second fade of another picture. In step 4 caja draws the
  desktop's picture on X11 and on Wayfire as before, and fades to the new one on X11
  (CMN-182).
- **Debug tool:** `DumpState` as in step 1, and `journalctl --user -t caminito-wallpaperd`.
  The patch decides at caja's start: a caja started before Caminito owns `org.mate.Caminito`
  draws its own picture.

### caja's "Set as wallpaper" sets every output, and `org.mate.background` follows the primary

- **Milestone:** M9
- **Steps:**
  1. In *MATE on Caminito* with two monitors and caminito-wallpaperd running, give each monitor
     a picture of its own, as in the item above's step 3.
  2. Run `gsettings get org.mate.background picture-filename`, then give the primary monitor
     another picture, `gsettings set "$(out "$first")" picture "$c"`, and set its `positioning`
     to `tile`. Run `gsettings get` for `picture-filename`, `picture-options` and
     `primary-color` again.
  3. Set the primary monitor's `kind` to `colour`, and run the three `gsettings get` again.
  4. In caja, right-click a picture under `~/Pictures` and choose *Set as wallpaper*.
- **Expected:** in step 2 `picture-filename` names the primary monitor's picture, and then
  `$c`, with `picture-options` `'wallpaper'`; neither change reaches the second monitor. In
  step 3 `picture-filename` is `''` and `primary-color` is the primary monitor's colour. In
  step 4 both monitors cross-fade to the picture, and `dconf dump /org/mate/caminito/wallpaper/`
  has it as the `picture` of `all/` and of each monitor, each keeping its `positioning`.
  `journalctl --user -t caminito-wallpaperd` has no line of the drawer taking its own
  write for another's, and the wallpaper doesn't flicker between pictures.
- **Debug tool:** `dconf watch /org/mate/desktop/background/` beside `dconf watch
  /org/mate/caminito/wallpaper/`, and `G_MESSAGES_DEBUG=caminito-wallpaperd` in the drawer's
  environment for its `took org.mate.background's new picture` line.

### The Wallpaper dialog sets a picture or a colour on each output at a click

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` built from the PKGBUILD and installed, log into *MATE on Caminito*
     with two monitors. Open the control center and find *Wallpaper*, and *Appearance*.
     Open *Wallpaper*.
  2. Read the sidebar: a picture of each monitor, *Same on all outputs*, and the five kinds.
     Choose *Picture*, and scroll the grid.
  3. With *Same on all outputs* on, click a picture under *MATE*, then one under *Plasma*
     if `plasma-workspace-wallpapers` is installed. Set *Positioning* to *Fit*, then *Tile*.
  4. Turn *Same on all outputs* off. Select the second monitor's picture in the sidebar,
     choose *Colour*, and click a swatch, then pick another colour with the button beside
     *Another colour*.
  5. Choose *Picture*, press *Add folder…* and choose a folder of your pictures, then press
     *Add picture…* and choose a picture outside it. Click a picture of the folder's.
  6. Choose *Slideshow*.
  7. With the window open, open *Wallpaper* again from the control center, or run
     `caminito-wallpaper`. Then press Escape.
  8. Unplug the second monitor and open the dialog; plug it back in.
  9. Run `ls /usr/share/applications | grep -i caminito-wallpaper`. Log out, log into *MATE*,
     the X11 session, and look for *Wallpaper* in its control center. In a checkout, run
     `cargo run -p caminito-wallpaper --bin caminito-wallpaper -- --demo` anywhere.
- **Expected:** in step 1 *Wallpaper* is under *Look and Feel* beside *Appearance*, with the
  wallpaper icon. In step 2 the monitors' pictures are as wide as each other, in the order
  they stand left to right, each showing its monitor's wallpaper with the monitor's name under
  it. The grid has a heading per source, in this order: *Current picture*, then the user's,
  *MATE* and *Plasma*, with one picture per Plasma package. The window doesn't stall while the
  thumbnails come in. In step 3 each click is on both monitors at once, with the cross-fade,
  and the sidebar's pictures show it, the positioning included; there is no Apply button. In
  step 4 the first monitor keeps its picture and the second turns to the colour at each click.
  In step 5 the folder is a heading of its own, named after it, with its pictures; the
  picture is under *Your pictures* and is on the second monitor at once, as the folder's is
  after it. In step 6 it says it isn't there yet, and nothing on the desktop changes. In
  step 7 the open window is raised and there is still one, and Escape closes it. In step 8
  the dialog shows one monitor's picture as a plain preview, with no *Same on all outputs*,
  and the monitor plugged back in comes back with its own wallpaper. In step 9 `ls` prints
  nothing, X11 MATE has no *Wallpaper*, and the demo shows two made-up monitors and changes
  nothing outside the window.
- **Debug tool:** `dconf dump /org/mate/caminito/wallpaper/`, `dconf watch
  /org/mate/caminito/wallpaper/` while clicking, and `caminito-wallpaper` run from a
  terminal for GTK's warnings.

### A Plasma pair turns dark as Night Light warms, and a timed XML is on its picture for the hour

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` built and installed, `mate-screensaver` installed (it has
     `/usr/share/backgrounds/cosmos/`), and a Plasma package with `contents/images_dark/`
     under `/usr/share/wallpapers` (`plasma-workspace-wallpapers` has several), log into
     *MATE on Caminito*. Turn Night Light on with the sun schedule: `gsettings set
     org.mate.Caminito night-light-enabled true` and `gsettings set org.mate.Caminito
     night-light-schedule sun`.
  2. Open *Wallpaper* and choose *Dynamic*. Read the grid, then click a Plasma pair.
  3. Put the sun where the pair is light, then at the horizon, then below civil twilight, by
     giving Caminito a place on the equator whose solar time is 17:00, 18:00 and 18:30 in
     turn, as minutes of the day. A minute after each, read the `Sun` property and look at the
     desktop.

     ```sh
     at() { echo $(( ($1 - $(date -u +%-H) * 60 - $(date -u +%-M)) / 4 + 540 )); }
     place() { gsettings set org.mate.Caminito night-light-last-location "(0.0, $(( $(at $1) % 360 - 180 )).0)"; }
     sun() { busctl --user get-property org.mate.Caminito /org/mate/Caminito org.mate.Caminito Sun; }
     place 1020; sleep 70; sun
     place 1080; sleep 70; sun
     place 1110; sleep 70; sun
     ```

  4. In the dialog, click *cosmos*. Work out which of its pictures the hour is on: numbering
     its `<static>` pictures from 0, the one shown now is

     ```sh
     python3 -c 'import datetime as d; print(int((d.datetime.now() - d.datetime(2009, 8, 4)).total_seconds() % 16200 // 1800))'
     ```

     Leave it until that number changes, half an hour at most.
  5. Run `gsettings reset org.mate.Caminito night-light-last-location`, and turn Night Light
     off if it was.
- **Expected:** in step 2 the grid has a heading *MATE* with *cosmos* and a heading *Plasma*
  with each pair, each thumbnail its day picture on the left and its night picture on the
  right; under the grid are *Add…*, a line, and *Positioning*. The click cross-fades the
  desktop to the pair, and the line says it follows the sun. In step 3 the `Sun` elevation is
  about 15°, then about 0°, then about −7°. At 15° the desktop is the light picture and Night
  Light is near its noon temperature; at 0° it is halfway between the light and dark pictures
  as Night Light warms; at −7° it is the dark picture, and Night Light is at its night
  temperature. Each change fades over about a second rather than jumping. In step 4 the line
  says it follows the clock, and the desktop and the sidebar show the picture numbered;
  within a minute of the number changing, the desktop cross-fades to the next.
  `gsettings get org.mate.background picture-filename` names the picture shown, and for a
  pair part way, the one that shows most.
- **Debug tool:** `dconf dump /org/mate/caminito/wallpaper/` for `kind='dynamic'` and its
  `dynamic`, `journalctl --user -t caminito-wallpaperd` for a dynamic wallpaper that can't be
  read, and the `Sun` property's `location-source`, which is `stored` in step 3.

### A slideshow changes every 5 minutes with a cross-fade, and differs on two monitors

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` built and installed, log into *MATE on Caminito* with two monitors.
     Make a folder of eight pictures of clearly different scenes, say `~/Pictures/Slides`.
  2. Open *Wallpaper*, leave *Same on all outputs* on, and choose *Slideshow*. Press the folder's
     button and choose `~/Pictures/Slides`. Set *Change every* to *5 minutes*.
  3. Look at both monitors, then watch them across the next whole 5 minutes of the clock (as
     10:05), and the one after.
  4. Copy one more picture into the folder, and take one out, then wait for the next change and
     read the dialog's count again.
  5. Tick *In order, not at random*, and watch two more changes.
  6. Move every picture out of the folder, and wait for the next minute; then rename the folder.
- **Expected:** in step 2 the pane shows the folder's name on its button and how many pictures
  it has, the desktop cross-fades to a picture of the folder on each monitor at once, and the
  sidebar shows each monitor's. In step 3 the two monitors show different pictures of the folder,
  and both change together on the whole 5 minutes, each with a cross-fade of about a second, and
  never to the picture the other has. Over those changes a monitor seldom comes back to a
  picture: each round of eight shows every picture once, and only where one round ends and the
  next begins can a picture come twice in a row. In step 4 the count follows within a minute,
  the new picture can come up, and the one taken out doesn't. In step 5 each monitor goes
  through the pictures in the order of their names, one monitor a picture ahead of the other
  (which one goes by the monitors' identities, not by where they stand). In step 6 both
  monitors fade to the wallpaper's colour within a minute, and the pane says the folder has no
  pictures, then that it isn't there; nothing crashes, and `pgrep caminito-wallpaperd` still
  shows one drawer.
- **Debug tool:** `dconf dump /org/mate/caminito/wallpaper/` for `kind='slideshow'`,
  `slideshow`, `slideshow-interval` and `slideshow-in-order`, and
  `G_MESSAGES_DEBUG=caminito-wallpaperd` in the drawer's environment for `the slideshow … has no
  pictures`.

### A HEIC dynamic wallpaper of your own is on its noon picture at noon and its night one at night

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` built and installed, and `libheif` installed with its `heif-dec`
     (Arch's `libheif` has it; Debian's is in `libheif-examples`), log into *MATE on
     Caminito*. Have a dynamic HEIC of your own, as macOS's *Mojave* or one from
     <https://dynamicwallpaper.club>, at `~/Pictures/dynamic.heic`.
  2. Open *Wallpaper*, choose *Dynamic*, press *Add…* and choose the file.
  3. Put the sun at noon, then at midnight, by giving Caminito a place on the equator whose
     solar time is 12:00, then 00:00, as in the item above. A minute after each, read the
     `Sun` property and look at the desktop and the sidebar.

     ```sh
     at() { echo $(( ($1 - $(date -u +%-H) * 60 - $(date -u +%-M)) / 4 + 540 )); }
     place() { gsettings set org.mate.Caminito night-light-last-location "(0.0, $(( $(at $1) % 360 - 180 )).0)"; }
     sun() { busctl --user get-property org.mate.Caminito /org/mate/Caminito org.mate.Caminito Sun; }
     place 720; sleep 70; sun
     place 0; sleep 70; sun
     ```

  4. Run `gsettings reset org.mate.Caminito night-light-last-location`, and `ls
     ~/.cache/caminito-wallpaper/heic/`.
- **Expected:** in step 2 the file is under *Your dynamic wallpapers*, grey for a few
  seconds while its pictures are decoded, then its day picture on the left and its night
  picture on the right. The desktop shows its colour for those seconds, then cross-fades to
  the file's picture for the sun now, and the line says it follows the sun (or the clock,
  for an older file keyed to times of the day). In step 3 the `Sun` elevation is above 60°,
  then below −60°: at noon the desktop is the
  file's midday picture, as macOS shows it at noon, and at midnight its night picture, in
  the sidebar as on the desktop. In step 4 there is one folder, named for the file, with a
  JPEG per picture.
- **Debug tool:** `G_MESSAGES_DEBUG=caminito-wallpaperd` for *decoding* and *decoded*,
  `journalctl --user -t caminito-wallpaperd` for a HEIC that can't be read or decoded, and
  `heif-info ~/Pictures/dynamic.heic` for its pictures.

### Each picture of the day shows with its credit, can be kept, and stays offline

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` built and installed, log into *MATE on Caminito* with the network
     on. Open *Wallpaper* and choose *Picture of the day*.
  2. Read the pane, then click each source in turn: *NASA Astronomy Picture of the Day*,
     *Bing* and *Wikimedia Commons*. Give each up to a minute to come in.
  3. With *Wikimedia Commons* chosen, press *Keep this picture*, then choose *Picture*.
  4. Choose *Picture of the day* again, and set *Positioning* to *Fit*.
  5. Turn the network off (`nmcli networking off`, or unplug it). Log out and back in, then
     suspend and wake the machine, and click *Bing* and *NASA* again. Turn the network back on.
  6. Run `ls ~/.cache/caminito-wallpaper/picture-of-the-day/`.
- **Expected:** in step 2 the pane lists the three sources, each with a line on it, the
  chosen one highlighted, and under them today's title and credit, with the credit's day.
  Each click cross-fades the desktop and the sidebar to that source's picture of today, as
  its web page shows it: <https://apod.nasa.gov/apod/>, <https://www.bing.com> and
  <https://commons.wikimedia.org/wiki/Main_Page>. On a NASA day that is a video, NASA's
  stays the last picture it had, or the colour on a first run. In step 3 the line beside the
  button names the copy, `~/Pictures/Wallpapers` has it, and the Picture grid has it under
  *Your pictures*. In step 4 the picture is fitted, with bars of the colour. In step 5 every
  output keeps the picture it had through the log-in and the wake, and each source clicked
  shows its last picture, title and credit; within a minute of the network coming back, any
  that was out of date is fetched. In step 6 there is one picture and one `.about` per
  source fetched.
- **Debug tool:** `journalctl --user -t caminito-wallpaperd`, which says why a fetch failed
  or why the day's isn't a picture, `G_MESSAGES_DEBUG=caminito-wallpaperd` for each picture
  kept, and `cat ~/.cache/caminito-wallpaper/picture-of-the-day/*.about`.

### Each monitor's lock screen shows its own wallpaper, and the lock comes up without the drawer

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` built and installed, log into *MATE on Caminito* with both monitors
     connected, and on the session bus run `gsettings set org.mate.Caminito locker-command
     "['caminito-locker', '--verbose']"`.
  2. Open *Wallpaper*, turn *Same on all outputs* off, and give the first monitor a picture
     and the second a colour. Lock with *System* ▸ *Lock Screen*, and unlock.
  3. Give the second monitor a Plasma pair under *Dynamic*, lock, and unlock.
  4. Run `pkill -KILL caminito-wallpaperd`, and lock at once. Unlock.
  5. Run `pkill -STOP caminito-wallpaperd`, lock, and unlock. Run `pkill -CONT
     caminito-wallpaperd`. Reset `locker-command`.
- **Expected:** in step 2 each monitor's lock screen shows that monitor's wallpaper, the
  picture as its *Positioning* places it on the desktop and the colour plain, under the
  unlock dialog on the monitor under the pointer. In step 3 the second monitor's lock screen
  shows the picture of the pair that the desktop shows more of then. In step 4 the lock
  comes up as quickly as before, each monitor with the wallpaper it had when the drawer was
  killed. In step 5 the lock comes up with the drawer stopped, the same way.
- **Debug tool:** `cat $XDG_RUNTIME_DIR/caminito-wallpaper/lock-screen`, the drawer's file,
  with each monitor's make, model and place, which the locker matches against GTK's. The
  journal's `caminito-locker` lines say where each monitor's came from: `Wallpaper of <connector>
  from the wallpaper file: …`, or `No wallpaper file`, `The wallpaper file has no …` or `… names
  a picture that is gone` before `Background from org.mate.background`.

## The control center

### Windows and System Shortcuts open under Caminito, and name it

- **Milestone:** M8, for step 4, which applies a changed shortcut at once (148). It's run in
  M8's closing run (173). The other steps are M6's.
- **Steps:**
  1. With `mate-control-center-caminito` 1.29.1-5 or later installed, and `caminito-git` at a
     build that has the `50-caminito-*.xml` manifests (`ls
     /usr/share/mate-control-center/keybindings`) and the launcher (`ls
     /usr/share/caminito/xdg/applications/mate-keybinding.desktop`), log into *MATE on
     Caminito*.
  2. Open *Windows* from the control center, with a window that Caminito decorates beside it.
     On *Behaviour*, set the titlebar's double-click action to *Toggle Shade*, double-click that
     window's titlebar, and set it back. On *Placement*, move the titlebar buttons to the
     left and back.
  3. Find *System Shortcuts* in the control center, under *Hardware*, and in the panel's
     menu, under *System*, *Preferences*, *Hardware*. Open it from the control center, and
     read its window's title. Find *Window Management*, and *Desktop*'s *Show the panel's "Run
     Application" dialog box*. Then close the dialog and the control center, since a second
     control center only raises the first, and from a terminal run `LANGUAGE= LC_ALL=
     LANG=es_ES.UTF-8 mate-control-center`, with that locale generated (`locale -a`), and
     find and open the dialog there.
  4. Click the shortcut of *Toggle maximization state*, press Super+Up, and run:

     ```sh
     gsettings get org.mate.Marco.window-keybindings toggle-maximized
     ```

     With a window focused, press Super+Up twice, then Alt+F10. Then click the shortcut
     again, press Backspace, run the command again, and press Super+Up and Alt+F10. Last,
     run `gsettings reset org.mate.Marco.window-keybindings toggle-maximized` and press
     Alt+F10.
  5. Log out, log into *MATE*, the X11 session, and open both dialogs there, the second one
     as *Keyboard Shortcuts*. Close the control center, run `LANGUAGE= LC_ALL=
     LANG=es_ES.UTF-8 mate-control-center`, and open it from there too. Then log into the Wayfire session and open both.
- **Expected:** in step 2 the dialog opens, with no *The current window manager is
  unsupported*, and it offers Alt and Super for moving a window. The window shades at the
  double click, and each window's buttons move as the setting does, with the dialog still
  open (06). In step 3 the control center and the menu each list *System Shortcuts* once, and
  no *Keyboard Shortcuts*, and the dialog's title is *System Shortcuts*. In Spanish the
  launcher and the title are both *Atajos del sistema* (183). The dialog opens without a
  crash, and lists Caminito's window management actions once each, with the shortcuts Marco's defaults give them: Alt+Tab, Alt+F4,
  Ctrl+Alt+D. In step 4 the key is a string that names Super and Up, `'<Mod4>Up'` or
  `'<Super>Up'`, and then `'disabled'`: the dialog writes the key. **The shortcut applies at
  once**, with no restart (148): the first Super+Up maximises the window and the second
  restores it, while Alt+F10 reaches the window and maximises nothing. Once it's disabled,
  neither maximises, and after the reset Alt+F10 maximises the window again. In X11 MATE both dialogs are as they were, with Marco's shortcuts
  listed, since the variables aren't read on X11: the launcher and the title are *Keyboard
  Shortcuts*, and in Spanish both are *Atajos de teclado*, as stock. In the Wayfire session *Windows* says the
  window manager is unsupported, as the stock one does, and *Keyboard Shortcuts* opens, where
  the stock one crashed, and lists no window management actions (101, ADR 0011).
- **Debug tool:** `mate-window-properties` and `mate-keybinding-properties` run from a
  terminal, `echo "$MATE_WM_NAME $MATE_WM_KEYBINDINGS"` there, `gsettings monitor
  org.mate.Marco.general`, `DumpState`'s `bindings`, which lists each combination in force
  with its key, and `coredumpctl` for a crash.

### Mouse and Keyboard open under Caminito, and apply live

- **Milestone:** M6
- **Steps:**
  1. With `mate-control-center-caminito` installed, on a machine with a touchpad and a mouse,
     log into *MATE on Caminito*.
  2. Open *Mouse* from the control center. On *Mouse*, move *Acceleration* to each end and
     move the mouse after each. On *Touchpad*, turn *Enable mouse clicks with touchpad* off,
     tap the touchpad, turn it on and tap again. Move the touchpad's *Acceleration* to each end
     and move a finger on it after each.
  3. Open *Keyboard* from the control center. On *Layouts*, with *English (US)* there (add it
     if it isn't), add *Spanish* beside it, and in *Options…* ▸ *Switching to another layout*, tick *Alt+Shift*. In a native
     app (`mate-terminal`) and an X11 one (`xterm`), type `;`, press Alt+Shift and type `;`
     again.
  4. On *General*, set *Repeat Keys*' *Delay* short and *Speed* fast, and hold a key down in
     `mate-terminal`; then set them long and slow, and hold it again.
  5. Log out, log into *MATE*, the X11 session, and open both dialogs there. Then log into the
     Wayfire session and open both.
- **Expected:** in steps 2 and 3 each dialog opens, with no warning, on XWayland
  (`xprop` can pick it), and *Mouse* has a *Touchpad* tab. The pointer and the touchpad
  speed up and slow down as the sliders move, a tap clicks only while tap-to-click is on,
  and each change applies with the dialog still open (53). `;` types `;`, then `ñ` after
  Alt+Shift, in both apps (54). The repeat starts sooner and runs faster in step 4, then later
  and slower. In X11 MATE both dialogs are as the stock ones, and *Mouse* has *Touchpad* only
  where the X server lists a touchpad. In the Wayfire session both do what the stock ones do, which is to crash at start with a
  segmentation fault, in an X11 call on a Wayland display:
  the gate is `MATE_WM_NAME=Caminito` (102, ADR 0011). On a machine with no touchpad,
  *Touchpad* is shown under Caminito anyway, and changes nothing (06).
- **Debug tool:** `mate-mouse-properties` and `mate-keyboard-properties` run from a terminal,
  `echo "$MATE_WM_NAME $GDK_BACKEND"` there, `gsettings monitor
  org.mate.peripherals-touchpad`, `gsettings monitor org.mate.peripherals-keyboard-xkb.kbd`,
  `xkbcomp "$DISPLAY" - | grep xkb_symbols`, and `coredumpctl` for a crash.

### System Info's Windowing System row names Caminito

- **Milestone:** M6
- **Steps:**
  1. With `mate-control-center-caminito` installed, log into *MATE on Caminito*. Open *System
     Info* from the control center, under *Hardware*, and read its *Windowing System* row.
  2. Close it, and run `GDK_BACKEND=x11 mate-system-info` in a terminal.
  3. Run `env -u MATE_WM_NAME mate-system-info`, and then `env -u MATE_WM_NAME
     GDK_BACKEND=x11 mate-system-info`.
  4. Log out, log into *MATE*, the X11 session, and open *System Info* there. Then log into
     the Wayfire session and open it.
- **Expected:** the row reads `Caminito (Wayland)` in step 1 and `Caminito (X11)` in step 2,
  where the dialog is an XWayland window (`xprop` can pick it). In step 3 it reads `Wayland`,
  then `X11`, as the stock dialog does. In X11 MATE it reads `X11`, and in the Wayfire session
  `Wayland`: the gate is `MATE_WM_NAME` alone (181, ADR 0011).
- **Debug tool:** `echo "$MATE_WM_NAME $GDK_BACKEND"` in the terminal, and
  [windowing-row.sh](../../.scratch/issues/181-checks/windowing-row.sh), which prints the row
  of a `mate-system-info` built from the series.

### Appearance has no Background tab under Caminito, and the desktop's menu opens Wallpaper

- **Milestone:** M9
- **Steps:**
  1. With `mate-control-center-caminito` 1.29.1-7 or later and `caminito-git` with the
     Wallpaper dialog installed (`ls
     /usr/share/caminito/xdg/applications/caminito-wallpaper.desktop`), log into *MATE on
     Caminito*. Open *Appearance* from the control center and read its tabs. Scroll the mouse
     wheel over the tabs, both ways.
  2. Close it. Right-click the desktop and choose *Change Desktop Background*.
  3. Close the dialog. Run `mate-appearance-properties --show-page=background`, then
     `mate-appearance-properties` with a picture's path, closing each window it opens.
  4. Run `XDG_DATA_DIRS=/usr/local/share:/usr/share mate-appearance-properties
     --show-page=background`, then the same with a picture's path in place of
     `--show-page=background`, then `env -u MATE_WM_NAME mate-appearance-properties`.
  5. Log out, log into *MATE*, the X11 session. Open *Appearance*, choose its *Background* tab
     and click another picture. Then right-click the desktop and choose *Change Desktop
     Background*.
- **Expected:** in step 1 the tabs are *Theme*, *Fonts* and *Interface*, with no
  *Background*, and the wheel never lands on a blank page. In step 2 the *Wallpaper* dialog
  opens, and no Appearance window does. In step 3 each command opens *Wallpaper*, and nothing
  on the desktop changes. In step 4 the first two commands, which can't find Caminito's
  launcher, open Appearance at *Theme*, without *Background*, and the desktop keeps its
  wallpaper; the third opens it with *Background*, as stock. In step 5 *Background* is there, the desktop takes the picture, and
  the menu opens Appearance at *Background*: the gate is `MATE_WM_NAME` alone (219, ADR 0011).
- **Debug tool:** `mate-appearance-properties` run from a terminal, for its `Could not open
  the Wallpaper dialog` warning, and `echo "$MATE_WM_NAME $XDG_DATA_DIRS"` there.

### The Window switcher shows live thumbnails

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito*. Open a terminal, a file manager and a video player, and play a
     video. Minimise the file manager.
  2. Tap Alt+Tab quickly, twice.
  3. Hold Alt and press Tab. Keep Alt held for a few seconds, then press Tab, Shift+Tab and
     the arrow keys. Release Alt with the file manager selected.
  4. Hold Alt and press Tab, then Escape. Hold Alt and press Tab again, and click a thumbnail.
  5. Hold Alt and press \`, and then, with Ctrl held too, Tab.
  6. With two monitors, move the pointer to the other one and hold Alt+Tab.
  7. Run `gsettings set org.mate.Marco.general compositing-fast-alt-tab true`, hold Alt+Tab,
     and set the key back.
- **Expected:** in step 2 the focus goes to the window used before and back, and no popup
  shows. In step 3 the popup appears in the middle of the output the pointer is on, in the GTK
  theme's colours and font, with a thumbnail of each window, most recently used first and the
  minimised file manager last. The video goes on playing in its thumbnail. The file manager's
  thumbnail shows it as it was when it was minimised. Each thumbnail has its application's
  icon in a corner, and the selected window's title is under the grid. Each key moves the
  selection, and nothing shows on the windows themselves. Releasing Alt restores the file
  manager and gives it the focus. In step 4 Escape leaves the focus and the stacking as they
  were, and the click gives the focus to the window clicked. In step 5 Alt+\` lists only the
  focused application's windows, and Ctrl+Alt+Tab is *Ctrl+Alt+Tab and Ctrl+Alt+Esc reach the
  panel and the desktop*'s to look at. In step 6 the popup is on the pointer's monitor. In step 7
  the grid shows icons with no thumbnails (108, ADR 0016).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, whose `switcher` member has the list,
  the selection, the popup's output and its cells while the popup shows, and `gsettings
  monitor org.mate.Marco.general`.

### The visual bell flashes

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito*. Open `mate-terminal`, a native Wayland window, and `xterm`,
     an XWayland one, side by side, with two monitors if there are.
  2. Run `gsettings set org.mate.Marco.general visual-bell true` and `gsettings set
     org.mate.Marco.general visual-bell-type fullscreen`. Run `printf '\a'` in each terminal.
  3. Run `gsettings set org.mate.Marco.general visual-bell-type frame_flash`, and `printf '\a'`
     in each terminal again. Run `sleep 2; printf '\a'` in xterm and focus mate-terminal
     before it rings.
  4. Run `gsettings set org.mate.Marco.general visual-bell false`, and `printf '\a'` in each.
  5. Run as part of M8's closing run (173).
- **Expected:** in step 2 every monitor flashes white, briefly, at each bell. In step 3 the
  terminal's own frame flashes, drawn for a moment as though its focus had changed, and
  nothing else does, the delayed bell flashing xterm's frame though it's unfocused; a
  terminal drawing its own titlebar flashes the monitor it's on instead. In step 4 nothing
  flashes (169).
- **Debug tool:** `gsettings monitor org.mate.Marco.general`, and `DumpState`'s `bell`, whose
  `count` goes up at each bell and whose `last` names its source and window.

### The bell and the workspace switches sound

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito*, with the speakers on. Open `mate-terminal`, a native
     Wayland window, and `xterm`, an XWayland one. In mate-control-center's *Sound*, on
     *Sound Effects*, choose a sound theme and tick *Enable window and button sounds*.
  2. Run `printf '\a'` in each terminal.
  3. Press Ctrl+Alt+Right, Ctrl+Alt+Left and, with the workspaces in two rows, Ctrl+Alt+Down
     and Ctrl+Alt+Up. Click another workspace in the panel's workspace switcher.
  4. Choose another sound theme in *Sound*, and repeat steps 2 and 3.
  5. Untick *Enable window and button sounds*, and repeat steps 2 and 3.
  6. Run `gsettings set org.mate.Marco.general audible-bell false`, and `printf '\a'` in
     each terminal. Set it back to `true`, run `gsettings set org.mate.peripherals-keyboard
     bell-mode off`, and ring again. Set `bell-mode` back to `on`.
  7. Run as part of M8's closing run (173).
- **Expected:** in step 2 each terminal's bell plays the theme's bell. In step 3 each switch
  plays the theme's sound for its direction, the pager's click too. In step 4 the sounds are
  the new theme's, with no restart. In step 5 the bell still plays and the switches are
  silent. In step 6 neither bell plays, with either key (170).
- **Debug tool:** `gsettings monitor org.mate.sound`, `DumpState`'s `bell`, the journal
  (`journalctl --user -b -t caminito`), where a sound libcanberra can't play is logged, and
  `canberra-gtk-play -i bell-window-system` or `-i desktop-switch-right`, which plays the
  same sound without Caminito.

## Bluetooth

These need a Bluetooth adapter and the devices themselves, which no mock stands in for. They are
the developer's to run (CMN-105).

### A keyboard pairs with a PIN

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` installed, log into *MATE on Caminito* with a Bluetooth adapter. Run
     `pgrep -a caminito-bluetooth`. Put a Bluetooth keyboard that asks for a PIN in pairing mode.
  2. Click the Bluetooth icon in the top panel, beside the notification area, and choose
     *Add New Device…*.
  3. Click the keyboard in the list. When the dialog shows a code, type it on the keyboard and
     press Enter.
  4. Type in a terminal on the keyboard. Click the icon again.
- **Expected:** in step 1 `caminito-bluetooth --background` is running. In step 2 the settings
  window opens on Add New Device and lists the keyboard by its name. In step 3 the dialog shows
  the code to type, and closes after Enter. The keyboard moves to the paired devices and connects.
  In step 4 the keys arrive, and the popup lists the keyboard under the connected devices, with
  its battery level if it reports one.
- **Debug tool:** `busctl --system tree org.bluez`, `bluetoothctl devices Paired`, and
  `journalctl --user -b -t caminito-bluetooth`.

### A phone pairs with a confirmation, one that the phone starts too

- **Milestone:** M9
- **Steps:**
  1. Open the settings window from *Bluetooth* in the control center, choose *Add New Device*,
     and click the phone, with its Bluetooth screen open.
  2. Compare the number the dialog shows with the phone's. Choose *Yes* in the dialog and accept on
     the phone.
  3. Choose *Forget* on the phone's page. Then, from the phone, search for the computer and pair
     with it. This time the window is closed.
- **Expected:** in step 2 both show the same number, and the phone moves to the paired devices.
  In step 3 a dialog asks whether to allow the pairing, though no window of caminito-bluetooth was
  open, and *Yes* pairs the phone, trusted.
- **Debug tool:** `bluetoothctl devices Paired` and `bluetoothctl info <address>`, whose `Trusted`
  line is `yes` after each pairing.

### A headset connects and disconnects from the popup

- **Milestone:** M9
- **Steps:**
  1. Pair a headset in the settings window. Turn it off, then on.
  2. Open the popup, click the headset's row, and choose *Connect*. Play a sound in *Sound*'s
     test, on the headset.
  3. Choose *Disconnect*.
  4. Hover over the icon in each of the three states: Bluetooth off, on with nothing connected,
     and connected.
- **Expected:** in step 2 the row moves to the connected devices, with its battery level where the
  headset reports it, and the sound plays on the headset (PipeWire's profile is not Caminito's
  to choose). In step 3 the row goes back to the paired ones. In step 4 the icon differs in each
  state, and the tooltip names the connected devices and their batteries.
- **Debug tool:** `bluetoothctl info <address>`, `wpctl status`.

### Off lasts across a reboot, and the icon hides with no adapter

- **Milestone:** M9
- **Steps:**
  1. Open the popup and turn Bluetooth off with its switch. Run `rfkill list bluetooth`.
  2. Reboot, log in again, and run `rfkill list bluetooth`. Open the popup.
  3. Turn it on with the switch, or press the airplane-mode key twice.
  4. Unplug a USB adapter, or run `sudo rmmod btusb`, and then plug it in again.
- **Expected:** in step 1 the adapter is `Soft blocked: yes`, and the popup says Bluetooth is off.
  In step 2 it's still blocked, and the popup says so. In step 3 it's on again, and the devices
  that were connected reconnect. In step 4 the icon is gone while there is no adapter, and
  comes back with it.
- **Debug tool:** `rfkill list`, `systemctl status systemd-rfkill`, and `bluetoothctl show`.

### A fresh user's top panel has the applet, under X11 MATE too

- **Milestone:** M9
- **Steps:**
  1. Make a user with no panel settings: `sudo useradd -m btfresh`, and log in as them to
     *MATE on Caminito*.
  2. Look at the top panel. Open *Control Center*.
  3. Log out. Log in as the same user to *MATE*, the X11 session, with its settings reset
     (`mate-panel --reset`, or a second new user).
- **Expected:** in step 2 the Bluetooth icon is in the top panel, beside the notification area,
  and *Bluetooth* is under Hardware in the control center. In step 3 the same panel and applet,
  and the popup and settings window work as in the Session.
- **Debug tool:** `dconf dump /org/mate/panel/objects/`, and `journalctl --user -b -t mate-panel`.

## Power applet

These need real hardware and a Session, with its real idle and mate-session, which no mock
stands in for. They are the developer's to run.

### A fresh user's top panel has the applet, and X11 MATE shows one battery icon

- **Milestone:** M9
- **Steps:**
  1. On a laptop, make a user with no panel settings: `sudo useradd -m pwfresh`, and log in as
     them to *MATE on Caminito*.
  2. Look at the top panel, and hover over the battery icon.
  3. Log out. Log in as the same user to *MATE*, the X11 session. Run
     `gsettings get org.mate.power-manager icon-policy`.
- **Expected:** in step 2 the battery icon is in the top panel beside the Bluetooth icon, and its
  tooltip has the charge and the time left. In step 3 the same panel, with one battery icon only:
  mate-power-manager's isn't in the notification area, and the command prints `'never'`.
- **Debug tool:** `dconf dump /org/mate/panel/objects/`, and `upower -d`.

### Unplugging and plugging in again

- **Milestone:** M9
- **Steps:**
  1. With the charger in, hover over the icon, and open the popup.
  2. Unplug the charger. Wait ten seconds, then hover and open the popup again.
  3. Plug it in again.
- **Expected:** in step 1 the icon is the charging glyph, the tooltip says *Full in …* (*Charging*
  while UPower has no estimate, or *Plugged in* when full), and the laptop's battery row says *Charging* or *Fully charged*. In
  step 2 the glyph loses its charging mark, the tooltip says *… left*, and the row says
  *Discharging*. In step 3 they change back within a few seconds.
- **Debug tool:** `upower --monitor-detail`.

### A profile change is seen in `powerprofilesctl`

- **Milestone:** M9
- **Steps:**
  1. With power-profiles-daemon (or tuned-ppd) running, open the popup, and run
     `powerprofilesctl`.
  2. Click *Power Saver*, then run `powerprofilesctl get`.
  3. Run `powerprofilesctl set performance` (or `balanced`, with no Performance tile), with the
     popup open.
  4. Run `systemctl stop power-profiles-daemon` (or `tuned`), and open the popup.
- **Expected:** in step 1 the popup has a tile for each profile `powerprofilesctl` lists, and the
  active one is pressed. In step 2 it prints `power-saver`, and that tile is pressed. In step 3
  the popup's pressed tile follows. In step 4 the tiles are gone. Start the daemon again after.
- **Debug tool:** `powerprofilesctl`, and `busctl --system introspect
  org.freedesktop.UPower.PowerProfiles /org/freedesktop/UPower/PowerProfiles`.

### The charge limit

- **Milestone:** M9
- **Steps:**
  1. On a laptop whose battery supports a charge limit (`upower -i
     /org/freedesktop/UPower/devices/battery_BAT0` has `charge-threshold-supported: yes`), open
     the popup and click the ⋮ button of the laptop's battery.
  2. Turn on *Limit charging to 80%* (or to the firmware's own end threshold, where
     `charge-end-threshold` is below 100), and answer polkit if it asks. Leave the charger in until
     the battery is above 80%, or start above it.
  3. Turn it off.
- **Expected:** in step 1 the popover has the battery's health and the switch. In step 2 `upower
  -i` says `charge-threshold-enabled: yes`, the row says *· limit 80%* (or the firmware's
  threshold), and the battery doesn't charge past it (*Not charging*). In step 3 it charges again to full. On a battery that doesn't
  support it, the popover has the health only.
- **Debug tool:** `upower -i`, and
  `cat /sys/class/power_supply/BAT0/charge_control_end_threshold`.

### Stay awake keeps the screen from blanking past `idle-delay`, and shows what else does

- **Milestone:** M9
- **Steps:**
  1. In a *MATE on Caminito* session, run `gsettings set org.mate.session idle-delay 1`. Open the
     Power applet's popup and turn on *Stay awake*. Run `gdbus call --session --dest
     org.gnome.SessionManager --object-path /org/gnome/SessionManager --method
     org.gnome.SessionManager.GetInhibitors`.
  2. Close the popup and leave the keyboard and the mouse alone for two minutes.
  3. Move the mouse. Open the popup, and turn *Stay awake* off. Leave everything alone for two
     minutes more.
  4. Set `idle-delay` back. Play a video in a browser, and open the popup.
  5. Turn *Stay awake* on again, and run `pkill -x mate-panel`. Open the popup of the new panel.
- **Expected:** in step 1 the switch is on, the popup lists a line *You*, *Stay awake is on* under
  a separator, and `GetInhibitors` names one inhibitor. In step 2 the screen stays on, and
  doesn't lock or blank. In step 3 the screen blanks after one minute (Caminito's idle, 128),
  and wakes at the mouse. In step 4 the popup lists the browser with its icon and the reason it
  gave, and the line disappears when the video stops. In step 5 the switch is off again, and
  `GetInhibitors` lists no inhibitor of the applet.
- **Debug tool:** `gdbus call` as in step 1, and `journalctl --user -b -t caminito` for the idle
  state.

## Thermal applet

These need real sensors and fans, which the fake hwmon trees of CMN-99's check script only stand
in for. They are the developer's to run.

### The readings match `sensors`

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` installed, log into *MATE on Caminito* as a user with no panel
     settings, as in the Bluetooth applet's item above. Look at the top panel.
  2. Right-click the top panel, choose *Add to Panel…*, and add *Temperatures and Fans*.
  3. Run `watch -n 2 sensors` in a terminal beside the panel. Hover over the applet.
- **Expected:** in step 1 the panel has no thermometer. In step 2 the list has *Temperatures and
  Fans* with a thermometer icon, and the applet shows the CPU's temperature (coretemp's
  *Package id 0* or k10temp's *Tctl*, or else the hottest reading), and the hottest GPU reading
  after it when there is a GPU, over the fastest fan's RPM, or *Fans Off* when every fan reads
  0. A computer with no fans has no second line. In step 3 each number agrees with `sensors` to
  within one reading, 2 s, and the tooltip names the same temperatures, and every fan with its
  speed or *Stopped*.
- **Debug tool:** `sensors -u`, `cat /sys/class/hwmon/hwmon*/name`, and `journalctl --user -b -t
  mate-panel`.

### A load turns the panel orange and spins the fan up

- **Milestone:** M9
- **Steps:**
  1. With the applet in the panel, run `stress-ng --cpu 0 --timeout 120s`. Watch the panel and
     `watch -n 2 sensors`.
  2. Let `stress-ng` stop, and watch for two minutes more.
- **Expected:** in step 1 the CPU's temperature turns orange when it gets to 75 °C, and red at
  90 °C, or at 5 °C below its critical value when `sensors` gives a lower one. The fan's RPM goes
  up, as in `sensors`. In step 2 the temperature goes back to the theme's text colour below
  75 °C, and the fan slows down again, or shows *Fans Off* when it stops.
- **Debug tool:** `sensors -u`, for each reading's `_crit`, and `cat
  /sys/class/hwmon/hwmon*/fan*_input`.

### A 24 px panel shows one line

- **Milestone:** M9
- **Steps:**
  1. With the applet in the top panel, open the panel's *Properties* and set its size to 24
     pixels.
  2. Set it to 32 pixels.
  3. Set the panel's orientation to *Left*.
- **Expected:** in step 1 the applet is one line, as `77°C / 85°C  1897 RPM`, and no part of it is
  cut off. In step 2 it's two lines: the temperatures over the fan's speed (one line on a
  computer with no fans). In step 3 it's the thermometer with the CPU's temperature as a bare
  number, as `77°`, under it.
- **Debug tool:** `dconf dump /org/mate/panel/toplevels/top/`, and `journalctl --user -b -t
  mate-panel`.

### The popup's tiles and Show all sensors match `sensors`

- **Milestone:** M9
- **Steps:**
  1. With the applet in the panel, left-click it.
  2. Click *Show all sensors*. Run `watch -n 1 sensors` in a terminal beside the popup.
  3. Run `stress-ng --cpu 0 --timeout 60s` with the popup open, and put the pointer on a row.
- **Expected:** in step 1 there is a tile for each of CPU, GPU, Storage and Fans that the
  computer has, two to a row. Each temperature tile has the hottest reading of its group, and
  that reading's name under it when the group has more than one; the Fans tile has the fastest
  fan, or *Stopped*. In step 2 every reading of `sensors`, apart from those that read 0 or
  can't be read and the battery's, is in its group, with a bar from 20 °C to 100 °C. The fans
  are last, and a fan with no maximum gets its bar about 10 s after the popup opened. In step 3
  the numbers and bars change every second, the CPU's turn orange and then red with the panel's,
  and the row under the pointer and *Show all sensors* stay as they are.
- **Debug tool:** `sensors -u`, and CMN-97's check script for the same popup on fake trees.

## Network

These need a real NetworkManager and a real VPN server, which caminito-network's tests only fake.
They are the developer's to run.

### A fresh user's top panel has the applet, and no nm-applet in a Session

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` installed and nm-applet's `network-manager-applet` too, make a user
     with no panel settings: `sudo useradd -m nwfresh`, and log in as them to *MATE on
     Caminito*.
  2. Look at the top panel, and run `pgrep -a nm-applet` and `pgrep -a caminito-network`.
  3. Right-click the Network icon and choose *Configure network connections…*.
  4. Log out. Log in as the same user to *MATE*, the X11 session, and run the two commands again.
- **Expected:** in step 2 the Network icon is in the top panel, between the notification area
  and the Bluetooth icon, and the notification area has no network icon of its own. `pgrep`
  finds no nm-applet, and finds `caminito-network`. In step 3 nm-connection-editor opens. In
  step 4 the same panel, now with nm-applet's icon in the notification area too, and
  `caminito-network` not running: nm-applet is the agent there.
- **Debug tool:** `dconf dump /org/mate/panel/objects/`, and `journalctl -b -u NetworkManager |
  grep -i agent`, which names the secret agent that registered: `org.mate.Caminito.Network` in
  a Session.

### Joining a WPA2 network from the popup

- **Milestone:** M9
- **Steps:**
  1. In a Session, near a WPA2 network the machine has never joined, open the Network popup and
     its *Networks* tab. Type part of the network's name in the search field.
  2. Click its row. Type the password in the bar under the list and press Enter.
  3. Run `nmcli -f connection.permissions,802-11-wireless-security.psk-flags connection show
     <name>`.
- **Expected:** in step 1 the list narrows to the networks whose names match, the network's row
  with a lock and its signal. In step 2 the bar shows a spinner with *Connecting to
  <name>…*, then the card shows the network, *Connected*, with its IPv4 address, and the icon
  shows the signal. In step 3 the permissions are empty (for all users) and the flags are `0`
  (NetworkManager keeps the secret).
- **Debug tool:** `nmcli device wifi list`, and `journalctl -b -u NetworkManager`.

### A wrong password is asked for again

- **Milestone:** M9
- **Steps:**
  1. In a Session, join a new WPA2 network from the popup, as above, with a wrong password.
  2. In the dialog that shows, type the right one and press *Connect*.
  3. Change the network's saved password to a wrong one with `nmcli connection modify <name>
     wifi-sec.psk wrongpassword`, then run `nmcli connection up <name>`, and press *Cancel* in
     the dialog.
- **Expected:** in step 1 a *Network Authentication* window from caminito-network says *The
  password for the Wi-Fi network <name> didn't work. Type it again.* In step 2 the network
  connects. In step 3 the same window shows; after *Cancel* the network doesn't come up, and a
  *Connection failed* notification gives the reason.
- **Debug tool:** `journalctl --user -b -t caminito-network`, and `journalctl -b -u
  NetworkManager` for `no secrets`.

### A VPN with a one-time code connects

- **Milestone:** M9
- **Steps:**
  1. With `caminito-git` installed and `networkmanager-openconnect` or `networkmanager-openvpn`,
     set up in nm-connection-editor a VPN whose server asks for a one-time code: OpenConnect
     with a token, or OpenVPN with a static or dynamic challenge. For OpenVPN, leave the
     password *Stored for all users* or *Ask every time*.
  2. Log into *MATE on Caminito*. Run `nmcli connection up <the VPN> --ask` in a terminal and
     wait, without typing in the terminal.
  3. Type the code, and the password where it's asked, and press *Connect*.
  4. Bring the VPN down and up again, and press *Cancel* this time.
- **Expected:** in step 2 a window asks for the VPN's secrets: for OpenVPN caminito-network's
  own, titled *Authentication required*, with the plugin's line naming the VPN and a field for
  each secret it asks, the code among them; for OpenConnect, the plugin's own login window. In
  step 3 the VPN connects, `nmcli connection show --active` lists it, and the Network applet's
  icon has its lock. In step 4 the window closes and the VPN doesn't come up, `nmcli` saying
  the secrets weren't given.
- **Debug tool:** `journalctl -b -u NetworkManager` for the agent's answer, and `journalctl
  --user -b -t caminito-network`.

### A secret nm-applet saved "for this user only" still works in a Session

- **Milestone:** M9
- **Steps:**
  1. In X11 MATE, with nm-applet running, edit a WPA2 Wi-Fi connection in
     `nm-connection-editor`: on *Wi-Fi Security*, set the password's icon to *Store the
     password only for this user*, and save. Connect to it once.
  2. Run `secret-tool search --all connection-uuid "$(nmcli -g connection.uuid connection show
     <name>)"`.
  3. Log out, and log into *MATE on Caminito*, with `caminito-network` installed. Disconnect
     from the network and connect again with `nmcli connection up <name>`.
  4. Lock the keyring (`seahorse`, right-click *Login*, *Lock*), and connect again. Unlock it
     in the prompt that shows.
  5. Lock it again and connect again; dismiss the keyring's prompt.
- **Expected:** step 2 lists one item labelled *Network secret for <name>/802-11-wireless-security/psk*.
  In step 3 the connection comes up with no prompt. In step 4 only the keyring's own prompt
  shows, and the connection comes up after it. In step 5 caminito-network's password dialog
  shows instead; with the password typed, the connection comes up.
- **Debug tool:** `journalctl --user -t caminito-network` for `can't read the keyring's
  secrets` and `can't save the secret in the keyring`; `busctl --user monitor
  org.freedesktop.secrets`.

## Marco parity

### Each placement mode places a native and an X11 window on two outputs

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito* with two monitors. Run `gsettings set org.mate.Marco.general
     center-new-windows false`.
  2. For each of `automatic`, `pointer` and `manual`, run `gsettings set
     org.mate.Marco.general placement-mode <mode>`. Then, with the pointer on each monitor in
     turn, once in the middle and once near a corner, open `pluma` (native) and `xterm`
     (X11) from a terminal on the other monitor.
  3. In `manual`, put one window down with a click, move one with the arrow keys and put it
     down with Return, drag one onto the other monitor, and press Escape on one.
  4. Run `gsettings set org.mate.Marco.general center-new-windows true` (Marco's default)
     and repeat step 2 with `automatic` only, focusing a window on each monitor in turn.
- **Expected:** in step 2 `automatic` puts each window beside the others on the monitor of
  the focused window, `pointer` centres each on the pointer, and near a corner keeps it
  within that monitor and clear of the panel. In `manual` each window shows on the pointer
  and follows it, with the keyboard and pointer taken from the apps, until step 3 puts it
  down; the arrows move it in small steps, it can go onto the other monitor, and Escape
  puts it back where it showed. In step 4 each window opens in the middle of the focused
  window's monitor, over the others. Native and X11 windows behave alike (154).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, for each window's geometry, and
  `gsettings monitor org.mate.Marco.general`.

### The window and workspace actions work from System Shortcuts on a native and an X11 window

- **Milestone:** M8, run in M8's closing run (173).
- **Steps:**
  1. Log into *MATE on Caminito*. Open `pluma` (native) with a frame, `gnome-calculator` or
     another header-bar app (native, its own titlebar), and `xterm` (X11), overlapping.
  2. Open *System Shortcuts* from the control center. Under *Window Management*, give a
     shortcut to each of *Maximize window*, *Toggle fullscreen mode*, *Toggle shaded state*,
     *Raise window if it's covered by another window, otherwise lower it*, *Raise window
     above other windows*, *Lower window below other windows*, *Maximize window vertically*,
     *Maximize window horizontally*, *Switch to workspace 2* and *5*, *Move window to
     workspace 3*, and *Run a terminal*. Under *Desktop*, or with `gsettings set
     org.mate.Marco.global-keybindings run-command-1 '<Super>1'` and `gsettings set
     org.mate.Marco.keybinding-commands command-1 'caja'`, bind a command.
  3. On each of the three windows in turn, press each shortcut, and Alt+F9, Alt+F5 after
     maximising, Ctrl+Shift+Alt+Right and Ctrl+Shift+Alt+Left. With four workspaces, press
     *Switch to workspace 5*. Clear `command-1` to the empty string and press its shortcut.
- **Expected:** each action does to the focused window what its name says, as under Marco,
  and takes effect the moment its shortcut is set. The shade shades pluma and xterm to their
  titlebars, and does nothing to the header-bar app. Raise-or-lower raises a covered window
  and lowers one on top. *Move window to workspace 3* sends the window there and the focus to
  the window under it; Ctrl+Shift+Alt+Right takes the window, the focus and the view to the
  next workspace together. *Switch to workspace 5* does nothing. The terminal shortcut opens
  mate-terminal, the command shortcut opens caja, and the empty command opens nothing and
  leaves a line in the journal (149).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, whose `bindings` lists the
  combinations in force, and whose windows show their state, workspace and stacking;
  `gsettings monitor org.mate.Marco.window-keybindings`; and `journalctl --user -b | grep
  caminito` for the empty command.

### `show-desktop-skip-list` keeps a native and an X11 window on screen

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito*. Open `mate-terminal` (native), `xterm` (X11) and `pluma`
     on the first workspace.
  2. Run `gsettings set org.mate.Marco.general show-desktop-skip-list 'mate-terminal, XTerm'`
     in the terminal, and focus `xterm`.
  3. Click the panel's Show Desktop button, then click it again. Press Ctrl+Alt+D, then
     press it again.
  4. Show the desktop, run `gsettings set org.mate.Marco.general show-desktop-skip-list ''`
     from the terminal, then stop showing it and show it again.
- **Expected:** in step 3 the terminal and `xterm` stay where they are while `pluma` is
  hidden, `xterm` keeps the keyboard, and the second press brings `pluma` back under them,
  with no change to the two listed windows. The button and the key behave alike. In step 4
  the change does nothing until the desktop is shown again, which then hides all three (164).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, for `showing_desktop` and each
  window's app id, `xprop WM_CLASS` on `xterm`, and `gsettings monitor org.mate.Marco.general`.

### A window kept above and a sticky window, from the Frame, a binding and the window list

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito* with four workspaces and the panel's Workspace Switcher.
     Run `gsettings set org.mate.Marco.general button-layout 'menu,above,stick:minimize,maximize,close'`,
     `gsettings set org.mate.Marco.window-keybindings toggle-above '<Super>a'` and
     `gsettings set org.mate.Marco.window-keybindings toggle-on-all-workspaces '<Super>s'`.
  2. Open `pluma` and `caja`. Click `pluma`'s above button, then click `caja`'s body.
     Click the button again, which is now unabove.
  3. Click `caja`'s stick button, then switch through the four workspaces with
     Ctrl+Alt+Right, watching the pager. Back on the first, click its unstick button.
  4. Repeat steps 2 and 3 with Super+A and Super+S on the focused window.
  5. Right-click `pluma`'s button in the window list and pick *Always on Visible Workspace*,
     if the menu has it, and switch workspaces.
  6. Open `xterm` and run `wmctrl -r :ACTIVE: -b add,above` in it, then click `pluma`; run
     `wmctrl -r :ACTIVE: -b add,sticky`, switch workspaces, and run `xprop _NET_WM_STATE
     _NET_WM_DESKTOP` in it.
- **Expected:** a window kept above stays over `caja` when `caja` is clicked, and goes back
  among the others when let go; the Frame shows unabove while it's kept above. A sticky
  window shows on every workspace at the same place, the pager draws it on each, and its
  button in the window list stays on every workspace; unsticking it leaves it on the
  workspace it was unstuck on. The keys and the window list do the same as the Frame. For
  `xterm`, `wmctrl` takes effect, and `xprop` shows `_NET_WM_STATE_ABOVE`,
  `_NET_WM_STATE_STICKY` and `_NET_WM_DESKTOP(CARDINAL) = 4294967295` (155).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, for each workspace's stacking,
  and `xprop` on the X11 window.

### The parity table's rows that no test covers

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito*, and log into *MATE*, the X11 session, on a second machine
     or after it, to compare with Marco.
  2. Go through `docs/parity.md`. For each *honoured* row whose *Covered by* cell is empty,
     or names only a unit test where the row's behaviour shows on screen, change the key
     with `gsettings set` (or press the action's binding) under Caminito and under Marco.
  3. For each *never* and *unsupported* row with a key, set the key away from its default
     under Caminito.
  4. Run as the last step of M8's closing run (173), once no row is *to do*.
- **Expected:** in step 2 Caminito does what the row says, and what Marco does unless the
  row says otherwise. In step 3 an *unsupported* key Caminito doesn't match is logged once,
  `… but Caminito doesn't support it yet; ignoring it`, and a *never* key is ignored without
  a word (09). No row is *to do* (173).
- **Debug tool:** `gsettings monitor org.mate.Marco.general`, Caminito's log, and
  `org.mate.Caminito.Debug`'s state dump.

### The focus modes and auto-raise

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito*. Open a terminal (a native window) and an X11 app, such as
     `xterm`, side by side, with some desktop showing between them and the panel at the top.
  2. Run `gsettings set org.mate.Marco.general focus-mode sloppy`. Move the pointer into the
     terminal, then the X11 app, then onto the desktop, then onto the panel, typing a letter
     at each stop.
  3. Run `gsettings set org.mate.Marco.general focus-mode mouse` and do the same.
  4. Hold Alt and press Tab with the pointer over one window, move the pointer over the
     other, and release Alt on the first.
  5. Run `gsettings set org.mate.Marco.general auto-raise true`, overlap the two windows,
     and move the pointer from the one on top into the part of the other that shows. Then
     do it again and leave it before half a second.
  6. Run `gsettings set org.mate.Marco.general focus-mode click`, then `gsettings set
     org.mate.Marco.general raise-on-click false`, and click the lower window's body, then its
     titlebar. Reset the four keys with `gsettings reset`.
- **Expected:** in step 2 each window takes the focus, and the letter, as the pointer enters
  it, without being raised, and on the desktop and the panel the last window keeps the
  focus. In step 3 the same, but on the desktop no window has the focus and the letter goes
  nowhere, and on the panel the last window keeps it. Each key change applies at once. In
  step 4 the focus stays put while the switcher is open, and goes to the window released on.
  In step 5 the lower window is focused at once and raised after half a second, Marco's
  default `auto-raise-delay`, and not raised the second time. In step 6 the click on the body
  focuses the window and leaves it below; the click on the titlebar raises it (153).
- **Debug tool:** `gsettings monitor org.mate.Marco.general`, and `org.mate.Caminito.Debug`'s
  state dump, whose `focus` and workspaces' `stacking` members show which window has the focus
  and which is on top.

### Ctrl+Alt+Tab and Ctrl+Alt+Esc reach the panel and the desktop, and Alt+Esc and Alt+F6 cycle

- **Milestone:** M8, run in M8's closing run (173).
- **Steps:**
  1. Log into *MATE on Caminito*, with caja drawing the desktop. Open pluma (a native window)
     and `xterm` (X11), and a second pluma window with `pluma --new-window`.
  2. Hold Alt and press Escape. Keep Alt held, look, press Escape again, and release Alt.
     Then tap Alt+Escape quickly, and Alt+Shift+Escape.
  3. Minimise `xterm`. Hold Alt and press Escape until `xterm` is the one shown, then press
     Escape once more, and release Alt.
  4. Focus a pluma window and press Alt+F6, twice.
  5. Hold Ctrl+Alt and press Tab, then release. Type a letter that an icon's name begins with.
     Click a window.
  6. Press Ctrl+Alt+Escape. Press an arrow key, and press Menu or Shift+F10. Close the menu
     and press Alt+Escape.
  7. Compare steps 2 to 6 with Marco under *MATE*, the X11 session.
- **Expected:** in step 2 no popup shows: each press of Escape raises the next window, most
  recently used first, with the focus where it was, and releasing Alt gives the raised window
  the focus. A quick Alt+Escape goes to the window used before, and Alt+Shift+Escape to the
  least recently used. In step 3 the minimised `xterm` shows while it's the selected one and
  is minimised again when the selection leaves it. In step 4 Alt+F6 goes between the two
  pluma windows only. In step 5 caja's desktop is the one thing to list, so it gets the
  keyboard at once and no popup shows. mate-panel's panels aren't listed: mate-panel asks for
  no keyboard on its Layer surfaces, a known gap that needs a carried patch (163). The letter selects an icon, while
  the window that had the focus keeps its Frame drawn focused, and the click gives the
  keyboard back to the window. In step 6 the desktop has the keyboard again: the arrow moves
  between the icons and the menu key opens the selected one's menu; Alt+Escape gives the
  keyboard back to a window. Under Marco the panels are reached too.
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump: the `switcher` member has `cycle`,
  the `windows` and `layers` listed and the selection while the modifier is held, and each
  output's `layer_surfaces` have their `keyboard_interactivity`.

### Alt+F7 and Alt+F8 move and resize a window by the keys

- **Milestone:** M8, run in M8's closing run (173).
- **Steps:**
  1. Log into *MATE on Caminito*. Open pluma (a native window) and `xterm` (X11), apart.
  2. Focus pluma, press Alt+F7, press Right three times, Down once, and Ctrl+Right once, move
     the mouse a little, and press Return. Note where the pointer is after each key.
  3. Press Alt+F7 again, move pluma with the arrows and the mouse, and press Escape.
  4. Press Alt+F8, press Right, then Right twice more, then Down, then Down twice more, and
     press Return. Then press Alt+F8, Left twice, and Escape.
  5. Repeat steps 2 to 4 with `xterm`, then from `xterm`'s own keyboard move and resize if
     it has them, and compare with Marco under *MATE*, the X11 session.
- **Expected:** in step 2 the pointer jumps to the middle of pluma when Alt+F7 is pressed and
  stays in its middle as each arrow moves it 10 pixels (Ctrl+Right 1); the mouse moves it
  too; Return leaves it there, and nothing typed reaches pluma during the move. In step 3
  Escape puts pluma back where it was before Alt+F7. In step 4 the first Right moves the
  pointer to the middle of pluma's right edge without resizing it, the next two widen it by
  10 pixels each with the pointer on the edge, and Down moves the pointer to the bottom edge
  to grow it from there; Escape after the second Alt+F8 puts the size back. `xterm` does the
  same. A step that would take a side past a monitor's edge, the panel's or another
  window's stops there, and the next key goes on, as in Marco (156).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, for the windows' geometry and the
  bindings in force.

### Focus-stealing prevention for native windows

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito*. Open `mate-terminal` and Firefox (native, with
     `MOZ_ENABLE_WAYLAND=1`), and focus the terminal.
  2. With `gsettings set org.mate.Marco.general focus-new-windows smart` (Marco's default),
     run `xdg-open https://mate-desktop.org` in the terminal, which passes Firefox a token. Then
     open `pluma` from the panel's menu. Then run `sleep 5; zenity --info` in the terminal and
     focus Firefox before it opens, and run `(sleep 5; pluma --new-window) &` and focus Firefox
     again.
  3. Hold Alt and press Tab while a refused window is open. Release Alt on it.
  4. Run `gsettings set org.mate.Marco.general focus-new-windows strict` and repeat step 2.
  5. Run as part of M8's closing run (173), and reset the key with `gsettings reset`.
- **Expected:** in step 2 the link opens in the running Firefox, which takes the focus and
  comes to the top; `pluma` from the menu takes the focus, as a first window does with no
  token; `zenity`, a first window, takes it too, and the second `pluma` window, which opens
  by itself while Firefox has the focus, doesn't: it opens under Firefox and is urgent. In step
  3 the urgent window has a dot in its cell's top right corner, and is no longer urgent once
  it has the focus. In step 4 the link still raises Firefox, as the terminal asked with a key
  press; `pluma` from the menu and `zenity` open under the focused window, urgent, since MATE's
  launchers pass no token yet (165).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, whose windows' `urgent` and the
  `focus` member show which was refused, `WAYLAND_DEBUG=1` on the app for its
  `xdg_activation_v1` requests, and `gsettings monitor org.mate.Marco.general`.

### Focus-stealing prevention for XWayland windows, and urgency in the window list

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito*, with `xterm` and `wmctrl` installed and
     `focus-new-windows` at `smart`, Marco's default. Open `xterm` from the panel's menu, and
     `mate-terminal`.
  2. In `xterm`, run `(sleep 5; GDK_BACKEND=x11 pluma --new-window) &`, wait for Pluma's
     window, then run the same command again and focus `mate-terminal` before the second
     window opens.
  3. Look at the second Pluma window's button in the panel's window list, and at its cell in
     the Window switcher (Alt+Tab, then Escape). In `mate-terminal`, find its id with
     `wmctrl -l` and run `xprop -id <id> _NET_WM_STATE`.
  4. Click the button in the window list.
  5. Focus `mate-terminal`, then in it run `wmctrl -a xterm`.
  6. Run as part of M8's closing run (173).
- **Expected:** in step 2 the first Pluma window takes the focus, as its client's first
  window; the second opens under the focused window and doesn't take the focus. In step 3 its
  button in the window list shows that it wants attention, its cell in the Window switcher
  has a dot in its top right corner, and its `_NET_WM_STATE` has
  `_NET_WM_STATE_DEMANDS_ATTENTION`. In step 4 the window comes to the top with the focus,
  and its button and its cell no longer show it wants attention. In step 5 `xterm` takes the
  focus: `wmctrl` gives no source indication, which Caminito honours as Marco does (166).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, whose windows' `urgent` and the
  `focus` member show which was refused, `xprop -id <window> _NET_WM_STATE _NET_WM_PID`, and
  `WAYLAND_DEBUG=1 mate-panel --replace` for the `xfce_foreign_toplevel_handle_v1` `state`
  events.

### A failed command's dialog, and `rename-workspace`

- **Milestone:** M8, run in M8's closing run (173).
- **Steps:**
  1. Log into *MATE on Caminito*, with `zenity` installed. Open *System Shortcuts* from the
     control center, or use `gsettings`, to bind `run-command-1` to Super+1, leaving
     `org.mate.Marco.keybinding-commands command-1` at its default, and press it.
  2. Run `gsettings set org.mate.Marco.keybinding-commands command-1 'no-such-program'` and
     press Super+1 again.
  3. Bind `rename-workspace` (`gsettings set org.mate.Marco.global-keybindings
     rename-workspace '<Super>r'`). Switch to the second workspace, press Super+R, type a new
     name and press OK. Press Super+R again and press Cancel.
- **Expected:** in step 1 an error dialog titled *Marco* says the text was empty, and takes
  the focus as a new window does; in step 2 it says there was an error running
  `no-such-program`, with GLib's message. Caminito goes on meanwhile. In step 3 the entry
  dialog asks for the *New Workspace Name:* with the workspace's name filled in, and the
  pager shows the new name once OK is pressed; Cancel leaves it (167).
- **Debug tool:** `gsettings monitor org.mate.Marco.workspace-names`, and Caminito's log, which
  has a line for each command that can't run.

### Shadows around a native and an X11 window

- **Milestone:** M8, run in M8's closing run (173).
- **Steps:**
  1. Log into *MATE on Caminito*, on a monitor at a fractional scale such as 1.5. Open
     `mate-terminal`, a native Wayland window with a Frame, `xterm`, an XWayland one, and a
     GTK4 application that draws its own titlebar, such as `gnome-calculator`. Put the two
     terminals so that they overlap, over a light desktop background.
  2. Focus each terminal in turn.
  3. Move the pointer onto the focused terminal's shadow, more than 10 logical pixels from
     its Frame and over the other terminal, and click.
  4. Resize a terminal, shade it by a double click on its titlebar with
     `action-double-click-titlebar` at `toggle_shade`, and unshade it. Maximise it, and tile it
     to a side.
  5. Take a screenshot of a terminal's window with `mate-screenshot -w`.
  6. Run `gsettings set org.mate.Marco.general compositing-manager false`, look, and set it
     back to `true`.
- **Expected:** in step 1 each terminal has a soft black shadow around its Frame, which
  follows the Frame's rounded corners with no gap and no hard edge, and Caminito draws none
  around the GTK4 window, which has its own. In step 2 the focused terminal's shadow is the
  larger, and lies down and right of its Frame; the other's is smaller and lies below it. In
  step 3 the pointer shows no resize cursor, and the click focuses and raises the terminal
  under the shadow. In step 4 the shadow follows the Frame as it's resized, lies around the
  titlebar alone while the window is shaded, and isn't there while the window is maximised
  or tiled to a side, as in Marco. In step 5 the screenshot is the window with its Frame and
  no shadow. In step 6 no window has a shadow while the key is off, at once, and they come
  back with it (171).
- **Debug tool:** `gsettings monitor org.mate.Marco.general`, and frame debug, which outlines
  each Frame's strips and its invisible resize border.

### The minimise animation

- **Milestone:** M8, run in M8's closing run (173).
- **Steps:**
  1. Log into *MATE on Caminito*, with the panel's window list showing. Open pluma (a native
     window) and `xterm` (X11), apart, away from the window list.
  2. Minimise pluma with its Frame's minimise button, and bring it back by its button in the
     window list. Then click its button in the window list while it has the focus, which
     minimises it, and bring it back again.
  3. Repeat step 2 with `xterm`.
  4. Minimise pluma and click its button in the window list at once, before it's gone.
  5. Run `gsettings set org.mate.Marco.general reduced-resources true` and repeat steps 2
     and 3. Then run `gsettings reset org.mate.Marco.general reduced-resources`.
- **Expected:** in steps 2 and 3 the window shrinks into its button in the window list in a
  quarter of a second, drawn with its content and its Frame, over the other windows and the
  panel, and the focus goes to the other window as it starts. In step 4 the window is back
  in its place at once, with nothing left on its way. In step 5 each window is gone at
  once, with no animation (172).
- **Debug tool:** `WAYLAND_DEBUG=1` on the panel, or `wlrctl`, for the
  `zwlr_foreign_toplevel_handle_v1.set_rectangle` it sends, and
  `gsettings monitor org.mate.Marco.general`.

### The opening and closing animations beside the slide, the Overview and the lock

- **Milestone:** `open-close-animation`, the developer's to run (254).
- **Steps:**
  1. Log into *MATE on Caminito* on the tty backend, with two monitors side by side and four
     workspaces in one row. Run `gsettings get org.mate.Caminito open-close-animation`,
     `gsettings get org.mate.Caminito workspace-slide` and
     `gsettings get org.mate.Caminito workspaces-per-output`, which print `true`. Open a
     terminal on the first workspace of the first monitor.
  2. Open pluma (a native window) and `xterm` (X11), and close each with its Frame's close
     button.
  3. In the terminal, run `sleep 0.1; pluma &` and press Ctrl+Alt+Right at once, so pluma
     opens on the second workspace as it slides in.
  4. In pluma, press Ctrl+Q and Ctrl+Alt+Left at once. Then on the second workspace run
     `pluma & sleep 2; pkill pluma` and press Ctrl+Alt+Left about two seconds later, so it
     closes as the slide runs.
  5. Open the Overview with Super+W, and from a terminal in it run `pluma &`, and close
     another window from its Frame. Close the Overview, and open and close a window as it
     closes.
  6. Run `pluma & sleep 0.05; mate-screensaver-command --lock`, and unlock.
  7. Close a window on the second monitor and unplug that monitor at once.
  8. Run `gsettings set org.mate.Caminito workspaces-per-output false` and repeat steps 3
     and 4. Set it back to `true`.
- **Expected:** in step 2 each window grows into its place and fades in as it opens, and
  shrinks and fades out as it closes, in a fifth of a second. In step 3 pluma comes in with
  its workspace as it grows and fades in, with no jump at the slide's end. In step 4 each
  closing window's picture leaves with its workspace as it shrinks and fades, and is drawn
  nowhere else. In step 5 nothing grows or shrinks in the Overview or during its
  transition: a window joins or leaves its layout as it does with the animation off. In
  step 6 the locker shows nothing that grows, and after the unlock pluma is at its place
  at rest. In step 7 nothing of the picture is left on the first monitor. In step 8 both
  monitors slide, each window with its own monitor's workspace (ADR 0032). In every step
  the animations are smooth at each monitor's refresh rate.
- **Debug tool:** `DumpState`'s `opening_windows`, `closing_windows` and `workspace_slides`.

### The Maximise animation beside the slide, the Overview and the lock

- **Milestone:** `maximize-animation`, the developer's to run (CMN-177).
- **Steps:**
  1. Log into *MATE on Caminito* on the tty backend, with two monitors side by side and four
     workspaces in one row. Run `gsettings get org.mate.Caminito maximize-animation`,
     `gsettings get org.mate.Caminito workspace-slide` and
     `gsettings get org.mate.Caminito workspaces-per-output`, which print `true`. Open pluma
     (a native window) on the first workspace of the first monitor, and `xterm` (X11) on its
     second workspace, each smaller than the monitor and away from its corner.
  2. Focus pluma, press Alt+F10 and Ctrl+Alt+Right at once, so pluma grows as its workspace
     slides out.
  3. Focus `xterm` and press Ctrl+Alt+Left and Alt+F10 at once. Then press Alt+F10 again.
  4. Maximise pluma with Alt+F10 and press Super+W at once. In the Overview, double-click
     `xterm`'s titlebar if it shows one, or else maximise it with `wmctrl -r xterm -b
     add,maximized_vert,maximized_horz` from a terminal. Close the Overview.
  5. Press Alt+F10 on pluma and run `mate-screensaver-command --lock` at once, from a
     terminal with `sleep 0.05;` before it. Unlock.
  6. Put pluma on the second monitor, press Alt+F10 on it and unplug that monitor at once.
  7. Run `gsettings set org.mate.Caminito workspaces-per-output false` and repeat steps 2
     to 6. Set it back to `true`.
- **Expected:** in step 2 pluma grows into the work area as it leaves with its workspace,
  drawn nowhere else, with no jump at the slide's end. In step 3 `xterm` grows into the work
  area as it comes in with its workspace, and the second Alt+F10 shrinks it back. In step 4
  pluma is maximised at once as the Overview opens, with no box left on its way, and
  nothing grows during the transition or in the Overview; `xterm` comes out of the Overview
  maximised, at rest. In step 5 the locker shows nothing that grows, and after the unlock
  pluma is at its place at rest. In step 6 pluma is on the first monitor at rest, and
  nothing of it is left on its way. In step 7 both monitors slide, each window with its own
  monitor's workspace (ADR 0032), and steps 4 to 6 are as before. In every step the
  animation is smooth at each monitor's refresh rate.
- **Debug tool:** `DumpState`'s `maximizing_windows` and `workspace_slides`.

### The Maximise animation for tiles, and a drag that pulls a window off

- **Milestone:** `maximize-animation`, the developer's to run (CMN-175).
- **Steps:**
  1. Log into *MATE on Caminito* on the tty backend. Run
     `gsettings get org.mate.Caminito maximize-animation`, which prints `true`. In
     mate-control-center's *System Shortcuts*, under *Window Management*, set *Tile window to
     east side* to Super+W. Open pluma (a native window) and `xterm` (X11), each smaller than
     the monitor and away from its corner.
  2. Drag pluma by its titlebar to the monitor's left edge and let go. Drag it to the top edge
     and let go.
  3. Drag pluma by its titlebar off its maximised place, down into the middle of the monitor,
     and let go there.
  4. Press Super+W four times, waiting for each to settle, then three more times.
  5. Run `gsettings set org.mate.Marco.general allow-tile-cycling false`, press Super+W twice,
     and reset it with `gsettings reset`.
  6. Tile pluma with Super+W and press Alt+F10. Then drag it by its titlebar off its tile.
  7. Repeat steps 2 to 6 with `xterm`.
- **Expected:** in step 2 pluma goes from where it was let go into the left half of the work
  area, and then into the whole work area, each time over a quarter of a second, with a
  picture of it as it was fading out and no old content at its new place. In step 3 pluma is
  under the pointer from the first frame the drag pulls it off, at its size from before, with
  nothing growing or shrinking, and it stays where it's let go. In step 4 each press moves it
  smoothly into its tile, a half, a third, a quarter and the whole width, then three quarters,
  two thirds, and back to its place from before. In step 5 it goes into the right half and back.
  In step 6 it grows from its tile into the work area, and the drag pulls it off at once, as in
  step 3. In step 7 `xterm` does the same.
- **Debug tool:** `DumpState`'s `maximizing_windows`, and each window's `tile`.

### The window menu

- **Milestone:** M8, run in M8's closing run (173).
- **Steps:**
  1. Log into *MATE on Caminito*. Open `pluma` (a native window with a Frame), a GTK4 app
     that draws its own titlebar, such as `gnome-calculator`, and an X11 GTK3 app with
     `GDK_BACKEND=x11`, such as `GDK_BACKEND=x11 gedit` or any GTK3 app with a header bar.
  2. Focus `pluma` and press Alt+Space. Then right click its titlebar, and then click its
     Frame's menu button. Right click the GTK4 app's titlebar, and the X11 app's.
  3. In `pluma`'s menu, choose each item once, opening the menu again each time: with the
     arrows and Return, with its underlined letter, and with a click. Go into *Move to
     Another Workspace* with Right and with the pointer.
  4. Open the menu and press Escape. Open it and click outside it.
  5. Maximise `pluma` and open its menu. Open the menu of a fixed-size dialog, such as
     `pluma`'s *About*.
  6. Run the Session in another language, such as `LANG=es_AR.UTF-8`, and open the menu.
- **Expected:** in step 2 each of the five opens the same menu: under the window's top left
  corner for Alt+Space, at the pointer for the right clicks, and under the button for the
  menu button. The GTK4 and the X11 app show Caminito's menu, not their own. In step 3 each
  item does what its binding does, the moves to a workspace leave the active workspace as
  it is, and the keybindings beside the items are those in force. In step 4 the menu closes,
  and the click reaches no window. In step 5 *Move* and *Resize* are greyed for the
  maximised window, and *Maximize* and *Resize* for the dialog. In step 6 the labels are
  Marco's in that language.
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, whose `window_menu` member lists
  the items, which are greyed, and the selection; `WAYLAND_DEBUG=1` on the GTK4 app for
  `xdg_toplevel.show_window_menu`, and `xprop -root _NET_SUPPORTED` for
  `_GTK_SHOW_WINDOW_MENU`.

### Modifier clicks and titlebar scrolling from the control center

- **Milestone:** M8, run in M8's closing run (173).
- **Steps:**
  1. Log into *MATE on Caminito*. Open `pluma` (a native window with a Frame) and an X11 app,
     such as `xterm`.
  2. Open the control center's *Windows*, and on *Behaviour* set *Movement Key* to *Super*.
     Run `gsettings get org.mate.Marco.general mouse-button-modifier`.
  3. On each of the two windows, with Super held and the pointer inside the window: drag with
     the left button, drag with the right button near a corner, and click the middle button.
     Then do the same with Alt held.
  4. Run `gsettings set org.mate.Marco.general resize-with-right-button false`, and with Super
     held drag with the middle button and click the right one, on each window.
  5. Run `gsettings set org.mate.Marco.general action-scroll-up-titlebar shade` and
     `gsettings set org.mate.Marco.general action-scroll-down-titlebar unshade`. Turn the wheel
     up and then down over each window's titlebar, and then over one of its buttons.
  6. Put the settings back: *Movement Key* to *Alt*, and `gsettings reset` for the three keys.
- **Expected:** in step 2 the key is `'<Super>'`. In step 3, with Super the left button moves the window, the right one resizes
  it from the corner nearest the pointer and the middle one opens the window menu at the
  pointer, and the app shows no sign of the clicks, such as its own context menu; with Alt
  the clicks are the app's. In step 4 the middle button resizes and the right one opens the
  menu. In step 5 the window rolls up and then unrolls, with no need to restart anything, and
  a scroll over a button does nothing.
- **Debug tool:** Caminito's log, which names a value of `mouse-button-modifier` it can't
  use; `org.mate.Caminito.Debug`'s state dump, whose `window_menu` member shows the open
  menu.

### A mouse button as a System shortcut

- **Milestone:** M8
- **Steps:**
  1. Log into *MATE on Caminito* with a mouse that has back and forward buttons. Open
     `pluma` (a native window) and Firefox, open two pages one after the other in a Firefox
     tab, and click the back button over the page.
  2. With `mate-control-center-caminito` 1.29.1-6 or later installed, open *System Shortcuts*,
     click *Toggle maximization state*'s shortcut, and with Super held click the back button
     over the dialog. Close the dialog.
  3. Click pluma to focus it. With Super held, click the back button over pluma, then over
     the panel, then over the desktop.
  4. Focus Firefox, go forward to the second page, and with Super held click the back button
     over the page. Then click it without Super.
  5. In *System Shortcuts*, set *Minimize window* to Shift, Super and the back button as in
     step 2, and right-click pluma's titlebar or press Alt+Space.
  6. In *System Shortcuts*, set *Move between windows, using a popup window* to Super and the
     forward button. With Super held, click the forward button twice, and let go of Super.
  7. Put the keys back: `dconf reset` for `/org/mate/marco/window-keybindings/toggle-maximized`,
     `/org/mate/marco/window-keybindings/minimize` and
     `/org/mate/marco/global-keybindings/switch-windows`.
- **Expected:** in step 1 Firefox goes back. In step 2 the row reads *Super+Back Button*, and
  `dconf read /org/mate/marco/window-keybindings/toggle-maximized` is `'<Super>Button8'`.
  In step 3 each click maximises or restores
  pluma, wherever the pointer is, and pluma stays focused and on top. In step 4 Super and
  the back button maximise Firefox and it doesn't go back a page; without Super it goes
  back. In step 5 the menu shows *Shift+Super+Back Button* beside *Minimize*. In step 6 the
  Window switcher opens, each click steps it, and letting go of Super picks the window. After step 7 Super and the back button go back in Firefox again (184).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, whose `bindings` lists
  `toggle-maximized` with `<Super>Button8`; Caminito's log, which names a value it can't
  bind, such as `Button1` with no modifier; `wev` or `xev` for the button's number.

### System Shortcuts captures a mouse button

- **Milestone:** M8
- **Steps:**
  1. With `mate-control-center-caminito` 1.29.1-6 or later installed, log into *MATE on
     Caminito* with a mouse that has back and forward buttons. Open `pluma`, and *System
     Shortcuts*. Each click below that sets a shortcut is over the dialog.
  2. Click *Toggle maximization state*'s shortcut, and with Super held click the back button.
     Run `dconf read /org/mate/marco/window-keybindings/toggle-maximized`. Focus pluma, and
     with Super held click the back button over pluma, then over the panel.
  3. Click *Add*, name a shortcut *Calculator* with the command `mate-calc`, click its
     shortcut, and click the forward button alone. Click the forward button over pluma.
  4. Click the calculator's shortcut and click the middle button alone. Click *Cancel*.
  5. Click the calculator's shortcut and, with Alt held, click the middle button. Click
     *Cancel*.
  6. Click the calculator's shortcut, then click another row with the left button. Click the
     calculator's shortcut again and press Backspace.
  7. Click the calculator's shortcut and, with Super held, click the back button. Click
     *Cancel*, then do it again and click *Reassign*.
  8. Close the dialog and open it again. Then run `LANGUAGE= LC_ALL= LANG=es_ES.UTF-8
     mate-keybinding-properties` from a terminal, with that locale generated (`locale -a`),
     and set a row to the middle button alone there, then cancel.
  9. Remove the calculator's shortcut, `dconf reset
     /org/mate/marco/window-keybindings/toggle-maximized`, and set it to `<Super>Button8`
     with `dconf write` again. Log out, log into *MATE*, the X11 session, and open *Keyboard
     Shortcuts*. Click *Toggle maximization state*'s shortcut and click the back button with
     Super held, then press Super+Up. Then `dconf reset` the key.
- **Expected:** in step 2 the row reads *Super+Back Button*, the key is `'<Super>Button8'`,
  and each click maximises or restores pluma at once, over pluma and over the panel. In step
  3 the row reads *Forward Button* and the click opens a calculator. In step 4 the dialog asks
  first, saying the middle button would no longer paste, and after *Cancel* the row reads
  *Forward Button* again. In step 5 it warns that over a window Alt and the middle button
  still act on the window, and the row is unchanged after *Cancel*; *OK* would set it. In
  step 6 the left click ends the capture with the row as it was, and Backspace makes it
  *Disabled*. In step 7 the dialog says *Super+Back Button* is already used for *Toggle
  maximization state*; after *Reassign* the calculator has it and *Toggle maximization state*
  is *Disabled*. In step 8 the rows read as they did, and in Spanish the labels are *Botón
  atrás*, *Botón adelante* and *Botón central*, and the question is in Spanish. In step 9
  the row reads *Disabled*, the back button with Super held captures nothing and the row
  stays capturing, Super+Up is captured as before, as `'<Mod4>Up'`, and the key keeps
  `'<Super>Button8'` until then (ADR 0011, ADR 0028).
- **Debug tool:** `dconf watch /org/mate/`; `mate-keybinding-properties` run from a terminal;
  `org.mate.Caminito.Debug`'s state dump, whose `bindings` lists the combinations in force;
  `wev` for the button's number.

### "Not responding" and Force Quit

- **Milestone:** M8, run in M8's closing run (173).
- **Steps:**
  1. Log into *MATE on Caminito*, with `zenity` installed. Open `pluma` (a native window)
     and `xterm` (X11).
  2. Stop `pluma` with `kill -STOP $(pidof pluma)` from a terminal. Focus it, press Alt+F4,
     and wait five seconds.
  3. In the dialog, choose *Wait*. Press Alt+F4 on `pluma` again, and before the dialog
     shows run `kill -CONT $(pidof pluma)`.
  4. Open `pluma` again, stop it, press Alt+F4 and wait for the dialog. Press Alt+F4 on
     `pluma` once more, and wait five seconds. Press Alt+F4 on the dialog, and then Escape
     in it.
  5. Press Alt+F4 on `pluma` once more, wait for the dialog and choose *Force Quit*.
  6. Repeat steps 2 to 5 with `xterm`, stopped with `kill -STOP $(pidof xterm)`.
  7. Open `pluma` again, stop it, press Alt+F4, wait for the dialog, and run
     `kill -CONT $(pidof pluma)`.
- **Expected:** in step 2 Marco's dialog shows after five seconds, titled *Marco*, with the
  window's title, "is not responding", and the buttons *Wait* and *Force Quit*. In step 3
  the dialog goes and `pluma` stays, stopped; then `pluma` closes with no dialog. In step
  4 the second Alt+F4 shows no second dialog, and gives the first one the focus; Alt+F4
  on the dialog leaves it and `pluma` as they are, and Escape closes the dialog and
  `pluma` stays. In step 5 `pluma`'s window goes at once and its
  process is gone (`pidof pluma` prints nothing). In step 6 `xterm` does the same. In
  step 7 the dialog goes by itself as `pluma` closes. Caminito, Xwayland and the other
  windows stay throughout (168).
- **Debug tool:** `journalctl -t caminito` for `forcing … to quit` and `can't force … to
  quit`; `WAYLAND_DEBUG=1` on a native app for `xdg_wm_base.ping` and `pong`, and
  `xprop WM_PROTOCOLS _NET_WM_PID WM_CLIENT_MACHINE` on an X11 window.

### The Dock applet's items in a real Session

- **Milestone:** `dock-applet`, the developer's to run (CMN-219).
- **Steps:**
  1. Log into *MATE on Caminito*, with Firefox, `mate-terminal`, `xterm` (X11) and a Flatpak
     installed. Right-click a panel, choose *Add to Panel…* and add *Dock*.
  2. Open Firefox from the main menu. Right-click its item and choose *Keep in Dock*, then
     close Firefox and click its item. Right-click the item again: the menu has Firefox's
     windows, its actions (*New Window*, *New Private Window*), *Remove from Dock* and
     *Close all windows*.
  3. Open `mate-terminal` twice, as two processes (`mate-terminal --disable-factory &`
     twice). Click its item while another window has the focus, and then again while one
     terminal has it.
  4. Open `xterm`, and the Flatpak from the main menu.
  5. Open pluma, keep it, and minimise it by a click on its item while it has the focus.
  6. Drag pluma's item before Firefox's, drag a launcher from the main menu onto the dock,
     and restart the panel with `mate-panel --replace &`.
  7. Move the panel to the left edge from its Properties.
- **Expected:** in step 2 Firefox's item stays after it closes and the click starts it again.
  In step 3 both terminals are under one item, which has the indicator line, highlighted
  while a terminal has the focus; the first click brings the most recent terminal forward
  and the second does too, as the Application overview isn't there yet. In step 4 `xterm`
  has an item of its own, by its `WM_CLASS`, and the Flatpak's window joins its entry's
  item, with the entry's icon and name as tooltip. In step 5 the window shrinks into pluma's
  item, not into the window list. In step 6 the order, and the launcher kept, are the same
  after the restart. In step 7 the items are a column, with the indicator on the screen's
  edge, and the menus open away from it.
- **Debug tool:** `WAYLAND_DEBUG=1 mate-panel --replace` for
  `zwlr_foreign_toplevel_handle_v1.set_rectangle` and `app_id`, `xprop WM_CLASS` on an X11
  window, and `gsettings get` on the instance's `prefs/` path (`dconf dump
  /org/mate/panel/objects/`) for `kept`.

### The Dock applet's Trash in a real Session

- **Milestone:** `dock-applet`, the developer's to run (CMN-220).
- **Steps:**
  1. Log into *MATE on Caminito* with the Dock on a panel. Look at the end of the row, after
     the separator: the Trash, with the empty icon.
  2. Drag a file from Caja onto the Trash.
  3. Click the Trash.
  4. Right-click the Trash and choose *Empty Trash*; choose *Cancel*. Do it again and
     choose *Empty Trash*.
  5. Move the panel to the left edge from its Properties, and drag a file onto the Trash
     again. With Caja open on the Trash, delete a file from there.
- **Expected:** in step 2 the file leaves Caja's folder and the Trash's icon turns full. In
  step 3 Caja opens the Trash. In step 4 the menu has *Open* and *Empty Trash* (greyed out
  while the Trash is empty), the question is Caja's, and Cancel leaves the Trash as it was;
  confirmed, it empties and the icon turns empty. In step 5 the Trash is at the end of the
  column, and its icon follows what Caja does to it.
- **Debug tool:** `gio list trash:///` and `gio monitor trash:///` for what the Trash holds
  and what it tells; `ls ~/.local/share/Trash/files`.

### The Dock applet's Removable drive items in a real Session

- **Milestone:** `dock`, the developer's to run (CMN-240).
- **Steps:**
  1. Log into *MATE on Caminito* with the Dock on a panel, and plug in a USB disk. Look after
     the separator, past the Kept folders if there are any. Rest the pointer on the new item.
  2. Click the item.
  3. Drag a file from Caja's home folder onto the item.
  4. Open a terminal on the disk (`cd /run/media/$USER/<name>` or `/media/$USER/<name>`) and
     leave it there. Right-click the item and choose *Eject*. Close the terminal, and choose
     *Eject* again.
  5. Plug in the disk again, and unmount it from Caja's sidebar. Mount a network share in Caja
     (*Connect to Server…*), and look at the dock.
- **Expected:** in step 1 the disk has an item with its icon, before the Trash, and its tooltip
  is the disk's name; the internal disks have none. In step 2 Caja opens the disk. In step 3 the
  file is copied to the disk and is still in the home folder. In step 4 the menu has *Open* and
  *Eject* (*Unmount* for a disk that can't be ejected), the first eject shows an error that the
  disk is busy, as the Trash shows its errors, and the second takes the item away. In step 5 the
  item goes with the unmount, and the network share has no item.
- **Debug tool:** `gio mount -li` for each mount's drive, volume and whether it can be ejected or
  unmounted, and `gio mount -o` to watch the monitor's events.

### The Dock applet's launch and attention feedback in a real Session

- **Milestone:** `dock-applet`, the developer's to run (CMN-221).
- **Steps:**
  1. With the Dock applet on a panel and Firefox kept in it but closed, click its item.
  2. Click the item of a kept program that never opens a window, such as a desktop entry
     whose `Exec` is `sleep 30`.
  3. With Firefox running, middle-click its item for a new instance.
  4. Open a terminal and Firefox. In the terminal run `sleep 3; mate-terminal
     --disable-factory`, and click Firefox's window before the three seconds are up.
  5. Leave the terminal's item highlighted a while, then focus its new window from the dock.
     Do it again, and focus the window by its titlebar or with Alt+Tab.
- **Expected:** in step 1 the icon pulses until Firefox's first window shows, and stops
  then. In step 2 the icon pulses for about 10 seconds and stops. In step 3 the running
  item pulses until the second window shows. In step 4 the new terminal's window is refused
  the focus, and the terminal's icon bounces a few times and keeps a highlighted box behind
  it. In step 5 the highlight goes away as the window takes the focus, from the dock or
  otherwise. Under `focus-new-windows` strict a window a launch from the dock opens is still
  refused the focus, as the dock has no keyboard focus for Caminito to tie the token to.
- **Debug tool:** `WAYLAND_DEBUG=1 mate-panel --replace` for `xdg_activation_v1` and
  `xdg_activation_token_v1.set_app_id`, `cat /proc/PID/environ | tr '\0' '\n' | grep
  XDG_ACTIVATION_TOKEN` on the launched program, and Caminito's `DumpState` for each window's
  `urgent`.

### The Dock applet's badges and progress bars in a real Session

- **Milestone:** `dock-applet`, the developer's to run (CMN-227).
- **Steps:** in a terminal of the Session, with `emit` for `.scratch/checks/CMN-227/hold-emit.py`,
  which sends the signal as `gdbus emit` does and then stays on the bus until Ctrl+C. (A
  `gdbus emit --session --object-path /test --signal com.canonical.Unity.LauncherEntry.Update`
  with the same two arguments leaves the bus as soon as it has sent, and the Dock then
  drops its state at once, as it does for any program that exits.)
  1. With the Dock applet on a panel and Caja running, run `emit
     'application://org.mate.Caja.desktop' "{'count': <int64 5>, 'count-visible': <true>}"`.
     Leave it running.
  2. In another terminal, `emit` the same with `count` 150, and then with
     `'count-visible': <false>`.
  3. `emit` `"{'progress': <0.4>, 'progress-visible': <true>}"`, with the panel on the bottom
     edge, and again with it on the top, left and right edges. Then `progress` 0.9, and
     `'progress-visible': <false>`.
  4. `emit` `"{'urgent': <true>}"`, focus Caja's window, and `emit` `"{'urgent': <false>}"`.
  5. With a badge and a bar showing, end the `emit` that sent them with Ctrl+C.
  6. Close Caja and, with its item not kept, `emit` a count for `org.mate.Caja.desktop`, and
     then open Caja. Then `emit` a count for `application://nothing.desktop`.
  7. With a badge showing from an `emit` that is still running, close Caja's last window.
- **Expected:** in step 1 Caja's item has a round badge with a white "5" on its top-right
  corner, in the highlight colour (on a top panel its bottom-right, and on a right panel its
  top-left: the corner away from the edge). In step 2 it reads "99+", and then the badge goes
  away. In step 3 a thin bar, 40 % full, runs across the icon just inside the indicator line on
  each of the four edges, over neither the indicator nor the badge, and goes away with
  `progress-visible` false. In step 4 the item gets the highlighted box and bounces a few times;
  focusing Caja's window leaves the box, and `urgent` false takes it away. In step 5 the badge
  and bar are gone as soon as the program ends. In step 6 the first signal makes no item, and
  when Caja opens its item shows the badge; the last signal makes no item. In step 7 the badge
  goes with the item's last window (CMN-227).
- **Debug tool:** `dbus-monitor "interface='com.canonical.Unity.LauncherEntry'"` for what
  programs send and from which unique name, and `dbus-monitor --session
  "member='NameOwnerChanged'"` for a sender leaving.

### A floating panel stands off its edge

- **Milestone:** `dock-applet`, the developer's to run (CMN-223).
- **Steps:**
  1. With `mate-panel-caminito` at `pkgrel` 14 or later, log into *MATE on Caminito*. Right-click
     the bottom panel, choose *Properties*, and tick *Floating*.
  2. Open a window and maximise it. Then untick *Expand* in the panel's properties, and
     close the dialog.
  3. Do the same to a panel at the left edge. Add one with *New Panel* and set its orientation
     to *Left* if there is none.
  4. Untick *Floating* in each.
  5. Log into *MATE on Wayfire* or an X11 *MATE* session, if there is one, and open the
     panel's properties.
- **Expected:** in step 1 the panel stands 8 pixels off the bottom edge, with rounded corners
  and no line along its top, and a maximised window ends at the panel's top edge, not behind
  it and not short of it. In step 2 the panel shrinks to its applets and stays centred along
  the edge, still 8 pixels off it. In step 3 the left panel stands off the left edge in the
  same way, centred from top to bottom. In step 4 each panel goes back to its edge, with
  square corners and its frame, and the work area grows back. In step 5 *Floating* isn't in
  the properties, and a panel whose `floating` key is set looks as it did.
- **Debug tool:** each output's `layer_surfaces` (their `geometry` and `exclusive_zone`) and
  `work_area` in `DumpState`, and `dconf watch /org/mate/panel/toplevels/` for the `floating`
  key.

### Intellihide under a maximised window

- **Milestone:** `dock-applet`, with the Dock applet's other items.
- **Steps:**
  1. With `mate-panel-caminito` at `pkgrel` 15 or later, log into *MATE on Caminito*. Add a
     panel along the bottom of the primary output, with the *Dock* applet or any applet, and open
     its *Properties*. Turn on *Intellihide*, which is
     beside *Autohide* and turns it off.
  2. Maximise a window (`pluma`). Then drag another window, not maximised, over the panel, and
     away from it.
  3. Put the pointer on the bottom row of pixels of the output under the panel and leave it
     there for a second. Move it up into the window, and down to the edge again for less than a
     quarter of a second.
  4. Switch to another workspace that has no window over the panel and back, and open the
     panel's menu (*Applications*, if it has one) with the pointer at the edge.
  5. Turn *Intellihide* off with the window still maximised. Log in once to *MATE on Wayfire*,
     or any other Wayland session, or to *MATE*, and open the same panel's *Properties*.
- **Expected:** in step 1 the panel reserves no space: a maximised window reaches the bottom of
  the output. In step 2 the panel slides down out of the output as the window covers it, over
  about 200 ms, and slides back when the window no longer overlaps it. In step 3 it slides up
  after the pointer has rested at the edge for about a quarter of a second, stays while the
  pointer is on it, slides down when the pointer leaves, and a short visit to the edge does
  nothing. In step 4 the panel is shown on the workspace where nothing overlaps it, and stays
  shown while its menu is open. In step 5 the panel is where it was, reserving its edge again,
  and under any other compositor *Properties* has no *Intellihide* (CMN-224, ADR 0047).
- **Debug tool:** `DumpState`'s `outputs`, whose panel's layer surface has `intellihide` with
  `hidden` and where it is `drawn`.
