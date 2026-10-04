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
  one, whichever side it was on before, and the pointer and windows can go to it again. The log
  has no `can't light` line.
- **Debug tool:** `wlr-randr`, the `outputs` and windows of `DumpState`, and `~/camino.log`.

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
