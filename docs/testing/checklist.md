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
     installed, use *Add* in *Keyboard Shortcuts* and press the keys there. Without it:

     ```sh
     d=/org/mate/desktop/keybindings/custom-check
     dconf write $d/name "'Calculator'"
     dconf write $d/action "'mate-calc'"
     dconf write $d/binding "'<Primary><Alt>k'"
     ```

  3. Click in a text editor's window, and press Ctrl+Alt+K. Close the calculator, then hold
     Ctrl+Alt+K for two seconds.
  4. Change the shortcut's keys to Shift+Super+1, in *Keyboard Shortcuts* or with `dconf
     write $d/binding "'<Shift><Mod4>exclam'"`. Press Ctrl+Alt+K, then Shift+Super+1.
  5. Remove the shortcut, in *Keyboard Shortcuts* or with `dconf reset -f $d/`. Press
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
- **Debug tool:** the journal's `caminito-locker` lines: `Creating background w:… h:…` for each
  monitor, `Status message: …`, `Switch User: enabled` or `disabled` (and `Warning: Unknown DM
  for switch button` where no display manager's path applies), and `Log Out: after its delay`
  then `Log Out: shown`. `busctl --system get-property org.freedesktop.DisplayManager
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
  logged as a bell, which neither sounds nor flashes until M8 (105, 17).
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

## The control center

### Windows and Keyboard Shortcuts open under Caminito, and name it

- **Milestone:** M6
- **Steps:**
  1. With `mate-control-center-caminito` installed, and `caminito-git` at a build that has the
     `50-caminito-*.xml` manifests (`ls /usr/share/mate-control-center/keybindings`), log into
     *MATE on Caminito*.
  2. Open *Windows* from the control center, with a window that Caminito decorates beside it.
     On *Behaviour*, set the titlebar's double-click action to *Toggle Shade*, double-click that
     window's titlebar, and set it back. On *Placement*, move the titlebar buttons to the
     left and back.
  3. Open *Keyboard Shortcuts* from the control center. Find *Window Management*, and
     *Desktop*'s *Show the panel's "Run Application" dialog box*.
  4. Click the shortcut of *Maximize window*, press Super+Up, and run:

     ```sh
     gsettings get org.mate.Marco.window-keybindings maximize
     ```

     Press Super+Up with a window focused. Then click the shortcut again, press Backspace,
     and run the command again.
  5. Log out, log into *MATE*, the X11 session, and open both dialogs there. Then log into the
     Wayfire session and open both.
- **Expected:** in step 2 the dialog opens, with no *The current window manager is
  unsupported*, and it offers Alt and Super for moving a window. The window shades at the
  double click, and each window's buttons move as the setting does, with the dialog still
  open (06). In step 3 the dialog opens without a crash, and lists Caminito's window management
  actions once each, with the shortcuts Marco's defaults give them: Alt+Tab, Alt+F4,
  Ctrl+Alt+D. In step 4 the key is a string that names Super and Up, `'<Mod4>Up'` or
  `'<Super>Up'`, and then `'disabled'`: the dialog writes the key. Super+Up maximises nothing: **a shortcut changed in the dialog changes no
  binding until M8**, when Caminito reads `org.mate.Marco.*` keybindings in place of its
  hard-coded set (17). In X11 MATE both dialogs are as they were, with Marco's shortcuts
  listed, since the variables aren't read on X11. In the Wayfire session *Windows* says the
  window manager is unsupported, as the stock one does, and *Keyboard Shortcuts* opens, where
  the stock one crashed, and lists no window management actions (101, ADR 0011).
- **Debug tool:** `mate-window-properties` and `mate-keybinding-properties` run from a
  terminal, `echo "$MATE_WM_NAME $MATE_WM_KEYBINDINGS"` there, `gsettings monitor
  org.mate.Marco.general`, and `coredumpctl` for a crash.

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
  focused application's windows, and Ctrl+Alt+Tab lists nothing under Wayland's panel, a Layer
  surface, so nothing happens. In step 6 the popup is on the pointer's monitor. In step 7
  the grid shows icons with no thumbnails (108, ADR 0016).
- **Debug tool:** `org.mate.Caminito.Debug`'s state dump, whose `switcher` member has the list,
  the selection, the popup's output and its cells while the popup shows, and `gsettings
  monitor org.mate.Marco.general`.

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
