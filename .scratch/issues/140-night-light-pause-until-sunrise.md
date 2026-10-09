# Pause Night Light until sunrise

Epic: night-light
Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: 138
Map: [Night Light](../night-light/map.md)

## What to build

**Pause** (CONTEXT.md), under "Turning it off and pausing it" in [the spec](../night-light/spec.md).

- **`PauseNightLight()`** on `org.mate.Caminito`:
  - It writes `night-light-paused-until` with the next −6° crossing from 138, or the next 07:00 with no location, capped at 24 h from now.
  - It means the same thing under either schedule.
  - It does nothing while Night Light is off.
- **`ResumeNightLight()`** writes 0.
- **Changing `night-light-enabled`** writes 0, whoever changes it.
- **While paused,** the target is 6500 K, faded to. When the time passes, Caminito writes 0 and fades to the schedule. A pause read at start whose time has passed is cleared.
- **A clock jump or a resume from sleep** is checked against the end time at once.
- `DumpState` shows the pause.

## Acceptance criteria

- [ ] Unit tests: pausing at 22:00 ends at tomorrow's −6° crossing; pausing at 10:00 ends at tomorrow's too; a polar night is capped at 24 h; no location ends at 07:00.
- [ ] A headless test with a fake clock: pause, the time passes, the key is 0 and the target is the schedule's. Flipping `enabled` clears a pause. A pause in the key at start that has passed is cleared. No test sleeps.
- [ ] A headless test: a pause survives a restart of Caminito.
- [ ] On the tty, by the developer: pause fades to neutral, resume fades back.
