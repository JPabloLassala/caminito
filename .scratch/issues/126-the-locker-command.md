# The locker command, its restarts and the give-up state

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, and the nested run of the checklist item is 136's
Milestone: M7
Blocked by: 125
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

A lock request that isn't a locker's own spawns the locker, and Caminito keeps the session locked whatever the locker does ([12](12-screen-locking.md)'s *Which locker* and *Robustness*).

- **The key:** `locker-command` in `org.mate.Caminito` (`as`, default `['caminito-locker']`), an argv with no shell ([09](09-gsettings-mapping.md)).
- **The spawn:** through the `helpers` supervisor, with the locker's stderr in the journal under its own identifier. Its pid is the one [125](125-lock-state-and-ext-session-lock.md)'s take-over rule accepts.
- **A locker that exits while locked** is restarted with the supervisor's backoff, and the new one takes the lock over.
- **The give-up state:** after five failed starts within a minute Caminito stops respawning and stays locked. It draws one line with the theme engine's cairo and pango: switch to a text console and run `loginctl unlock-session <id>`, with the session's ID filled in.
- **A request seam** for the tests and for [127](127-screensaver-shim.md), [128](128-idle-and-blanking.md) and [130](130-logind-lock-unlock-and-sleep.md): one function that asks for a lock, refused and logged under `org.mate.lockdown disable-lock-screen`.

## Acceptance criteria

- [x] Headless tests, written before the code, with `locker-command` set to a test locker:
  - a lock request spawns it, and its `lock` is accepted;
  - killed while locked, it comes back and takes the lock over, with the session locked in between (14);
  - a command that always fails reaches the give-up state after five starts, and the dump says so and has the message with the session's ID (14);
  - with `disable-lock-screen` the request is refused.
- [x] The key is in the schema, and a change of it is used by the next lock.
- [ ] The checklist has *How the give-up state looks*, tagged M7 (17), and it was run nested with `locker-command` set to `['false']`. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **An empty or missing command** is a failed start, so it ends in the give-up state. It doesn't leave the session unlocked.
- **Leaving the give-up state** is logind's `Unlock`, which is [130](130-logind-lock-unlock-and-sleep.md)'s.

## Done (2026-10-09)

- **The seam** is `State::request_lock`, which returns whether the request was accepted. Nothing outside the tests calls it until [127](127-screensaver-shim.md), [128](128-idle-and-blanking.md) and [130](130-logind-lock-unlock-and-sleep.md), so it carries an `expect(dead_code)` for the release build, as 125's `LockState::requested` did.
- **One helper per lock:** each lock that needs a locker gets a new supervised helper, with the command the settings have at the request and a restart count of its own, so a change of `locker-command` is used by the next lock, and a lock after a give-up starts afresh. The helper is finished at the unlock: the locker that unlocked is left to exit and reaped, and isn't started again. A new lock while that one is still exiting kills it.
- **The supervisor** gained an argv (`HelperCommand::args`), the running helper's process, and `Helper::finish`. Its restart policy is unchanged: the backoff from 500 ms, and five exits within a minute give up.
- **The environment:** the locker runs on Caminito's socket, with `GDK_BACKEND=wayland` and `Caminito::environment`, and its stderr goes to the journal under its program's name.
- **The give-up state** is the locker's (`none`, `starting`, `running`, `gave-up` in the dump's `locker`, with its `message`), not a fifth lock state: the lock stays `pending` or `locker-lost`. A lock request while given up spawns nothing, so only logind's `Unlock` ([130](130-logind-lock-unlock-and-sleep.md)) ends it. The message is drawn white on the lock's background in the interface font, wrapped to the output, at each output's scale.
- **The session's ID** is `XDG_SESSION_ID`. Without it the message names `loginctl unlock-sessions`, which unlocks every session, since a bare `unlock-session` from a console acts on the console's own.
- **A forking locker** such as `swaylock -f` can't take the lock: the process that asks isn't the one spawned. The schema's description says so.
- **disable-lock-screen** is read from `org.mate.lockdown`, vendored into `tests/schemas/` from mate-desktop 1.28.2.
- **Not done here:** a lock that a hand-started locker held and lost (`locker-lost` with no locker of Caminito's) gets Caminito's locker only at the next lock request, as the request spawns one when no locker holds the lock.
- **Test first:** the headless tests and the settings tests were written first and failed to build before the code. Each headless test was then checked by taking out the line it covers: without the spawned locker's process the take-over tests fail, and without `finish` at the unlock the test of a locker that unlocks fails.
- **The checklist item** is *How the give-up state looks*, tagged M7. It wasn't run: nothing asks Caminito for a lock in a nested run until 127's `Lock`, and the ticket was done in a container with no display. It's left to [136](136-packaging-and-closing-m7.md).
