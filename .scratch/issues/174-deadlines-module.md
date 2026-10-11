# The `deadlines` module: one seam for Caminito's clock

Epic: caminito-v0.1
Type: implementation
Status: resolved
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

Policy, idle, AccessX and the bell each have a pure state with a deadline, and each has its own copy of the loop glue that sets a calloop timer for it:

- `caminito/src/policy_timer.rs` (`Wake`, `wake_policy`, `policy_elapse`)
- `caminito/src/idle/mod.rs` (`Wake`, `wake_idle`, `idle_elapse`)
- `caminito/src/input/accessx/apply.rs` (`Wake`, `wake_accessx`)
- `caminito/src/bell.rs` (`set_flash_timer`, `bell_elapse`)

The copies differ a little in when they keep a timer and when they replace it. `Fixture::advance_clock` (`caminito/src/tests/fixture.rs`) calls `idle_elapse`, `policy_elapse`, `bell_elapse` and `night_light_tick` by name, so each new feature that waits on the clock edits the fixture ([153](153-focus-modes-and-auto-raise.md) and [169](169-visual-bell.md) both did). AccessX is not in that list, so a test cannot move the clock past a slow key's delay.

This was decided in an architecture review on 2026-10-11. [175](175-popup-delay-and-grace-period-join-deadlines.md) and [176](176-repeat-and-night-light-join-deadlines.md) add the other holders.

## What to build

A module `caminito/src/deadlines.rs` that owns the question "who is due, and when must the loop wake".

- **A closed list of deadline holders.** The module knows each holder. For each one it can read the holder's deadline and run what the holder has due. The holders in this ticket are policy, idle, AccessX and the bell's flash. The pure states keep their `deadline()` and `elapse(now)`; they do not change.
- **One `run_due`.** It reads Caminito's clock one time and runs each holder that is due at that `now`, in the order of the list. The order is written in a comment, as the refresh pass's order is. Each holder's effects are applied as they are now (`idle_effects`, `accessx_effects`, the end of the flash).
- **One calloop timer,** for the earliest deadline of all the holders. When it fires it calls `run_due`.
- **One place that sets the timer:** one call in the refresh pass (`State::refresh_and_flush_clients`), in place of `wake_policy`. It must come after the pointer's crossings and after idle's refresh, because those change deadlines. The rule is the one idle and policy have now: a timer that is due at or before the earliest deadline is kept, because it finds nothing to do and the refresh pass after it sets the next. AccessX's rule (replace unless equal) goes away. Every `elapse` is safe when nothing is due.
- **The fixture:** `advance_clock` adds to `clock_offset`, calls `run_due` and dispatches. It names no holder but Night Light, whose `night_light_tick` call stays until 176. The tests' clock stays real time plus the offset; a clock that moves only when a test moves it is not part of this work.
- **Deleted:** `policy_timer.rs`, the three `Wake` structs, `wake_policy`, `wake_idle`, `wake_accessx`, `set_flash_timer`, and the fields on `Caminito` and `Idle` that held their timers. `idle_elapse`, `policy_elapse` and `bell_elapse` stay only if something other than `run_due` calls them.
- **`CODING_STANDARDS.md`** gets one rule: a feature that waits on Caminito's clock gives a `deadline()` and an `elapse(now)` and joins the list in `deadlines`; it sets a calloop `Timer` of its own only when it waits on a process or a protocol object.

Not holders, and not to be changed: the timers that wait on a process or a protocol object (`session.rs`, `helpers.rs`, `xwayland/restart.rs`, `dbus/logind.rs`, `backend/tty.rs`, `output.rs`'s global removal, `handlers/compositor.rs`).

No entry in `CONTEXT.md`: `deadlines` is a mechanism, not domain language.

## Acceptance criteria

- [x] A headless test through the fixture, written before the code and seen to fail: with slow keys on, a key held past its delay reaches the client after `advance_clock`, with no real wait.
- [x] The tests that call `advance_clock` pass without change: `tests::idle`, `tests::dpms`, `tests::screensaver`, `tests::bell`, `tests::focus_modes`, `tests::gamma_control`, `tests::night_light`.
- [x] `grep -rn "struct Wake" caminito/src` finds nothing, and `policy_timer.rs` is gone.
- [x] `Fixture::advance_clock` calls `run_due` and names no holder but Night Light.
- [x] A unit test of the keep-or-replace decision, only if that decision comes out as a pure function without effort. If it does not, say so in the Done section.
- [x] The rule is in `CODING_STANDARDS.md`.
- [x] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- The exact place of the call in the refresh pass, with its "after" comment.
- How `run_due` reaches each holder's effects, given that idle's and AccessX's need `State` and the bell's needs only `Caminito`.

## Done (2026-10-11)

- **Tested first:** `tests::accessx::a_key_slow_keys_hold_back_reaches_the_client_once_the_clock_is_past_its_delay` holds a key with a slow-keys delay of one minute and moves the clock 59 s and then 1 s. It failed before the code (no key after `advance_clock`) and passes now.
- **The place of the call:** `self.caminito.refresh_deadlines()` is the last step of `State::refresh_and_flush_clients` before `flush_clients`, not the place `wake_policy` had. Its comment: "After everything that moves a deadline (174): the pointer's crossings, which queue an auto-raise, idle's refresh, the minimise animations' start, and the handlers before this pass." `refresh_minimize_animations` runs after the place `wake_policy` had, so that place was too early.
- **How `run_due` reaches the effects:** the list is an enum, `Holder`, with a `HOLDERS` array for the order. `Holder::elapse` takes `&mut State` and `now`. Policy's is `policy.elapse(now)`. Idle's and AccessX's are `State::idle_elapse(now)` and `State::accessx_elapse(now)`, which stay in their modules because `idle_effects` and `accessx_effects` are private there. The bell's is `Caminito::bell_elapse(now)`.
- **`idle_elapse` and `bell_elapse` stay, with `now` as a parameter.** Only `run_due` calls them, so this is not the ticket's words to the letter. They no longer read the clock, and they are the only path to the private effects. `policy_elapse` is gone.
- **The minimise animations are a fifth holder.** Ticket 172 merged after this ticket was written, with a step timer of its own and a call in `advance_clock`. Without this, `advance_clock` could not name Night Light alone. `MinimizeAnimations` keeps `next_step`. A late step is not made up for: the next one is `STEP` after the `now` it ran at, which is 176's rule for periodic holders.
- **The keep-or-replace decision is a pure function,** `deadlines::serves(set, earliest)`, with four unit tests.
- **`advance_clock` drops the loop timer** (`Caminito::drop_deadline_timer`). The timer counts real time, so after a move of the clock it is late by that amount, and with one timer for all holders it could be late for a different holder. The dispatch in `advance_clock` sets the next.
- **The new rule in `CODING_STANDARDS.md` is not yet true of all the code.** The switcher's popup delay and the Global menu's grace period are 175's; the accelerator repeat and Night Light's two timers are 176's.
- **What was run:** `cargo fmt --all`, `cargo clippy -p caminito --all-targets -- -D warnings`, and 278 tests of `caminito` in the OrbStack machine: `tests::accessx`, `tests::idle`, `tests::dpms`, `tests::screensaver`, `tests::bell`, `tests::focus_modes`, `tests::gamma_control`, `tests::night_light`, `tests::minimise_animation`, `tests::lock`, `tests::keyboard_monitor`, and the unit tests of `deadlines`, `minimize_animation`, `input::accessx` and `idle`. **Not run:** the full suite and the other steps of `tools/check` (cargo-deny, the other crates' tests, the palette helper, the helpers' tests, caminito-locker, the extension). The change touches none of them.
