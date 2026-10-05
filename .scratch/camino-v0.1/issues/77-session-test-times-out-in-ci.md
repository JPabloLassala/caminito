# A session test timed out in CI

Type: implementation
Status: open
Milestone: M5
Blocked by: 61
Map: [Camino v0.1](../map.md)

## What to build

`tests::session::the_activation_environment_has_the_variables_before_the_command_starts` ([61](61-session-start-up-and-exit.md)) failed in the CI job of the first push that had it, `6c21223` on 2026-10-05 (run 37317288069): `the condition didn't hold within 5 s`, at `camino/src/tests/session.rs:299`. That's the wait for the file the bus-activated service writes. The other 591 tests of that run passed. [76](76-tty-login-left-over.md) found it while looking at the Packaging job.

- **It isn't the code path being broken everywhere.** The Packaging job of the same push ran the same test in `camino-git`'s `check()`, in release, and passed. On the developer's machine it passed five times in a row alone, in about 0.5 s each.
- **Not known:** whether it's the 5 s limit under the load of the full debug run in CI's container, or the bus there not activating the service at all. The test's session command asks for the activation with `dbus-send` and doesn't wait for an answer, so a failed activation looks the same as a slow one.
- **What to do:** find which of the two it is, by the test's own output when it fails (what `dbus-send` and the bus said), and fix the test or the code. Raising the limit without knowing isn't a fix.

## Acceptance criteria

- [ ] The cause is written down in this ticket's notes.
- [ ] When the wait fails, the test's panic says what the activation's request answered.
- [ ] CI is green on a push, and on ten runs in a row of the test under the full suite's load.

## Notes

- [73](73-packaging-and-closing-m5.md) asks for CI green on its closing commit's parent, so this blocks it.
