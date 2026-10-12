# The portal left from the last Session has no display

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, and the checklist item, which needs a second login to a Session on the built package, is the developer's
Milestone: M5
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

Found while testing [182](182-about-caminito.md) on 2026-10-12: no link in *About Caminito* opened. Each click did start Firefox, which exited with `Error: no DISPLAY environment variable specified`.

- **GTK4 opens a link through the portal's OpenURI,** and xdg-desktop-portal gives what it launches its own environment.
- **That portal had neither `WAYLAND_DISPLAY` nor `DISPLAY`.** It started at 02:39:15, as the Session before logged out, and the new Caminito at 02:39:20.
- **How it came to start then:** mate-session restarts the user's bus as the user's last session ends ([74](74-systemd-environment-after-logout.md)). `xdg-desktop-portal.service` requires `dbus.service`, so systemd restarts it with the bus, after Caminito has had systemd forget the display variables.
- **A user manager that outlives the login keeps that portal** for the next one: a second login of the user, as on the developer's machine, or lingering. The portal is started on demand and is never restarted by itself.
- **xdg-desktop-portal-gtk isn't left the same way.** It exits with `cannot open display` until its start limit, and the bus starts it again in the new Session with the right environment.

So from the second login on, a link in any GTK4 program doesn't open, and nor does anything else the portal launches.

## What to build

- **Once systemd's user manager has taken the Session's environment, Caminito has it restart the portal:** `TryRestartUnit` of `xdg-desktop-portal.service`, mode `replace`, which does nothing to a unit that isn't running.
- **The session command is spawned only after the manager answered,** as for the environment, so no client asks the old portal.
- **A machine without xdg-desktop-portal** has no such unit, and that is no failure and no line in the log. Any other failure is one `warn!`, and the Session starts anyway.
- **A nested run's private bus** has no systemd on it (14), and nothing is restarted there.

## Acceptance criteria

- [x] A headless test, written before the code: the manager's stand-in is asked to restart the portal after `SetEnvironment` and before the command starts.
- [x] A headless test: a manager with no unit for the portal starts the Session with no warning.
- [ ] After a logout and a second login to a Session, with the user manager left running, the portal's process has this Session's `WAYLAND_DISPLAY` and `DISPLAY`, and a link in *About Caminito* opens in the browser.
- [x] [checklist.md](../../docs/testing/checklist.md) has a by-eye entry for it.

## Done (2026-10-12)

- **The call is in `dbus::systemd::export`,** after `SetEnvironment`, and `Exported` has its answer for `session` to log. It is made only when the manager took the environment.
- **A unit that doesn't exist is told apart by the error's name,** `org.freedesktop.systemd1.NoSuchUnit`, which the systemd here (`busctl --user call … TryRestartUnit ss caminito-no-such-unit.service replace`) answers with.
- **The answer is to the request, not to the restart:** `TryRestartUnit` returns the job once it is queued. A client that asks for the portal while it restarts is held by the bus until the unit has its name.
- **Only the portal itself is restarted, not its backends.** `mate-portals.conf` names the GTK one, which can't run without a display. A backend that can, left from a login to another desktop, isn't covered.
- **`Event::Exported` is boxed now:** the third result made the variant large enough for clippy's `large_enum_variant`.
- **Written before the code,** in `tests/session.rs`: the stand-in for the manager got `TryRestartUnit`, and `the_portal_is_restarted_once_systemd_has_the_environment_and_before_the_command_starts` failed for want of the call. The five tests that list the stand-in's calls have the new one in them.
- **Seen on the developer's machine, by hand:** `systemctl --user restart xdg-desktop-portal.service` in the running Session gave the portal `WAYLAND_DISPLAY=wayland-1` and `DISPLAY=:1`. Whether the links then opened wasn't reported yet.
- **Not run: a second login on a build with this change.** That is the checklist's new item, "A link opens in the browser after a second login".
- **Checks:** `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass. `cargo test -p caminito` under `dbus-run-session` had 1809 passed and one failed, `not_responding::a_window_whose_client_answers_the_ping_gets_no_dialog::x11`, which passed three times alone afterwards and isn't this ticket's. The rest of `tools/check` wasn't run.
