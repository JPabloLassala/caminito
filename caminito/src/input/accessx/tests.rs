use super::*;

// xkb key codes: evdev's plus 8.
const CTRL: u32 = 37;
const A: u32 = 38;
const SHIFT: u32 = 50;
const B: u32 = 56;
const RIGHT_SHIFT: u32 = 62;
const CAPS: u32 = 66;

fn role(keycode: u32) -> Role {
    match keycode {
        CTRL => Role::Modifier(Modifier::Control),
        SHIFT | RIGHT_SHIFT => Role::Modifier(Modifier::Shift),
        CAPS => Role::Lock,
        _ => Role::Other,
    }
}

/// Every feature off, with the schema's other defaults.
fn off() -> Settings {
    Settings {
        gestures: false,
        sticky_keys: false,
        sticky_latch_to_lock: true,
        sticky_two_key_off: false,
        sticky_modifier_beep: false,
        slow_keys: false,
        slow_delay: Duration::from_millis(300),
        slow_beep_press: false,
        slow_beep_accept: false,
        slow_beep_reject: false,
        bounce_keys: false,
        bounce_delay: Duration::from_millis(300),
        bounce_beep_reject: false,
        toggle_keys: false,
    }
}

fn sticky() -> Settings {
    Settings {
        sticky_keys: true,
        ..off()
    }
}

fn slow() -> Settings {
    Settings {
        slow_keys: true,
        ..off()
    }
}

fn bounce() -> Settings {
    Settings {
        bounce_keys: true,
        ..off()
    }
}

fn gestures() -> Settings {
    Settings {
        gestures: true,
        ..off()
    }
}

/// A filter and the clock that drives it.
struct Driven {
    filter: Filter,
    now: Duration,
    /// What the filter asked for since `take`, each with the clock's milliseconds.
    log: Vec<String>,
}

impl Driven {
    fn new(settings: Settings) -> Self {
        Self {
            filter: Filter::new(settings),
            now: Duration::ZERO,
            log: Vec::new(),
        }
    }

    fn record(&mut self, effects: Vec<Effect>) {
        let now = self.now.as_millis();
        for effect in effects {
            self.log.push(match effect {
                Effect::Key {
                    keycode,
                    pressed: true,
                    ..
                } => format!("{now}: press {}", keycode.raw()),
                Effect::Key { keycode, .. } => format!("{now}: release {}", keycode.raw()),
                Effect::Beep(beep) => format!("{now}: beep {beep:?}"),
                Effect::Turn { feature, on, by } => {
                    format!(
                        "{now}: {feature:?} {} by {by:?}",
                        if on { "on" } else { "off" }
                    )
                }
            });
        }
    }

    /// Moves the clock to `millis`, by way of each time the filter asked to be woken at.
    fn at(&mut self, millis: u64) -> &mut Self {
        let then = Duration::from_millis(millis);
        while let Some(deadline) = self.filter.deadline().filter(|deadline| *deadline <= then) {
            self.now = self.now.max(deadline);
            let effects = self.filter.elapse(self.now);
            self.record(effects);
        }
        self.now = then;
        self
    }

    fn key(&mut self, keycode: u32, pressed: bool) -> &mut Self {
        let key = Key {
            keycode: Keycode::new(keycode),
            role: role(keycode),
            pressed,
            time: self.now.as_millis() as u32,
        };
        let effects = self.filter.key(key, self.now);
        self.record(effects);
        self
    }

    fn press(&mut self, keycode: u32) -> &mut Self {
        self.key(keycode, true)
    }

    fn release(&mut self, keycode: u32) -> &mut Self {
        self.key(keycode, false)
    }

    /// A press and a release at the clock's time.
    fn tap(&mut self, keycode: u32) -> &mut Self {
        self.press(keycode).release(keycode)
    }

    fn settings(&mut self, settings: Settings) -> &mut Self {
        let effects = self.filter.settings_changed(settings, self.now);
        self.record(effects);
        self
    }

    fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }
}

const NOTHING: [&str; 0] = [];

#[test]
fn with_everything_off_every_key_passes_as_it_is() {
    let mut d = Driven::new(off());
    d.tap(SHIFT).press(SHIFT).tap(A).release(SHIFT).tap(CAPS);
    assert_eq!(
        d.take(),
        [
            "0: press 50",
            "0: release 50",
            "0: press 50",
            "0: press 38",
            "0: release 38",
            "0: release 50",
            "0: press 66",
            "0: release 66",
        ]
    );
    assert_eq!(d.filter.deadline(), None);
}

