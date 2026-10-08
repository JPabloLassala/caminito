# Session start-up and exit

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M5
Blocked by: 62
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`caminito --session` ([08](08-session-launch-model.md), [ADR 0004](../../docs/adr/0004-mate-session-as-caminito-child-on-xwayland.md)), in the `session` module ([15](15-crate-and-module-architecture.md)): the start-up order, the environment export, mate-session as Caminito's child, and the Session ending with it. `main.rs` stops refusing `--session` (20).

- **Arguments:** `--session` spawns `mate-session --disable-acceleration-check`, and `--session=CMD` spawns `CMD` instead. Bare `caminito` is unchanged: no child, and no export beyond what it does now.
- **Start-up order (08):**
  1. the backend up and the outputs lit;
  2. the Wayland socket open;
  3. Xwayland and `caminito-gtk-palette` started, as now;
  4. wait for Xwayland to be ready, with the root properties published (38). If Xwayland never comes up, the session still starts, with no `DISPLAY` and an error in the log;
  5. wait until `dbus` owns its names ([62](62-screensaver-names.md));
  6. export the environment;
  7. spawn the session command with `GDK_BACKEND=x11`.
- **The environment (08):** `WAYLAND_DISPLAY`, `DISPLAY`, `XDG_SESSION_TYPE=wayland`, `XDG_CURRENT_DESKTOP=MATE` only when it isn't already set, `MATE_WM_NAME=Caminito`, `MATE_WM_KEYBINDINGS=Caminito`, and `XCURSOR_THEME` and `XCURSOR_SIZE` from `org.mate.peripherals-mouse`.
  - It's kept in `Caminito::environment` (34), not set in the process: `set_var` isn't safe once the settings and D-Bus threads run.
  - It's pushed to the D-Bus activation environment (`UpdateActivationEnvironment`) and to the systemd user manager (`SetEnvironment`), and Caminito waits for both before it spawns.
  - **The systemd check ([14](14-development-and-testing-loop.md)):** when `org.freedesktop.systemd1` has no owner on Caminito's bus, the systemd part is skipped and logged once. Caminito never runs `systemctl`.
- **The Session ends with its child (08):** when the session command exits, Caminito tears down and exits with the command's status.
  1. SIGTERM to its helpers and Xwayland.
  2. The Wayland socket closed.
  3. `WAYLAND_DISPLAY`, `DISPLAY`, `MATE_WM_*` and `XCURSOR_*` unset in the systemd user manager (`UnsetEnvironment`), when it was there to set them.
  4. The VT given back.
- **SIGTERM or SIGINT to Caminito:** forwarded to the session command as SIGTERM. Caminito waits up to 5 s for it to exit, then runs the same teardown.
- **No quit binding** in a Session (08). Bare `caminito` keeps what it has.
- **Logging (14):** under `--session` with stderr not a tty, logs go to journald through `tracing-journald`; otherwise to stderr as now. The helpers' stderr goes to the journal the same way (`StderrTarget::Journal`, 28).

## Acceptance criteria

- [x] Unit tests, written before the code: the arguments (`--session`, `--session=CMD`, neither); the environment built from an Xwayland display, a cursor theme and size, with `XDG_CURRENT_DESKTOP` set and unset, and with no `DISPLAY`.
- [x] Headless tests on the tests' private bus, with a stand-in session command:
  - the command starts only after Xwayland is ready and the names of 62 are owned, and it's given every variable above and `GDK_BACKEND=x11`;
  - the D-Bus activation environment has the variables before the command starts (a D-Bus-activated test service sees them);
  - with no `org.freedesktop.systemd1` on the bus, the systemd export is skipped and logged once, and the session still starts;
  - with a stand-in `org.freedesktop.systemd1`, `SetEnvironment` is called before the command starts and `UnsetEnvironment` on exit;
  - the command exiting with status 3 ends the loop with status 3, after Xwayland and the palette helper have been stopped;
  - SIGTERM to Caminito reaches the command, and a command that ignores it is given up on after 5 s.
- [x] Nested, `caminito --session=foot` shows foot, and closing foot ends Caminito with foot's status.
- [x] Nested, `caminito --session` on a private bus starts mate-session, and the log says the systemd export was skipped.

## Notes

