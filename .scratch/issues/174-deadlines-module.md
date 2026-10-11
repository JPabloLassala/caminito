# The `deadlines` module: one seam for Caminito's clock

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
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

- [ ] A headless test through the fixture, written before the code and seen to fail: with slow keys on, a key held past its delay reaches the client after `advance_clock`, with no real wait.
- [ ] The tests that call `advance_clock` pass without change: `tests::idle`, `tests::dpms`, `tests::screensaver`, `tests::bell`, `tests::focus_modes`, `tests::gamma_control`, `tests::night_light`.
- [ ] `grep -rn "struct Wake" caminito/src` finds nothing, and `policy_timer.rs` is gone.
- [ ] `Fixture::advance_clock` calls `run_due` and names no holder but Night Light.
- [ ] A unit test of the keep-or-replace decision, only if that decision comes out as a pure function without effort. If it does not, say so in the Done section.
- [ ] The rule is in `CODING_STANDARDS.md`.
- [ ] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- The exact place of the call in the refresh pass, with its "after" comment.
- How `run_due` reaches each holder's effects, given that idle's and AccessX's need `State` and the bell's needs only `Caminito`.
