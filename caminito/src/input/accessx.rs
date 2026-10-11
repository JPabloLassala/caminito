//! AccessX (13, 134): sticky, slow and bounce keys, the toggle keys' beep and the two
//! gestures, as a pure filter over key events and time. The X server does this inside XKB
//! under X11, where msd only hands it `org.mate.accessibility-keyboard`; nothing does under
//! Caminito but this.
//!
//! The filter is stage 2 of the keyboard path, before xkbcommon, so it knows keys by their
//! codes, and is told each one's [`Role`]. It reads no clock: every call takes `now`, and
//! [`Filter::deadline`] says when it next needs [`Filter::elapse`].
//!
//! A key goes through bounce keys, then slow keys, then sticky keys, as in XKB. The gestures
//! watch the keys as they come, before any of them.
//!
//! Sticky keys never touch xkb's state. A latched modifier is one whose key the filter keeps
//! down past its release, until the next key's press has gone out, and a locked one is kept
//! down until its next press. So the bindings and the client see the modifier held, as they
//! would with the key itself, and xkbcommon's own sticky keys can replace this stage when
//! Smithay exposes them (13).

mod apply;

use std::collections::HashSet;
use std::time::Duration;

use smithay::input::keyboard::{Keycode, Keysym};

use crate::settings::translate::AccessX as Settings;

/// How many presses of Shift in a row toggle sticky keys.
const SHIFT_PRESSES: u32 = 5;
/// The longest time between two of those presses.
const SHIFT_PRESSES_APART: Duration = Duration::from_secs(15);
/// How long Shift is held to toggle slow keys.
const SHIFT_HOLD: Duration = Duration::from_secs(8);

/// A modifier sticky keys latch and lock, whichever of its keys is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
    Shift,
    Control,
    Alt,
    Super,
    /// AltGr.
    Level3,
    Level5,
}

/// What a key is to the filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// A modifier held while other keys are pressed, which sticky keys hold instead.
    Modifier(Modifier),
    /// CapsLock, NumLock or ScrollLock, which the toggle keys beep for.
    Lock,
    Other,
}

impl Role {
    /// The role of the key that `keysym` is on, unshifted.
    fn of(keysym: Keysym) -> Self {
        match keysym {
            Keysym::Shift_L | Keysym::Shift_R => Role::Modifier(Modifier::Shift),
            Keysym::Control_L | Keysym::Control_R => Role::Modifier(Modifier::Control),
            Keysym::Alt_L | Keysym::Alt_R | Keysym::Meta_L | Keysym::Meta_R => {
                Role::Modifier(Modifier::Alt)
            }
            Keysym::Super_L | Keysym::Super_R | Keysym::Hyper_L | Keysym::Hyper_R => {
                Role::Modifier(Modifier::Super)
            }
            Keysym::ISO_Level3_Shift | Keysym::Mode_switch => Role::Modifier(Modifier::Level3),
            Keysym::ISO_Level5_Shift => Role::Modifier(Modifier::Level5),
            Keysym::Caps_Lock | Keysym::Shift_Lock | Keysym::Num_Lock | Keysym::Scroll_Lock => {
                Role::Lock
            }
            _ => Role::Other,
        }
    }
}

/// One key event, as a device sent it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    keycode: Keycode,
    role: Role,
    pressed: bool,
    /// The event's own time, in milliseconds, which goes on with it.
    time: u32,
}

/// A beep the filter asks for, each by the key of the settings that turns it on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Beep {
    /// `stickykeys-modifier-beep`: a modifier was pressed.
    StickyModifier,
    /// `slowkeys-beep-press`: a key was pressed, and waits.
    SlowPress,
    /// `slowkeys-beep-accept`: a key was held for the delay.
    SlowAccept,
    /// `slowkeys-beep-reject`: a key was let go before the delay.
    SlowReject,
    /// `bouncekeys-beep-reject`: a press was dropped.
    BounceReject,
    /// `togglekeys-enable`: a lock key was pressed.
    ToggleKey,
}

/// A feature the filter turns on and off itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Feature {
    StickyKeys,
    SlowKeys,
}

/// What made the filter turn a feature on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cause {
    /// Shift was pressed five times in a row.
    ShiftFiveTimes,
    /// Shift was held for eight seconds.
    ShiftHeld,
    /// Two keys were pressed at once, with `stickykeys-two-key-off`.
    TwoKeys,
}

