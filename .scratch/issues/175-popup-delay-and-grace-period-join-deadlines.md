# The Window switcher's popup delay and the Global menu's grace period join `deadlines`

Epic: caminito-v0.1
Type: implementation
Status: resolved
Blocked by: 174
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

[174](174-deadlines-module.md) gave Caminito one module for the things that wait on its clock. Two one-shot delays are still calloop timers of their own, so a test cannot move the clock past them:

- **The Window switcher's popup delay** (`POPUP_DELAY`, `caminito/src/switcher/mod.rs`): the timer's token is in the open switcher's `timer` field, and the closure shows the popup. `caminito/src/tests/switcher.rs` waits 150 ms of real time in two tests to see that no popup comes.
- **The Global menu's grace period** (`global_menu_applet_changed`, `caminito/src/dbus/global_menu.rs`): one timer for each grace period, which asks `Switch::time_passed` and waits again with `TimeoutAction::ToDuration`. `Switch` already takes `now`.

## What to build

- **The popup delay becomes a stored deadline.** The open switcher keeps the time the popup is due at, in place of a timer's token. It joins the list in `deadlines` with a deadline and an elapse that shows the popup. A switcher that closes before then takes its deadline with it.
- **The grace period becomes a deadline of `Switch`.** `Switch` says when its grace period ends; the elapse is `global_menu_time_passed`, with what follows it now. It joins the list. The timer in `global_menu_applet_changed` goes.
- **The two real-time waits in `tests/switcher.rs` go.** Each becomes an `advance_clock` past the delay. The comment that gives the reason for the 150 ms goes with them.

Each holder is run in the order of the list, as 174 says, and nothing else about `deadlines` changes.

## Acceptance criteria

- [x] Headless tests through the fixture, written before the code and seen to fail: the popup shows after `advance_clock(POPUP_DELAY)` and not before; with the applet gone from the bus, the Global menu's offer ends after `advance_clock` past the grace period and not before, and an applet that comes back within it keeps the offer.
- [x] `tests/switcher.rs` has no `dispatch_for`.
- [x] `switcher/mod.rs` and `dbus/global_menu.rs` have no `Timer::from_duration`.
- [x] `tests::switcher` and `tests::global_menu` pass, and `Switch`'s unit tests pass.
- [x] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section: where each deadline is stored, and whether `Switch`'s unit tests need a case for its new deadline.

## Done (2026-10-11)

- **Tested first:** the tests were changed before the code and run. Four failed: `tests::switcher::the_popup_is_not_drawn_before_its_delay` (xdg and x11; no popup after `advance_clock`), `tests::global_menu::the_capability_follows_the_applets_bus_name` (the capability stayed for 5 s after `advance_clock(GRACE)`) and `tests::global_menu::the_capability_stays_for_the_grace_period_after_the_applet_leaves` (the switch was `leaving` after the grace period). They pass now. The applet that comes back is `a_restart_of_the_panel_within_the_grace_period_changes_nothing`, which goes through `advance_clock(GRACE)` because the helper `the_grace_period_passes` is now that call; `tests::menu_claim` uses the same helper.
- **Where the popup's deadline is stored:** `OpenSwitcher::popup_due`, in place of `timer`. `switch_action` sets it to `now + POPUP_DELAY` for a switcher with a popup, and a `cycle-*` action sets none. `refresh_switcher` drops the open switcher and the deadline with it. The holder is `Holder::SwitcherPopup`, with `Caminito::switcher_popup_deadline` and `switcher_popup_elapse(now)`.
- **Where the grace period's deadline is stored:** it was there already, as `until` in `Switch::Leaving`. `Switch::deadline()` gives it. The holder is `Holder::GlobalMenu`, and its elapse is `global_menu_time_passed(now)`, the name the ticket gives it; the other holders' are `*_elapse`. The registrar's name and the properties follow in the refresh pass, as they did after the timer.
- **`Switch::applet_changed` and `Switch::time_passed` return nothing now.** The grace period's length and the time left of it were for the timer only.
- **`Switch`'s unit tests did need the deadline:** they asserted on the two returned values, so each now asserts on `deadline()`. One case is new, `losing_the_name_again_keeps_the_grace_period_that_runs`.
- **The order in `HOLDERS`:** policy, idle, AccessX, the Global menu's grace period, the Window switcher's popup, the bell, the minimise animations. The grace period is after the three that others look at, and nothing in the list looks at it. The popup is with those that only draw, after idle, whose lock closes the switcher.
- **"Not before" has a margin of real time in the popup's test.** The tests' clock is real time plus the offset, so the test moves the clock a third of the delay and has the 60 ms left for its own run. Before this ticket the same test had 90 ms. A move to 1 ms before the end failed on x11 in the first run. The grace period's test has one second.
- **Not changed:** `show` and one more test in `tests/switcher.rs` still dispatch until the popup is visible, which the loop timer of `deadlines` now ends. The ticket names only the two `dispatch_for` waits.
- **Code review:** no hard finding on either axis. The findings were the name `global_menu_time_passed` and the real-time margin, both above.
- **What was run:** `cargo fmt --all`, `cargo clippy -p caminito --all-targets -- -D warnings`, and 170 tests of `caminito` in the OrbStack machine with `175-test.sh`: `tests::switcher`, `tests::global_menu`, `tests::menu_claim`, `tests::app_menu_registrar`, and the unit tests of `dbus::global_menu` and `deadlines`. **Not run:** the full suite and the other steps of `tools/check` (cargo-deny, the other crates' tests, the palette helper, the helpers' tests, caminito-locker, the extension). The change touches none of them.
