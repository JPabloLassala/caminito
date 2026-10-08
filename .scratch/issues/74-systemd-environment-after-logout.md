# The systemd environment stays after logout

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M5
Blocked by: 61
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

After some logouts, systemd's user manager still has the variables Caminito exported. [72](72-session-file-and-first-login.md) found it in its run on Xvfb as `caminitotest`: in two of five runs from a new user manager, `systemctl --user show-environment` had `WAYLAND_DISPLAY`, `DISPLAY`, `MATE_WM_NAME` and `XCURSOR_SIZE` after Caminito exited, and Caminito logged `systemd didn't unset the environment within 5s`. In the other three they were gone. `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` stay each time, as [61](61-session-start-up-and-exit.md) decided.

- **The cause:** as mate-session exits, it restarts the user's message bus when the session is the user's last (`maybe_restart_user_bus` in `gsm-manager.c`: `TryRestartUnit` of `dbus.service`, from `do_phase_exit`). Caminito's connection is to the bus that stops. Its `UnsetEnvironment` (61) races that: the bus stops about 1 ms after mate-session exits. When the request loses, it gets no answer and no error.
- **Two orders were seen when it failed.** In one, mate-session exits first and the bus stops 1 ms later. In the other, the bus goes first: zbus logs `Failed to parse NameLost signal`, and Caminito logs `stopping the session command` before `the session command exited`. So the loss of the bus is also treated as a request to stop, which is harmless here but isn't what the log says.
- **What the leftover variables do:** systemd starts the user's services again on the new bus with a `DISPLAY` that is gone. `xdg-desktop-portal-gtk` failed with `cannot open display: :2` until its start limit. A user manager that outlives the login (lingering, or a second login of the same user, as on the developer's machine) keeps them for the next session of any kind.
- **What to decide:** how Caminito has systemd forget the variables when its connection is gone. One way: when the call fails or doesn't answer, make a new connection to the session bus, which `dbus.socket` starts again, and call `UnsetEnvironment` there, all within 61's 5 s. Another: a carried patch that takes the restart out of mate-session under Caminito ([69](69-mate-session-manager-caminito.md)), which leaves the old session's bus services running.

## Acceptance criteria

- [x] A headless test, written before the code, in which the bus Caminito exported on goes away as the session command exits, and systemd's stand-in on the bus that replaces it is asked to unset the variables.
- [x] Ten runs in a row of `72-checks/xvfb-login.sh` print only `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` under *the systemd environment after*, and Caminito's journal has no `systemd didn't unset the environment`.
- [x] Caminito still exits within 5 s of the session command when no bus comes back.

## Notes

- **How to see it:** `sudo .scratch/issues/72-checks/xvfb-login.sh` in a machine with systemd, on the installed packages. It needs no tty. It fails about every second run.
- **Whether a login on a tty restarts the bus too** wasn't checked: the run on Xvfb has no logind session, so mate-session's "last session for the user" is true there. With one login of `caminitotest` on a tty it should be true as well. [76](76-tty-login-left-over.md) looks.
- **Resolved on 2026-10-05.** Decided: Caminito asks again on a new connection. The carried patch that takes the restart out of mate-session wasn't done.
- **The cause wasn't the race alone.** zbus runs a connection's tasks on a thread that ends when the connection has no task left (`connection/builder.rs`, "Run as long as there is a task to run"). When the bus closes, the connection's tasks end, so the thread ends, and the `UnsetEnvironment` that Caminito then spawned on that executor never ran. That is the "no answer and no error".
- **What Caminito does now** (`session.rs`, `dbus/systemd.rs`):
  - Each request to forget is made over a connection of its own, on a thread of its own, never over Caminito's connection.
  - A request that fails is followed by another 50 ms later. One that isn't answered in a fifth of the wait (1 s of the 5 s) is followed by another.
  - After 61's 5 s Caminito exits anyway, and the warning `systemd didn't unset the environment within 5s` now ends with the last request's error, if one failed.
- **Written before the code,** in `tests/session.rs`, each on a `dbus-daemon` of the test's own that the test stops:
  - a bus that stops and is replaced at the same address, whose stand-in for systemd is asked to unset: seen to fail;
  - a systemd that doesn't answer its first request, on the tests' bus: seen to fail;
  - a bus that isn't replaced, given up on after the wait: it passed before the change too, and holds the third criterion.
- **The ten runs** were on the developer's machine (x86_64), as `caminitotest`:
  - Ten in a row with this ticket's build: each printed only `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` after logout, and the journal had no `systemd didn't unset the environment`.
  - Four with `master`'s build, for comparison: three left `DISPLAY`, `MATE_WM_NAME`, `WAYLAND_DISPLAY` and `XCURSOR_SIZE`, with the warning in the journal.
- **Not on the installed packages.** The machine's `caminito-git` is r116, from before `--session`, and its `mate-session-manager` is the stock one. `xvfb-login.sh` now takes a caminito binary as its argument and runs it in place of the installed one, and the runs used that, with stock mate-session and the installed helpers. A run on the built packages is [73](73-packaging-and-closing-m5.md)'s.
- **Not tested:** the order in which the bus stops just after it took a request. The headless tests cover a bus that's gone before the request and a systemd that doesn't answer, and the ten runs don't say which order each had.
- **Review left as is:**
  - Every failure is asked again until the wait is over, since a systemd that isn't back on the new bus yet fails the same way as one that refuses. A request that can never succeed costs the 5 s.
  - A request that's never answered keeps its thread and its connection until Caminito exits.
  - `Dbus` keeps its bus's address, which only a test sets.
  - The loss of the bus is still followed by `stopping the session command`.
- **Left behind by the runs:** `xvfb-login.sh` leaves `caminitotest`'s user manager and some of the session's processes running (`kunifiedpush-distributor`, `keyboxd`, and an Xvfb from a run that was the first), as 72's did.
- **A login on a tty does restart the bus (2026-10-05, 76).** After `caminitotest`'s logout from tty4 on the developer's machine, `dbus.service` was active since the second of the logout, and the bus went first (`Failed to parse NameLost signal`, then `stopping the session command`). That time the variables were unset: only `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` stayed, and Caminito logged no `systemd didn't unset the environment`. One sample, from a user manager that earlier runs of `xvfb-login.sh` had left running.
