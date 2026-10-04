# Session start-up and exit

Type: implementation
Status: open
Milestone: M5
Blocked by: 62
Map: [Camino v0.1](../map.md)

## What to build

`camino --session` ([08](08-session-launch-model.md), [ADR 0004](../../../docs/adr/0004-mate-session-as-camino-child-on-xwayland.md)), in the `session` module ([15](15-crate-and-module-architecture.md)): the start-up order, the environment export, mate-session as Camino's child, and the Session ending with it. `main.rs` stops refusing `--session` (20).

- **Arguments:** `--session` spawns `mate-session --disable-acceleration-check`, and `--session=CMD` spawns `CMD` instead. Bare `camino` is unchanged: no child, and no export beyond what it does now.
- **Start-up order (08):**
  1. the backend up and the outputs lit;
  2. the Wayland socket open;
  3. Xwayland and `camino-gtk-palette` started, as now;
  4. wait for Xwayland to be ready, with the root properties published (38). If Xwayland never comes up, the session still starts, with no `DISPLAY` and an error in the log;
  5. wait until `dbus` owns its names ([62](62-screensaver-names.md));
  6. export the environment;
  7. spawn the session command with `GDK_BACKEND=x11`.
- **The environment (08):** `WAYLAND_DISPLAY`, `DISPLAY`, `XDG_SESSION_TYPE=wayland`, `XDG_CURRENT_DESKTOP=MATE` only when it isn't already set, `MATE_WM_NAME=Camino`, `MATE_WM_KEYBINDINGS=Camino`, and `XCURSOR_THEME` and `XCURSOR_SIZE` from `org.mate.peripherals-mouse`.
  - It's kept in `Camino::environment` (34), not set in the process: `set_var` isn't safe once the settings and D-Bus threads run.
  - It's pushed to the D-Bus activation environment (`UpdateActivationEnvironment`) and to the systemd user manager (`SetEnvironment`), and Camino waits for both before it spawns.
  - **The systemd check ([14](14-development-and-testing-loop.md)):** when `org.freedesktop.systemd1` has no owner on Camino's bus, the systemd part is skipped and logged once. Camino never runs `systemctl`.
- **The Session ends with its child (08):** when the session command exits, Camino tears down and exits with the command's status.
  1. SIGTERM to its helpers and Xwayland.
  2. The Wayland socket closed.
  3. `WAYLAND_DISPLAY`, `DISPLAY`, `MATE_WM_*` and `XCURSOR_*` unset in the systemd user manager (`UnsetEnvironment`), when it was there to set them.
  4. The VT given back.
- **SIGTERM or SIGINT to Camino:** forwarded to the session command as SIGTERM. Camino waits up to 5 s for it to exit, then runs the same teardown.
- **No quit binding** in a Session (08). Bare `camino` keeps what it has.
- **Logging (14):** under `--session` with stderr not a tty, logs go to journald through `tracing-journald`; otherwise to stderr as now. The helpers' stderr goes to the journal the same way (`StderrTarget::Journal`, 28).

## Acceptance criteria

- [ ] Unit tests, written before the code: the arguments (`--session`, `--session=CMD`, neither); the environment built from an Xwayland display, a cursor theme and size, with `XDG_CURRENT_DESKTOP` set and unset, and with no `DISPLAY`.
- [ ] Headless tests on the tests' private bus, with a stand-in session command:
  - the command starts only after Xwayland is ready and the names of 62 are owned, and it's given every variable above and `GDK_BACKEND=x11`;
  - the D-Bus activation environment has the variables before the command starts (a D-Bus-activated test service sees them);
  - with no `org.freedesktop.systemd1` on the bus, the systemd export is skipped and logged once, and the session still starts;
  - with a stand-in `org.freedesktop.systemd1`, `SetEnvironment` is called before the command starts and `UnsetEnvironment` on exit;
  - the command exiting with status 3 ends the loop with status 3, after Xwayland and the palette helper have been stopped;
  - SIGTERM to Camino reaches the command, and a command that ignores it is given up on after 5 s.
- [ ] Nested, `camino --session=foot` shows foot, and closing foot ends Camino with foot's status.
- [ ] Nested, `camino --session` on a private bus starts mate-session, and the log says the systemd export was skipped.

## Notes

- **Nested runs:** `--session` under `tools/dev-nested` is a nested run, not a Session (CONTEXT.md). The real systemd path first runs in [72](72-session-file-and-first-login.md), as `caminotest`.
- **mate-session's patch** ([69](69-mate-session-manager-camino.md)) isn't needed for this ticket's tests. Unpatched, mate-session starts Marco, which finds no X window manager selection free and exits; that's 72's concern.
- **An Xwayland crash still ends the session** (11): mate-session is an X11 client and exits when its display goes. Camino restarts Xwayland as it does now, then tears down because its child exited.
- **The journald path can't be tested in CI,** whose container has no journald (28). It's checked in 72.