#[test]
fn a_modifier_tapped_alone_latches_for_the_next_key_only() {
    let mut d = Driven::new(sticky());
    d.tap(SHIFT);
    // The modifier stays down past its release.
    assert_eq!(d.take(), ["0: press 50"]);

    d.at(10).press(A).at(20).release(A);
    assert_eq!(
        d.take(),
        ["10: press 38", "10: release 50", "20: release 38"]
    );

    d.at(30).tap(B);
    assert_eq!(d.take(), ["30: press 56", "30: release 56"]);
}

#[test]
fn a_modifier_held_while_another_key_is_pressed_doesnt_latch() {
    let mut d = Driven::new(sticky());
    d.press(SHIFT).tap(A).release(SHIFT);
    assert_eq!(
        d.take(),
        [
            "0: press 50",
            "0: press 38",
            "0: release 38",
            "0: release 50"
        ]
    );
}

#[test]
fn two_latched_modifiers_both_go_with_the_next_key() {
    let mut d = Driven::new(sticky());
    d.tap(SHIFT).tap(CTRL).press(A);
    assert_eq!(
        d.take(),
        [
            "0: press 50",
            "0: press 37",
            "0: press 38",
            "0: release 50",
            "0: release 37"
        ]
    );
}

#[test]
fn a_lock_key_doesnt_take_the_latch() {
    let mut d = Driven::new(sticky());
    d.tap(SHIFT).tap(CAPS).press(A);
    assert_eq!(
        d.take(),
        [
            "0: press 50",
            "0: press 66",
            "0: release 66",
            "0: press 38",
            "0: release 50"
        ]
    );
}

#[test]
fn a_second_press_locks_a_latched_modifier_with_latch_to_lock_and_a_third_unlocks_it() {
    let mut d = Driven::new(sticky());
    d.tap(SHIFT).tap(SHIFT);
    assert_eq!(d.take(), ["0: press 50"]);

    // Locked: it outlasts the keys pressed with it.
    d.tap(A).tap(B);
    assert_eq!(
        d.take(),
        [
            "0: press 38",
            "0: release 38",
            "0: press 56",
            "0: release 56"
        ]
    );

    d.tap(SHIFT);
    assert_eq!(d.take(), ["0: release 50"]);
    d.tap(A);
    assert_eq!(d.take(), ["0: press 38", "0: release 38"]);
}

#[test]
fn the_modifiers_other_key_locks_it_too() {
    let mut d = Driven::new(sticky());
    d.tap(SHIFT).tap(RIGHT_SHIFT).tap(A);
    assert_eq!(d.take(), ["0: press 50", "0: press 38", "0: release 38"]);
}

#[test]
fn a_second_press_unlatches_a_modifier_without_latch_to_lock() {
    let mut d = Driven::new(Settings {
        sticky_latch_to_lock: false,
        ..sticky()
    });
    d.tap(SHIFT).tap(SHIFT);
    assert_eq!(d.take(), ["0: press 50", "0: release 50"]);
    d.tap(A);
    assert_eq!(d.take(), ["0: press 38", "0: release 38"]);
}

#[test]
fn two_keys_at_once_turn_sticky_keys_off_with_two_key_off() {
    let mut d = Driven::new(Settings {
        sticky_two_key_off: true,
        ..sticky()
    });
    d.tap(CTRL).press(SHIFT).press(A);
    assert_eq!(
        d.take(),
        [
            "0: press 37",
            "0: press 50",
            "0: StickyKeys off by TwoKeys",
            // What was latched goes with the press, and is let go. Shift is still held.
            "0: press 38",
            "0: release 37",
        ]
    );

    d.release(A).release(SHIFT).tap(SHIFT);
    assert_eq!(
        d.take(),
        [
            "0: release 38",
            "0: release 50",
            "0: press 50",
            "0: release 50"
        ]
    );
}

#[test]
fn two_keys_at_once_leave_sticky_keys_on_without_two_key_off() {
    let mut d = Driven::new(sticky());
    d.press(SHIFT).tap(A).release(SHIFT).tap(SHIFT);
    d.take();
    d.press(A);
    assert_eq!(d.take(), ["0: press 38", "0: release 50"]);
}

#[test]
fn turning_sticky_keys_off_lets_go_of_what_was_latched_and_locked() {
    let mut d = Driven::new(sticky());
    d.tap(CTRL).tap(SHIFT).tap(SHIFT);
    d.take();
    d.at(40).settings(off());
    assert_eq!(d.take(), ["40: release 37", "40: release 50"]);
    d.tap(SHIFT);
    assert_eq!(d.take(), ["40: press 50", "40: release 50"]);
}