- **Nested runs:** `--session` under `tools/dev-nested` is a nested run, not a Session (CONTEXT.md). The real systemd path first runs in [72](72-session-file-and-first-login.md), as `caminitotest`.
- **mate-session's patch** ([69](69-mate-session-manager-caminito.md)) isn't needed for this ticket's tests. Unpatched, mate-session starts Marco, which finds no X window manager selection free and exits; that's 72's concern.
- **An Xwayland crash still ends the session** (11): mate-session is an X11 client and exits when its display goes. Caminito restarts Xwayland as it does now, then tears down because its child exited.
- **The journald path can't be tested in CI,** whose container has no journald (28). It's checked in 72.
- **Resolved on 2026-10-05.** Checked in an OrbStack Arch Linux machine (aarch64): the tests in `tests/session.rs`, `session.rs` and `main.rs`, and the full suite on a private bus (`dbus-run-session`). caminito-theme's goldens fail on aarch64, as they do on `master`.
- **Written before the code:** the argument tests, the environment tests and the first seven tests of `tests/session.rs` were written and seen to fail (no `session` module) before the module. The two tests for a stop during the export and for a systemd that doesn't answer came after the review.
- **The nested criteria were checked on Xvfb,** not in a desktop: the machine has none. [61-nested-checks.sh](61-nested-checks.sh) runs them.
  - `--session=foot <script>`: Caminito's state dump, taken from inside foot, lists foot's window, framed and focused, and Caminito exits with the script's status, 5.
  - `--session`: mate-session 1.28 starts, and the panel and caja with it, and the journal has "org.freedesktop.systemd1 isn't on the bus, so the environment isn't exported to systemd".
  - SIGTERM to that Caminito, by hand: foot is stopped, and Caminito exits with foot's status.
- **Not tested:** the signal source itself (the headless tests call `Caminito::quit`, which it calls; the SIGTERM was by hand), a Session with no Xwayland beyond the environment's unit test, the socket closing, and the real systemd path, which is [72](72-session-file-and-first-login.md)'s.
- **Decided here, beyond what the ticket says:**
  - **`--session=CMD` is split at whitespace** into a program and its arguments, with no shell and no quoting. `--session=` with no word is refused.
  - **Caminito's exit status** is the command's, or 128 and the signal that ended it. It's 1 if the command couldn't start, was given up on, or was never spawned.
  - **A command that ignores SIGTERM for 5 s is killed,** so it doesn't outlive the Session.
  - **The winit window closing** is treated as SIGTERM is: the command is stopped first.
  - **A stop before the command is spawned** spawns none. One that comes during the export waits for the bus's answer, so systemd is still asked to forget.
  - **systemd has 5 s to answer `UnsetEnvironment`.** Then Caminito exits anyway, since a systemd that's shutting down may not answer.
  - **Xwayland is sent SIGTERM by its process id, found in `/proc`** among Caminito's children (`xwayland::process`, which was the fixture's). Smithay keeps the `Child` to itself, and its own way to stop Xwayland, dropping it, leaves `X11Wm`'s source calling back for a window manager that's gone.
  - **`XCURSOR_THEME` is exported only when the theme isn't empty,** as Xwayland's own is (34). The default theme has no name to export.
  - **With no `DISPLAY`, the command is spawned with `DISPLAY` removed,** since a nested run has its host's.
  - **All of `Caminito::environment` is exported.** It holds nothing but the Session's variables.
  - **Whether logs go to the journal is decided once,** in `logging::init`, and the helpers' stderr follows what it did, so a journal that can't be reached sends both to stderr.
- **`tools/dev-nested --session` logs to the journal,** since the script pipes Caminito's stderr, so its `WAYLAND_DISPLAY=… DISPLAY=…` line isn't printed. The script's header says so. Whether a nested run should log to stderr whatever it is, is open.
- **"The Wayland socket closed" closes the listening socket.** The clients that are connected stay until Caminito exits, right after.
- **Review left as is:** the `SIGCHLD` pipe is written a third time, after `helpers` and `stop_on_signals`. `Restart::without_x` also means that the Session ended. The tests reach `state.caminito` for the exit status, the palette helper and `quit`, for want of a seam. "Xwayland didn't start" is logged as an error, as the ticket asks, though Caminito carries on.
