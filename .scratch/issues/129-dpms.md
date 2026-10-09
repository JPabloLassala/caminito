# Powering the outputs off when idle

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, and the tty run of the checklist item is 136's
Milestone: M7
Blocked by: 128
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

After the idle time of mate-power-manager's keys, the outputs are powered off, and any activity powers them on ([09](09-gsettings-mapping.md), [17](17-milestone-slicing.md)'s exit: idle blanks, then powers the outputs off).

- **The keys:** `org.mate.power-manager sleep-display-ac` and `sleep-display-battery` (seconds, 0 for never), picked by UPower's `OnBattery`. `sleep-display-ups` is never read.
- **On the tty** the outputs' DPMS is set through DRM. Nested, powering off is the blank of [128](128-idle-and-blanking.md) and nothing more.
- **Powering on** restores each output's mode and its frame, with no layout change: clients, the panel and msd see no output go or come.
- **While locked** the timers run without inhibitors ([12](12-screen-locking.md)).

## Acceptance criteria

- [x] Tests with a driven clock, written before the code: the outputs are off at the key's time, the key is chosen by `OnBattery` from a stand-in UPower, 0 never powers off, and an inhibitor holds it off.
- [x] No client sees an output removed or added across a power-off and a power-on.
- [ ] The checklist has an item, tagged M7, and it was run on the tty with the developer's monitors: both go to standby, a key lights both in their saved layout, and a playing video keeps them on. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **mate-power-manager's own DPMS** goes to XWayland and does nothing (09). If it gets in the way under Caminito, say so here: `mate-power-manager-caminito` exists since [107](107-brightness-and-power-keys.md).
- **[87](87-hot-plug-blanks-the-other-monitor.md)'s AOC** goes black when the other monitor is turned off or on. Whether a power-on does the same is seen in the check.

## Done (2026-10-09)

- **The timer:** the power-off is another of `IdleState`'s timers, `PowerOff`, due `sleep-display` after the activity the blank counts from, and held off by the same inhibitors, which don't apply while locked (12). mate-power-manager counts its own from its idle time and cancels it for an inhibitor (`gpm-idle.c:186-230` at 1.28.1), so the two agree to within its dim time. It doesn't ask `idle-activation-enabled`, which is mate-screensaver's key, and `SetActive(true)` blanks without powering off. With MATE's defaults the blank comes at 5 minutes and the power-off at 30. Activity ends both. An inhibitor that comes while the outputs are off leaves them off, as it leaves the blank.
- **The keys:** `org.mate.power-manager` is read whole and isn't required. `sleep-display-ac` and `sleep-display-battery` are in seconds, 0 is never, and a negative one is its default and a problem. `sleep-display-ups` is never read. The schema is vendored into `tests/schemas/` from mate-power-manager 1.28.1.
- **UPower:** `dbus::upower` follows `OnBattery` on the system bus. It asks when UPower's name gets an owner and at each `PropertiesChanged` of its interface, with `NoAutoStart`, as `session_manager` asks mate-session. Without UPower, or without a system bus, the machine counts as on AC. It runs nested too: which key applies is the machine's. A change applies at once: plugged in past AC's time, the outputs go off then.
- **On the tty** each lit monitor's CRTC is turned off with `DrmCompositor::clear`, as niri's `deactivate_monitors` does. No frame is drawn until activity, and the next frame sets the mode again from the state Smithay keeps. Outputs aren't disabled, so no output goes or comes and policy moves nothing. Switched away, only the flag is set, and switching back turns the CRTCs off again after `activate`. A monitor lit while the others are off (plugged in, or enabled through output management) is turned off after it's lit: Smithay's `initialize_output` commits a frame first, so it flashes black once. That wasn't worth keeping the monitor unlit for, since an unlit monitor that fails to light loses its output.
- **Nested,** powering off is the blank: the scene is the blank alone while the outputs are blanked or powered off.
- **The lock while off:** no frame is drawn, and none shows the session, so the redraw pass counts a powered-off output as drawn for the lock. A lock taken then is confirmed to its locker, and logind's sleep isn't held. The review found this. The headless backend had kept drawing while off, which hid it. It now draws nothing then either, and `a_lock_taken_while_powered_off_is_confirmed` covers it.
- **Test first:** the settings tests and `IdleState`'s unit tests were written first, and they failed to build before the code. The headless tests in `tests::dpms` were written before the code that applies the effects. Each was then checked by taking out the line it covers: the backend's power-off, the power-on, `OnBattery`'s choice, the inhibitor's hold, and the scene's blank for a powered-off output each failed their tests. `a_change_of_on_battery_applies_at_once` passed without `OnBattery` at first, so it was rewritten so that the two keys give different answers.
- **No client sees an output go or come:** `no_client_sees_an_output_go_or_come`, with two monitors. A client's `wl_output` globals, output management's serial and heads, a window's enter and leave events, each monitor's mode sets and the dump's outputs are all the same after a power-off and a power-on.
- **128's tests** set `sleep-display-ac` to 0: one of them waits an hour, which is past AC's default 30 minutes.
- **The debug dump's `idle`** has `powered_off` and `on_battery`.
- **mate-power-manager's own DPMS:** it's on XWayland, where it does nothing (09). Whether it gets in the way under Caminito wasn't seen, as nothing was run in a Session. 136's run is where to look.
- **Not done:** a mode change while the outputs are off is taken, and set when they're powered on. The monitors are powered on one `queue_frame` each, not in one commit, so whether the AOC blacks out as the LG comes back (87) is for the checklist run.
- **The full suite** passed but for 236 tests that fail on `master` too in this container: its Ubuntu Xwayland, xkeyboard-config and session tooling aren't CI's Arch ones. `caminito-theme`'s goldens differ by a few anti-aliased pixels for the same reason. Clippy 1.97 flags the three `manual_range_contains` in `caminito-theme` that `master` has too; the rest of the workspace is clean. Two stray `.snap.new` files that 128 committed were removed.
- **The checklist item** is *Idle powers both monitors off, a key lights them in their layout, and a video keeps them on*, tagged M7. It wasn't run: the ticket was done in a container with no display, so it's left to [136](136-packaging-and-closing-m7.md).
