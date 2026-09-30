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