#[test]
fn a_key_held_less_than_the_slow_keys_delay_is_dropped() {
    let mut d = Driven::new(slow());
    d.press(A).at(299).release(A).at(1000);
    assert_eq!(d.take(), NOTHING);
    assert_eq!(d.filter.deadline(), None);
}

#[test]
fn a_key_held_longer_than_the_slow_keys_delay_is_delivered_once_at_the_delay() {
    let mut d = Driven::new(slow());
    d.at(100).press(A);
    assert_eq!(d.filter.deadline(), Some(Duration::from_millis(400)));
    d.at(399);
    assert_eq!(d.take(), NOTHING);

    d.at(900);
    assert_eq!(d.take(), ["400: press 38"]);
    d.release(A);
    assert_eq!(d.take(), ["900: release 38"]);
}

#[test]
fn a_slow_keys_press_is_timed_at_the_delay_after_the_keys_own() {
    let mut filter = Filter::new(slow());
    let key = Key {
        keycode: Keycode::new(A),
        role: Role::Other,
        pressed: true,
        time: 5000,
    };
    assert_eq!(filter.key(key, Duration::from_millis(100)), []);
    assert_eq!(
        filter.elapse(Duration::from_millis(400)),
        [Effect::Key {
            keycode: Keycode::new(A),
            pressed: true,
            time: 5300
        }]
    );
}

#[test]
fn a_key_let_go_after_the_delay_is_delivered_though_the_timer_is_late() {
    let mut filter = Filter::new(slow());
    let key = |pressed| Key {
        keycode: Keycode::new(A),
        role: Role::Other,
        pressed,
        time: 0,
    };
    filter.key(key(true), Duration::ZERO);
    let effects = filter.key(key(false), Duration::from_millis(400));
    assert_eq!(
        effects
            .iter()
            .map(|effect| matches!(effect, Effect::Key { pressed: true, .. }))
            .collect::<Vec<_>>(),
        [true, false]
    );
}

#[test]
fn settings_read_before_a_gestures_key_was_written_dont_undo_it() {
    let mut d = Driven::new(gestures());
    d.tap(SHIFT)
        .tap(SHIFT)
        .tap(SHIFT)
        .tap(SHIFT)
        .tap(SHIFT)
        .tap(CTRL);
    d.take();

    // Another key changed: sticky keys are still off in what was read.
    d.settings(Settings {
        toggle_keys: true,
        ..gestures()
    });
    assert_eq!(d.take(), NOTHING);
    // Their key follows, and then it's the key's to turn off.
    let followed = Settings {
        sticky_keys: true,
        toggle_keys: true,
        ..gestures()
    };
    d.settings(followed).settings(Settings {
        sticky_keys: false,
        ..followed
    });
    assert_eq!(d.take(), ["0: release 37"]);
}

#[test]
fn a_modifier_whose_role_changed_while_it_was_down_still_latches() {
    let mut filter = Filter::new(sticky());
    let key = |role, pressed| Key {
        keycode: Keycode::new(SHIFT),
        role,
        pressed,
        time: 0,
    };
    filter.key(key(Role::Modifier(Modifier::Shift), true), Duration::ZERO);
    assert_eq!(filter.key(key(Role::Other, false), Duration::ZERO), []);
}

#[test]
fn slow_keys_hold_each_key_for_its_own_delay() {
    let mut d = Driven::new(slow());
    d.press(A).at(100).press(B).at(350).release(B).at(500);
    assert_eq!(d.take(), ["300: press 38"]);
}

#[test]
fn a_modifier_latches_through_slow_keys_once_its_accepted() {
    let mut d = Driven::new(Settings {
        slow_keys: true,
        ..sticky()
    });
    d.press(SHIFT).at(400).release(SHIFT).press(A).at(800);
    assert_eq!(
        d.take(),
        ["300: press 50", "700: press 38", "700: release 50"]
    );
}

#[test]
fn turning_slow_keys_off_drops_the_keys_still_waiting() {
    let mut d = Driven::new(slow());
    d.press(A).at(100).settings(off()).at(500).release(A);
    assert_eq!(d.take(), NOTHING);
    d.tap(A);
    assert_eq!(d.take(), ["500: press 38", "500: release 38"]);
}

#[test]
fn a_second_press_of_the_same_key_within_the_bounce_keys_delay_is_dropped() {
    let mut d = Driven::new(bounce());
    d.tap(A).at(299).tap(A);
    assert_eq!(d.take(), ["0: press 38", "0: release 38"]);
}

