# The systemd environment stays after logout

Type: implementation
Status: open
Milestone: M5
Blocked by: 61
Map: [Camino v0.1](../map.md)

## What to build

After some logouts, systemd's user manager still has the variables Camino exported. [72](72-session-file-and-first-login.md) found it in its run on Xvfb as `caminotest`: in two of five runs from a new user manager, `systemctl --user show-environment` had `WAYLAND_DISPLAY`, `DISPLAY`, `MATE_WM_NAME` and `XCURSOR_SIZE` after Camino exited, and Camino logged `systemd didn't unset the environment within 5s`. In the other three they were gone. `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` stay each time, as [61](61-session-start-up-and-exit.md) decided.

- **The cause:** as mate-session exits, it restarts the user's message bus when the session is the user's last (`maybe_restart_user_bus` in `gsm-manager.c`: `TryRestartUnit` of `dbus.service`, from `do_phase_exit`). Camino's connection is to the bus that stops. Its `UnsetEnvironment` (61) races that: the bus stops about 1 ms after mate-session exits. When the request loses, it gets no answer and no error.
- **Two orders were seen when it failed.** In one, mate-session exits first and the bus stops 1 ms later. In the other, the bus goes first: zbus logs `Failed to parse NameLost signal`, and Camino logs `stopping the session command` before `the session command exited`. So the loss of the bus is also treated as a request to stop, which is harmless here but isn't what the log says.
- **What the leftover variables do:** systemd starts the user's services again on the new bus with a `DISPLAY` that is gone. `xdg-desktop-portal-gtk` failed with `cannot open display: :2` until its start limit. A user manager that outlives the login (lingering, or a second login of the same user, as on the developer's machine) keeps them for the next session of any kind.
- **What to decide:** how Camino has systemd forget the variables when its connection is gone. One way: when the call fails or doesn't answer, make a new connection to the session bus, which `dbus.socket` starts again, and call `UnsetEnvironment` there, all within 61's 5 s. Another: a carried patch that takes the restart out of mate-session under Camino ([69](69-mate-session-manager-camino.md)), which leaves the old session's bus services running.

## Acceptance criteria

- [ ] A headless test, written before the code, in which the bus Camino exported on goes away as the session command exits, and systemd's stand-in on the bus that replaces it is asked to unset the variables.
- [ ] Ten runs in a row of `72-checks/xvfb-login.sh` print only `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` under *the systemd environment after*, and Camino's journal has no `systemd didn't unset the environment`.
- [ ] Camino still exits within 5 s of the session command when no bus comes back.

## Notes

- **How to see it:** `sudo .scratch/camino-v0.1/issues/72-checks/xvfb-login.sh` in a machine with systemd, on the installed packages. It needs no tty. It fails about every second run.
- **Whether a login on a tty restarts the bus too** wasn't checked: the run on Xvfb has no logind session, so mate-session's "last session for the user" is true there. With one login of `caminotest` on a tty it should be true as well. [76](76-tty-login-left-over.md) looks.