/// What the filter asks of the keyboard path, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    /// A key event for the next stage.
    Key {
        keycode: Keycode,
        pressed: bool,
        /// In milliseconds, as a key event's.
        time: u32,
    },
    Beep(Beep),
    /// The filter turned `feature` on or off, which its key is to follow.
    Turn {
        feature: Feature,
        on: bool,
        by: Cause,
    },
}

/// What sticky keys know of a modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stuck {
    /// Its key is down. It latches at the release unless another key was `used` with it.
    Down { used: bool },
    /// Kept down for the next key.
    Latched,
    /// Kept down until its next press.
    Locked,
}

/// A modifier sticky keys are holding, with the key that's down for it in the next stage.
#[derive(Debug, Clone, Copy)]
struct Held {
    modifier: Modifier,
    keycode: Keycode,
    state: Stuck,
}

/// A press slow keys are holding back.
#[derive(Debug, Clone, Copy)]
struct Waiting {
    /// The press as it goes on, timed at the delay.
    key: Key,
    due: Duration,
}

/// The filter's state.
#[derive(Debug)]
pub(crate) struct Filter {
    settings: Settings,
    /// Sticky keys' modifiers, in the order they were pressed.
    held: Vec<Held>,
    /// Slow keys' presses.
    waiting: Vec<Waiting>,
    /// The last key released, and when: the one bounce keys drop a press of.
    released: Option<(Keycode, Duration)>,
    /// The keys whose press didn't go on, whose release doesn't either.
    swallowed: HashSet<Keycode>,
    /// Shift's presses in a row, and the time of the last.
    shift_presses: u32,
    last_shift_press: Duration,
    /// When a Shift held since its press toggles slow keys.
    shift_hold: Option<Duration>,
    /// Whether the filter turned sticky keys on or off itself, and its key hasn't followed.
    sticky_ahead: bool,
    /// The same, for slow keys.
    slow_ahead: bool,
}

/// `now` as a key event's time.
fn millis(now: Duration) -> u32 {
    // Truncated, as the events' own times wrap.
    now.as_millis() as u32
}

impl Filter {
    pub(crate) fn new(settings: Settings) -> Self {
        Self {
            settings,
            held: Vec::new(),
            waiting: Vec::new(),
            released: None,
            swallowed: HashSet::new(),
            shift_presses: 0,
            last_shift_press: Duration::ZERO,
            shift_hold: None,
            sticky_ahead: false,
            slow_ahead: false,
        }
    }

    /// One key event from a device, at `now`.
    fn key(&mut self, key: Key, now: Duration) -> Vec<Effect> {
        // First what was due before it, should the timer be late: a key held for the delay
        // isn't one let go too soon.
        let mut effects = self.elapse(now);
        self.gestures(key, now, &mut effects);
        if self.bounces(key, now, &mut effects) {
            return effects;
        }
        if key.pressed && self.settings.slow_keys {
            let delay = self.settings.slow_delay;
            self.waiting.retain(|w| w.key.keycode != key.keycode);
            self.waiting.push(Waiting {
                key: Key {
                    time: key.time.wrapping_add(millis(delay)),
                    ..key
                },
                due: now + delay,
            });
            if self.settings.slow_beep_press {
                effects.push(Effect::Beep(Beep::SlowPress));
            }
            return effects;
        }
        let waiting = self
            .waiting
            .iter()
            .position(|w| w.key.keycode == key.keycode);
        if let (false, Some(waiting)) = (key.pressed, waiting) {
            self.waiting.remove(waiting);
            if self.settings.slow_beep_reject {
                effects.push(Effect::Beep(Beep::SlowReject));
            }
            return effects;
        }
        self.pass(key, &mut effects);
        effects
    }

    /// The time [`Filter::elapse`] is next needed at, if any.
    pub(crate) fn deadline(&self) -> Option<Duration> {
        let waiting = self.waiting.iter().map(|w| w.due);
        waiting.chain(self.shift_hold).min()
    }

    /// Time went by to `now`: the keys slow keys held for their delay go on, and a Shift held
    /// for long enough toggles slow keys.
    fn elapse(&mut self, now: Duration) -> Vec<Effect> {
        let mut effects = Vec::new();
        loop {
            let due = self
                .waiting
                .iter()
                .enumerate()
                .filter(|(_, w)| w.due <= now);
            let Some((first, _)) = due.min_by_key(|(_, w)| w.due) else {
                break;
            };
            let key = self.waiting.remove(first).key;
            if self.settings.slow_beep_accept {
                effects.push(Effect::Beep(Beep::SlowAccept));
            }
            self.pass(key, &mut effects);
        }
        if self.shift_hold.is_some_and(|due| due <= now) {
            self.shift_hold = None;
            // The release that follows isn't one of five presses.
            self.shift_presses = 0;
            let on = !self.settings.slow_keys;
            self.turn(
                Feature::SlowKeys,
                on,
                Cause::ShiftHeld,
                millis(now),
                &mut effects,
            );
        }
        effects
    }