#[test]
fn a_press_of_another_key_within_the_bounce_keys_delay_isnt_dropped() {
    let mut d = Driven::new(bounce());
    d.tap(A).at(100).tap(B);
    assert_eq!(
        d.take(),
        [
            "0: press 38",
            "0: release 38",
            "100: press 56",
            "100: release 56"
        ]
    );
}

#[test]
fn a_second_press_of_the_same_key_after_the_bounce_keys_delay_isnt_dropped() {
    let mut d = Driven::new(bounce());
    d.press(A).at(50).release(A).at(350).tap(A);
    assert_eq!(
        d.take(),
        [
            "0: press 38",
            "50: release 38",
            "350: press 38",
            "350: release 38"
        ]
    );
}

#[test]
fn a_key_that_goes_on_bouncing_goes_on_being_dropped() {
    let mut d = Driven::new(bounce());
    // The delay runs from the last release, a dropped press's too.
    d.tap(A).at(200).tap(A).at(400).tap(A);
    assert_eq!(d.take(), ["0: press 38", "0: release 38"]);
}

#[test]
fn a_modifiers_press_beeps_with_modifier_beep() {
    let mut d = Driven::new(Settings {
        sticky_modifier_beep: true,
        ..sticky()
    });
    // Latching, locking and unlocking each beep, and no other key does.
    d.tap(SHIFT).tap(SHIFT).tap(SHIFT).tap(A);
    assert_eq!(
        d.take(),
        [
            "0: press 50",
            "0: beep StickyModifier",
            "0: beep StickyModifier",
            "0: release 50",
            "0: beep StickyModifier",
            "0: press 38",
            "0: release 38",
        ]
    );
}

#[test]
fn slow_keys_beep_on_press_accept_and_reject_as_each_key_says() {
    let all = Settings {
        slow_beep_press: true,
        slow_beep_accept: true,
        slow_beep_reject: true,
        ..slow()
    };
    let mut d = Driven::new(all);
    d.press(A).at(100).release(A).at(200).press(B).at(600);
    assert_eq!(
        d.take(),
        [
            "0: beep SlowPress",
            "100: beep SlowReject",
            "200: beep SlowPress",
            "500: beep SlowAccept",
            "500: press 56",
        ]
    );

    let run = |settings: Settings| {
        let mut d = Driven::new(settings);
        d.press(A).at(100).release(A).at(200).press(B).at(600);
        d.take()
    };
    assert_eq!(
        run(Settings {
            slow_beep_press: false,
            ..all
        }),
        [
            "100: beep SlowReject",
            "500: beep SlowAccept",
            "500: press 56"
        ]
    );
    assert_eq!(
        run(Settings {
            slow_beep_accept: false,
            ..all
        }),
        [
            "0: beep SlowPress",
            "100: beep SlowReject",
            "200: beep SlowPress",
            "500: press 56"
        ]
    );
    assert_eq!(
        run(Settings {
            slow_beep_reject: false,
            ..all
        }),
        [
            "0: beep SlowPress",
            "200: beep SlowPress",
            "500: beep SlowAccept",
            "500: press 56"
        ]
    );
}

#[test]
fn a_press_bounce_keys_drop_beeps_with_beep_reject() {
    let mut d = Driven::new(Settings {
        bounce_beep_reject: true,
        ..bounce()
    });
    d.tap(A).at(100).tap(A);
    assert_eq!(
        d.take(),
        ["0: press 38", "0: release 38", "100: beep BounceReject"]
    );
}

#[test]
fn a_lock_keys_press_beeps_with_toggle_keys() {
    let mut d = Driven::new(Settings {
        toggle_keys: true,
        ..off()
    });
    d.tap(CAPS).tap(A).tap(SHIFT);
    assert_eq!(
        d.take(),
        [
            "0: press 66",
            "0: beep ToggleKey",
            "0: release 66",
            "0: press 38",
            "0: release 38",
            "0: press 50",
            "0: release 50",
        ]
    );
}

/// The effects that aren't keys.
fn turns(log: Vec<String>) -> Vec<String> {
    let key = |line: &String| line.contains(": press ") || line.contains(": release ");
    log.into_iter().filter(|line| !key(line)).collect()
}

