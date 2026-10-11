# The accelerator repeat and Night Light's timers join `deadlines`

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Blocked by: 174
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

[174](174-deadlines-module.md) gave Caminito one module for the things that wait on its clock. Three periodic timers are still calloop timers of their own:

- **The accelerator repeat** (`start_accelerator_repeat`, `caminito/src/input/repeat.rs`): a timer that fires after the repeat delay and then each period, through `TimeoutAction::ToDuration`. `caminito/src/tests/accelerators.rs` waits in real time in four tests.
- **Night Light's minute** (`start_night_light`, `caminito/src/night_light/mod.rs`): works the sun out about once a minute. `Fixture::advance_clock` and `Fixture::set_wall_clock` call `night_light_tick` by name.
- **Night Light's fade step** (`step_night_light`): wakes the loop each `STEP` while a fade runs, so that the refresh pass hands the outputs' temperatures on.

## What to build

- **Each becomes a holder in `deadlines`,** with a stored time it is next due at, a deadline and an elapse. The three `Timer::from_duration` calls go, with the timer tokens in `Repeat` and `night_light.stepping`.
- **No catch-up.** A periodic holder that runs late runs one time, and is next due one period after the `now` it ran at. It is what calloop's `ToDuration` does today when the loop is late, and it prevents a burst of repeats, or of the sun's working-out, after the machine resumes from suspend. A test that wants two repeats moves the clock two times.
- **The repeat's timestamps do not change:** each signalled repeat carries the time the first press had plus the delay and the periods, as now.
- **`advance_clock` names no holder.** Its `night_light_tick` call goes. `set_wall_clock` keeps its call, because a jump of the wall clock is not time that passed.
- **The four real-time waits in `tests/accelerators.rs` go.** Each becomes `advance_clock` calls: one past the delay, then one for each repeat that the test wants. A test that checks that no repeat comes moves the clock too.

Each holder is run in the order of the list, as 174 says, and nothing else about `deadlines` changes.

## Acceptance criteria

- [ ] Headless tests through the fixture, written before the code and seen to fail: a held accelerator signals its first repeat after `advance_clock` past the delay and one more for each `advance_clock` of a period; one `advance_clock` of many periods signals one repeat, not many.
- [ ] `tests/accelerators.rs` has no `dispatch_for`.
- [ ] `input/repeat.rs` and `night_light/mod.rs` have no `Timer::from_duration`.
- [ ] `Fixture::advance_clock` names no holder.
- [ ] `tests::accelerators`, `tests::night_light` and `tests::gamma_control` pass.
- [ ] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- Whether the fade step is a holder with an empty elapse (it only has to wake the loop) or something simpler that the refresh pass can do with the one timer.
- Whether the minute's next time is kept beside `night_light.worked_out` or derived from it.
