# A session test timed out in CI

Type: implementation
Status: resolved
Milestone: M5
Blocked by: 61
Map: [Caminito v0.1](../map.md)

## What to build

`tests::session::the_activation_environment_has_the_variables_before_the_command_starts` ([61](61-session-start-up-and-exit.md)) failed in the CI job of the first push that had it, `6c21223` on 2026-10-05 (run 37317288069): `the condition didn't hold within 5 s`, at `caminito/src/tests/session.rs:299`. That's the wait for the file the bus-activated service writes. The other 591 tests of that run passed. [76](76-tty-login-left-over.md) found it while looking at the Packaging job.

- **It isn't the code path being broken everywhere.** The Packaging job of the same push ran the same test in `caminito-git`'s `check()`, in release, and passed. On the developer's machine it passed five times in a row alone, in about 0.5 s each.
- **Not known:** whether it's the 5 s limit under the load of the full debug run in CI's container, or the bus there not activating the service at all. The test's session command asks for the activation with `dbus-send` and doesn't wait for an answer, so a failed activation looks the same as a slow one.
- **What to do:** find which of the two it is, by the test's own output when it fails (what `dbus-send` and the bus said), and fix the test or the code. Raising the limit without knowing isn't a fix.

## Acceptance criteria

- [x] The cause is written down in this ticket's notes.
- [x] When the wait fails, the test's panic says what the activation's request answered.
- [x] CI is green on a push, and on ten runs in a row of the test under the full suite's load.

## Notes

- [73](73-packaging-and-closing-m5.md) asks for CI green on its closing commit's parent, so this blocks it.
- **The cause: the bus never found the service file,** not the load. A container job's `HOME` is `/github/home`, and dbus-daemon takes its home from the user database, `/root`. `ActivatedService` writes the file below `$HOME/.local/share` when neither `XDG_RUNTIME_DIR` nor `XDG_DATA_HOME` is set, so the bus looked elsewhere and answered `ServiceUnknown`. The Packaging job and the developer's machine have one home, so it passed there. Fixed in CI's environment by `823579b`, which sets `XDG_DATA_HOME=$HOME/.local/share` for the caminito tests. The test itself still writes below `HOME`: anywhere `HOME` isn't the bus's home it still fails, but now it says why.
- **Reproduced (2026-10-06)** in an Ubuntu 24.04 container, with `HOME` set apart from `/root` and no `XDG_DATA_HOME`: the same failure at the same wait. With `3bab833` and `22be6c3` the panic reads `the condition didn't hold within 5 s: the activation with …/home/.local/share/dbus-1/services/org.mate.Caminito.Test.Activated….service answered: Error org.freedesktop.DBus.Error.ServiceUnknown: The name … was not provided by any .service files`.
- **What the answer tells apart:** the command keeps `dbus-send --print-reply`'s output in `answer`, in the background, with a 4 s reply timeout so it ends within the test's wait (the Session kills only its command). A service file the bus doesn't find is answered at once (`ServiceUnknown`). A service that starts is never answered, as it doesn't take its name, so a slow activation, or one that started and failed, shows as no answer, then `NoReply` at 4 s. Checked by hand: the bus doesn't answer when the started program exits without its name; `dbus-send` waits out its timeout.
- **Ten runs (2026-10-06)** of the full `cargo test -p caminito` on `22be6c3`, in that container with CI's `XDG_DATA_HOME`: the test passed in all ten. Five other tests fail in that container on the base commit too (four of `tests::keymap` and `the_pointer_goes_through_an_override_redirect_windows_input_shaped_shadow`), from its xkeyboard-config and Xwayland 23, and `xwayland_crashing_again_and_again_as_it_starts_makes_caminito_give_up_and_run_without_x` failed once, in the first run. CI is on Arch and passed all of them.
- **CI green** on `3bab833` (run 37448826512) and `22be6c3` (run 37449025172), and on `823579b` and `1f68b0b` before them.