#[test]
fn shift_pressed_five_times_turns_sticky_keys_on_and_five_more_off() {
    let mut d = Driven::new(gestures());
    for n in 0..4 {
        d.at(n * 100).tap(SHIFT);
    }
    assert_eq!(turns(d.take()), NOTHING);
    d.at(400).tap(SHIFT);
    assert_eq!(turns(d.take()), ["400: StickyKeys on by ShiftFiveTimes"]);

    // On from then on.
    d.at(500).tap(CTRL).tap(A);
    assert_eq!(
        d.take(),
        [
            "500: press 37",
            "500: press 38",
            "500: release 37",
            "500: release 38"
        ]
    );

    for n in 0..5 {
        d.at(1000 + n * 100).tap(RIGHT_SHIFT);
    }
    let log = d.take();
    // Shift latched, locked, unlocked, latched and locked, and is let go at the end.
    assert_eq!(
        log,
        [
            "1000: press 62",
            "1200: release 62",
            "1300: press 62",
            "1400: StickyKeys off by ShiftFiveTimes",
            "1400: release 62",
        ]
    );
    d.at(2000).tap(CTRL);
    assert_eq!(d.take(), ["2000: press 37", "2000: release 37"]);
}

#[test]
fn another_key_between_the_presses_starts_shifts_count_again() {
    let mut d = Driven::new(gestures());
    d.tap(SHIFT).tap(SHIFT).tap(SHIFT).tap(SHIFT);
    d.press(SHIFT).tap(A).release(SHIFT);
    d.tap(SHIFT).tap(SHIFT).tap(SHIFT).tap(SHIFT);
    assert_eq!(turns(d.take()), NOTHING);
    d.tap(SHIFT);
    assert_eq!(turns(d.take()), ["0: StickyKeys on by ShiftFiveTimes"]);
}

#[test]
fn presses_of_shift_too_far_apart_start_its_count_again() {
    let mut d = Driven::new(gestures());
    d.tap(SHIFT).tap(SHIFT).tap(SHIFT).tap(SHIFT);
    d.at(15_001).tap(SHIFT);
    assert_eq!(turns(d.take()), NOTHING);
}

#[test]
fn shift_held_for_eight_seconds_turns_slow_keys_on_and_again_off() {
    let mut d = Driven::new(gestures());
    d.press(SHIFT).at(7999);
    assert_eq!(d.take(), ["0: press 50"]);
    d.at(9000).release(SHIFT);
    assert_eq!(
        d.take(),
        ["8000: SlowKeys on by ShiftHeld", "9000: release 50"]
    );

    // On from then on: Shift itself now waits out the delay.
    d.at(10_000).press(SHIFT).at(19_000).release(SHIFT);
    assert_eq!(
        d.take(),
        [
            "10300: press 50",
            "18000: SlowKeys off by ShiftHeld",
            "19000: release 50"
        ]
    );
    d.tap(A);
    assert_eq!(d.take(), ["19000: press 38", "19000: release 38"]);
}

#[test]
fn shift_let_go_or_another_key_pressed_before_eight_seconds_toggles_nothing() {
    let mut d = Driven::new(gestures());
    d.press(SHIFT).at(7000).release(SHIFT).at(9000);
    d.press(SHIFT).at(10_000).tap(A).at(20_000).release(SHIFT);
    assert_eq!(turns(d.take()), NOTHING);
    assert_eq!(d.filter.deadline(), None);
}

#[test]
fn the_gestures_toggle_nothing_without_enable() {
    let mut d = Driven::new(off());
    d.tap(SHIFT).tap(SHIFT).tap(SHIFT).tap(SHIFT).tap(SHIFT);
    d.press(SHIFT).at(9000).release(SHIFT);
    assert_eq!(turns(d.take()), NOTHING);
    assert_eq!(d.filter.deadline(), None);
}

#[test]
fn clearing_forgets_the_keys_held_and_waiting() {
    let mut d = Driven::new(Settings {
        slow_keys: true,
        ..sticky()
    });
    d.press(SHIFT).at(400).release(SHIFT).press(A);
    d.take();
    d.filter.clear();
    assert_eq!(d.filter.deadline(), None);
    d.at(2000).press(B).at(3000);
    assert_eq!(d.take(), ["2300: press 56"]);
}

#[test]
fn a_keys_role_is_its_keysyms() {
    use smithay::input::keyboard::Keysym;

    assert_eq!(Role::of(Keysym::Shift_R), Role::Modifier(Modifier::Shift));
    assert_eq!(
        Role::of(Keysym::Control_L),
        Role::Modifier(Modifier::Control)
    );
    assert_eq!(Role::of(Keysym::Alt_L), Role::Modifier(Modifier::Alt));
    assert_eq!(Role::of(Keysym::Super_L), Role::Modifier(Modifier::Super));
    assert_eq!(
        Role::of(Keysym::ISO_Level3_Shift),
        Role::Modifier(Modifier::Level3)
    );
    assert_eq!(Role::of(Keysym::Caps_Lock), Role::Lock);
    assert_eq!(Role::of(Keysym::Num_Lock), Role::Lock);
    assert_eq!(Role::of(Keysym::a), Role::Other);
}
