# The accelerator repeat and Night Light's timers join `deadlines`

Epic: caminito-v0.1
Type: implementation
Status: resolved
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

- [x] Headless tests through the fixture, written before the code and seen to fail: a held accelerator signals its first repeat after `advance_clock` past the delay and one more for each `advance_clock` of a period; one `advance_clock` of many periods signals one repeat, not many.
- [x] `tests/accelerators.rs` has no `dispatch_for`.
- [x] `input/repeat.rs` and `night_light/mod.rs` have no `Timer::from_duration`.
- [x] `Fixture::advance_clock` names no holder.
- [x] `tests::accelerators`, `tests::night_light` and `tests::gamma_control` pass.
- [x] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- Whether the fade step is a holder with an empty elapse (it only has to wake the loop) or something simpler that the refresh pass can do with the one timer.
- Whether the minute's next time is kept beside `night_light.worked_out` or derived from it.

## Done (2026-10-11)

- **Tested first:** `tests::accelerators::a_held_grabbed_key_signals_repeats_at_the_keyboards_delay_and_rate` (one repeat after `advance_clock` past the delay, and one for each `advance_clock` of a period) and `a_held_grabbed_key_signals_one_repeat_for_many_periods_passed_at_once`. With the calloop timers still there, the two failed, with the three tests of what ends the repeats ("the condition didn't hold within 5 s"). They pass now.
- **The tests' repeat is slow:** a delay of 20 s and a period of 1 s, so that real time brings no repeat in a test and only `advance_clock` does. The loop timer of `deadlines` counts real time in a test too.
- **The repeat:** `Repeat` keeps `due`, on Caminito's clock. `accelerator_repeat_elapse` signals one repeat and sets `due` to `now` plus the period. The timestamps and the conditions that end the repeats did not change. The warning "can't repeat the accelerator" went with its timer.
- **Settled, the fade step:** no holder of its own and no stored time. There is one `NightLight` holder, and its deadline is the earlier of the minute and, while a fade runs, `now + STEP`. The step has no elapse: the loop timer wakes the loop, and the refresh pass hands the temperatures on. The deadline moves later at each refresh pass, but `serves` keeps a timer that is due earlier, so each step fires. This is why `NightLight::deadline` takes `now`, which the other holders' `deadline()` do not.
- **Settled, the minute:** derived from `night_light.worked_out`, a minute after the last working-out. A working-out for a settings change, a resume or a jump of the clock thus moves the next minute later; before, the minute was at a fixed rate. No catch-up follows from this.
- **The order:** the repeat is after AccessX, whose keys it follows, and Night Light is last, because it only moves temperatures.
- **`set_wall_clock`** keeps its `night_light_tick` call, as the ticket says.
- **Not tested:** no test shows that the loop wakes for a fade's step or for the minute in real time. `advance_clock` calls `run_due` itself, and the fixture's dispatch runs the refresh pass at each 10 ms.
- **Run** (OrbStack machine, aarch64): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test -p caminito --lib` for `tests::accelerators`, `tests::night_light`, `tests::gamma_control`, the `deadlines` and `night_light` unit tests, and the other modules that call `advance_clock` (`tests::idle`, `tests::accessx`, `tests::focus_modes`, `tests::minimise_animation`, `tests::bell`, `tests::screensaver`, `tests::dpms`) with `tests::lock`. All passed.
- **Not run:** the full `caminito` suite and the other steps of `tools/check` (cargo-deny, the other crates' tests, the palette helper, the helpers' tests, caminito-locker, the extension). The change touches none of them.
