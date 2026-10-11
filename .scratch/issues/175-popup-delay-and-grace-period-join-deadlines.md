# The Window switcher's popup delay and the Global menu's grace period join `deadlines`

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
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

- [ ] Headless tests through the fixture, written before the code and seen to fail: the popup shows after `advance_clock(POPUP_DELAY)` and not before; with the applet gone from the bus, the Global menu's offer ends after `advance_clock` past the grace period and not before, and an applet that comes back within it keeps the offer.
- [ ] `tests/switcher.rs` has no `dispatch_for`.
- [ ] `switcher/mod.rs` and `dbus/global_menu.rs` have no `Timer::from_duration`.
- [ ] `tests::switcher` and `tests::global_menu` pass, and `Switch`'s unit tests pass.
- [ ] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section: where each deadline is stored, and whether `Switch`'s unit tests need a case for its new deadline.
