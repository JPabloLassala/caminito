# The locker command, its restarts and the give-up state

Epic: caminito-v0.1
Type: implementation
Status: open
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

- [ ] Headless tests, written before the code, with `locker-command` set to a test locker:
  - a lock request spawns it, and its `lock` is accepted;
  - killed while locked, it comes back and takes the lock over, with the session locked in between (14);
  - a command that always fails reaches the give-up state after five starts, and the dump says so and has the message with the session's ID (14);
  - with `disable-lock-screen` the request is refused.
- [ ] The key is in the schema, and a change of it is used by the next lock.
- [ ] The checklist has *How the give-up state looks*, tagged M7 (17), and it was run nested with `locker-command` set to `['false']`. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **An empty or missing command** is a failed start, so it ends in the give-up state. It doesn't leave the session unlocked.
- **Leaving the give-up state** is logind's `Unlock`, which is [130](130-logind-lock-unlock-and-sleep.md)'s.