    /// New settings, at `now`. A feature turned off lets go of what it held. A feature the
    /// filter turned on or off itself stays as the filter has it until its key says the
    /// same: settings read before the key was written would undo it.
    fn settings_changed(&mut self, mut settings: Settings, now: Duration) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.sticky_ahead &= settings.sticky_keys != self.settings.sticky_keys;
        self.slow_ahead &= settings.slow_keys != self.settings.slow_keys;
        if self.sticky_ahead {
            settings.sticky_keys = self.settings.sticky_keys;
        }
        if self.slow_ahead {
            settings.slow_keys = self.settings.slow_keys;
        }
        self.settings = settings;
        if !settings.sticky_keys {
            self.let_go(millis(now), &mut effects);
        }
        if !settings.slow_keys {
            self.stop_waiting();
        }
        if !settings.bounce_keys {
            self.released = None;
        }
        if !settings.gestures {
            self.shift_presses = 0;
            self.shift_hold = None;
        }
        effects
    }

    /// Forgets every key, when the next stage's keys were released behind the filter's back.
    pub(crate) fn clear(&mut self) {
        *self = Self {
            sticky_ahead: self.sticky_ahead,
            slow_ahead: self.slow_ahead,
            ..Self::new(self.settings)
        };
    }

    /// The two gestures, with `enable`: Shift five times in a row toggles sticky keys, and
    /// Shift held for eight seconds toggles slow keys, which `elapse` sees to.
    fn gestures(&mut self, key: Key, now: Duration, effects: &mut Vec<Effect>) {
        if !self.settings.gestures {
            return;
        }
        let shift = key.role == Role::Modifier(Modifier::Shift);
        // Shift is held alone, from its press: any other event ends that.
        self.shift_hold = (shift && key.pressed).then_some(now + SHIFT_HOLD);
        match (shift, key.pressed) {
            (true, true) => {
                let in_a_row = self.shift_presses > 0
                    && now.saturating_sub(self.last_shift_press) <= SHIFT_PRESSES_APART;
                self.shift_presses = if in_a_row { self.shift_presses + 1 } else { 1 };
                self.last_shift_press = now;
            }
            (true, false) if self.shift_presses >= SHIFT_PRESSES => {
                self.shift_presses = 0;
                let on = !self.settings.sticky_keys;
                self.turn(
                    Feature::StickyKeys,
                    on,
                    Cause::ShiftFiveTimes,
                    key.time,
                    effects,
                );
            }
            (false, true) => self.shift_presses = 0,
            (_, false) => {}
        }
    }

    /// Turns `feature` on or off itself, ahead of its key, and says so.
    fn turn(
        &mut self,
        feature: Feature,
        on: bool,
        by: Cause,
        time: u32,
        effects: &mut Vec<Effect>,
    ) {
        effects.push(Effect::Turn { feature, on, by });
        match feature {
            Feature::StickyKeys => {
                self.settings.sticky_keys = on;
                self.sticky_ahead = true;
                if !on {
                    self.let_go(time, effects);
                }
            }
            Feature::SlowKeys => {
                self.settings.slow_keys = on;
                self.slow_ahead = true;
                if !on {
                    self.stop_waiting();
                }
            }
        }
    }

    /// Bounce keys: whether `key` is a press to drop, of the key last released, within the
    /// delay of that release. A dropped press's release counts too, so a key that goes on
    /// bouncing goes on being dropped.
    fn bounces(&mut self, key: Key, now: Duration, effects: &mut Vec<Effect>) -> bool {
        if !self.settings.bounce_keys {
            return false;
        }
        if !key.pressed {
            self.released = Some((key.keycode, now));
            return false;
        }
        let bounced = self.released.is_some_and(|(keycode, at)| {
            keycode == key.keycode && now.saturating_sub(at) < self.settings.bounce_delay
        });
        if bounced {
            self.swallowed.insert(key.keycode);
            if self.settings.bounce_beep_reject {
                effects.push(Effect::Beep(Beep::BounceReject));
            }
        }
        bounced
    }

    /// Slow keys were turned off: the presses still waiting never go on, nor their releases.
    fn stop_waiting(&mut self) {
        let waiting = self.waiting.drain(..);
        self.swallowed.extend(waiting.map(|w| w.key.keycode));
    }

    /// A key that bounce and slow keys let through: sticky keys, then the next stage.
    fn pass(&mut self, key: Key, effects: &mut Vec<Effect>) {
        if !key.pressed && self.swallowed.remove(&key.keycode) {
            return;
        }
        if self.settings.sticky_keys {
            self.stick(key, effects);
        } else {
            effects.push(key.effect());
        }
        if key.pressed && key.role == Role::Lock && self.settings.toggle_keys {
            effects.push(Effect::Beep(Beep::ToggleKey));
        }
    }

    /// Sticky keys.
    fn stick(&mut self, key: Key, effects: &mut Vec<Effect>) {
        if key.pressed {
            let mut down = self.held.iter_mut().filter(|held| {
                matches!(held.state, Stuck::Down { .. }) && held.keycode != key.keycode
            });
            if self.settings.sticky_two_key_off && down.next().is_some() {
                // Someone who holds one key and presses another has no use for sticky keys.
                // What was latched still goes with this press, and is let go after it.
                effects.push(Effect::Turn {
                    feature: Feature::StickyKeys,
                    on: false,
                    by: Cause::TwoKeys,
                });
                self.settings.sticky_keys = false;
                self.sticky_ahead = true;
                effects.push(key.effect());
                self.let_go(key.time, effects);
                return;
            }
            for held in down {
                held.state = Stuck::Down { used: true };
            }
        }
        match (key.role, key.pressed) {
            (Role::Modifier(modifier), true) => {
                let held = self.held.iter().position(|held| held.modifier == modifier);
                match held.map(|held| (held, self.held[held].state)) {
                    None => {
                        effects.push(key.effect());
                        self.held.push(Held {
                            modifier,
                            keycode: key.keycode,
                            state: Stuck::Down { used: false },
                        });
                    }
                    // The modifier's other key, while the first is down, is a key like any.
                    Some((held, Stuck::Down { .. })) => {
                        self.held[held].state = Stuck::Down { used: true };
                        effects.push(key.effect());
                    }
                    Some((held, Stuck::Latched)) if self.settings.sticky_latch_to_lock => {
                        self.held[held].state = Stuck::Locked;
                        self.swallowed.insert(key.keycode);
                    }
                    Some((held, Stuck::Latched | Stuck::Locked)) => {
                        let held = self.held.remove(held);
                        effects.push(held.release(key.time));
                        self.swallowed.insert(key.keycode);
                    }
                }
                if self.settings.sticky_modifier_beep {
                    effects.push(Effect::Beep(Beep::StickyModifier));
                }
            }
            // By its key alone: a layout switched since the press can have changed its role.
            (_, false) => {
                let held = self.held.iter().position(|held| {
                    held.keycode == key.keycode && matches!(held.state, Stuck::Down { .. })
                });
                match held {
                    Some(held) if self.held[held].state == (Stuck::Down { used: false }) => {
                        self.held[held].state = Stuck::Latched;
                    }
                    Some(held) => {
                        self.held.remove(held);
                        effects.push(key.effect());
                    }
                    None => effects.push(key.effect()),
                }
            }
            (Role::Other, true) => {
                effects.push(key.effect());
                // The latch was for this press, and no later one.
                let (latched, kept) =
                    (self.held.drain(..)).partition(|held: &Held| held.state == Stuck::Latched);
                self.held = kept;
                let latched: Vec<Held> = latched;
                effects.extend(latched.iter().map(|held| held.release(key.time)));
            }
            // A lock key takes no latch: in XKB only a key that isn't a modifier's does.
            (Role::Lock, true) => effects.push(key.effect()),
        }
    }

    /// Sticky keys were turned off: the modifiers kept down are let go. One whose key is
    /// still down goes with its own release.
    fn let_go(&mut self, time: u32, effects: &mut Vec<Effect>) {
        for held in self.held.drain(..) {
            if !matches!(held.state, Stuck::Down { .. }) {
                effects.push(held.release(time));
            }
        }
    }
}

impl Key {
    /// The event for the next stage, as it is.
    fn effect(self) -> Effect {
        Effect::Key {
            keycode: self.keycode,
            pressed: self.pressed,
            time: self.time,
        }
    }
}

impl Held {
    /// The release that sticky keys held back.
    fn release(self, time: u32) -> Effect {
        Effect::Key {
            keycode: self.keycode,
            pressed: false,
            time,
        }
    }
}

#[cfg(test)]
mod tests;
