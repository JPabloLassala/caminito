//! The AccessX filter in the keyboard path (15): the keys it lets through go on to stage 3,
//! its beeps ring the bell event (105), a feature it turned on or off is written to its key
//! and told in a notification, and a timer wakes it when it asked to be.

use std::time::Duration;

use smithay::backend::input::{InputTime, KeyState, Keycode};
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use super::{Cause, Effect, Feature, Key, Role, Settings};
use crate::bell::{BellFor, BellSource};
use crate::settings::Value;
use crate::settings::translate::A11Y_KEYBOARD;
use crate::state::{Caminito, State};

/// The timer for the filter's deadline.
pub(crate) struct Wake {
    /// The deadline it's for.
    due: Duration,
    timer: RegistrationToken,
}

impl State {
    /// Stage 2 of the keyboard path: a device's key event at `time`, through the filter.
    pub(in crate::input) fn accessx_stage(
        &mut self,
        keycode: Keycode,
        key_state: KeyState,
        time: InputTime,
    ) {
        let key = Key {
            keycode,
            role: self.key_role(keycode),
            pressed: key_state == KeyState::Pressed,
            time: time.millis(),
        };
        let now = self.caminito.now();
        let effects = self.caminito.accessx.key(key, now);
        self.accessx_effects(effects);
    }

    /// New AccessX settings, for the filter.
    pub(crate) fn accessx_settings_changed(&mut self, settings: Settings) {
        let now = self.caminito.now();
        let effects = self.caminito.accessx.settings_changed(settings, now);
        self.accessx_effects(effects);
    }

    /// What `keycode` is to the filter: by the keysym it has unshifted in the active layout,
    /// so a keymap that moves Ctrl to CapsLock moves what sticky keys hold too.
    fn key_role(&mut self, keycode: Keycode) -> Role {
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        keyboard.with_xkb_state(self, |context| {
            let xkb = context.xkb().lock().unwrap();
            let keysyms = xkb.raw_syms_for_key_in_layout(keycode, xkb.active_layout());
            keysyms
                .first()
                .map_or(Role::Other, |keysym| Role::of(*keysym))
        })
    }

    /// Does what the filter asked, and sets the timer for what it's waiting on.
    fn accessx_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Key {
                    keycode,
                    pressed,
                    time,
                } => {
                    let key_state = if pressed {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    };
                    self.key_past_accessx(keycode, key_state, InputTime::from_millis(time));
                }
                Effect::Beep(beep) => {
                    tracing::debug!("AccessX beeps: {beep:?}");
                    self.caminito.ring_bell(BellSource::AccessX, BellFor::Focus);
                }
                Effect::Turn { feature, on, by } => self.caminito.accessx_turned(feature, on, by),
            }
        }
        self.wake_accessx();
    }

    /// Sets the timer for the filter's deadline, if it isn't set for it already.
    fn wake_accessx(&mut self) {
        let caminito = &mut self.caminito;
        let deadline = caminito.accessx.deadline();
        if caminito.accessx_wake.as_ref().map(|wake| wake.due) == deadline {
            return;
        }
        if let Some(wake) = caminito.accessx_wake.take() {
            caminito.loop_handle.remove(wake.timer);
        }
        let Some(due) = deadline else {
            return;
        };
        let timer = Timer::from_duration(due.saturating_sub(caminito.now()));
        let timer = caminito.loop_handle.insert_source(timer, |_, _, state| {
            // This timer is done, and the filter's next deadline gets another.
            state.caminito.accessx_wake = None;
            let now = state.caminito.now();
            let effects = state.caminito.accessx.elapse(now);
            state.accessx_effects(effects);
            TimeoutAction::Drop
        });
        match timer {
            Ok(timer) => caminito.accessx_wake = Some(Wake { due, timer }),
            Err(err) => tracing::warn!("can't set AccessX's timer: {}", err.error),
        }
    }
}

impl Caminito {
    /// The filter turned `feature` on or off: its key follows, so the Keyboard dialog shows
    /// it, and a notification says so, where msd asks first under X11 (13).
    fn accessx_turned(&mut self, feature: Feature, on: bool, by: Cause) {
        let (key, name) = match feature {
            Feature::StickyKeys => ("stickykeys-enable", "Sticky Keys"),
            Feature::SlowKeys => ("slowkeys-enable", "Slow Keys"),
        };
        self.settings_thread
            .set(A11Y_KEYBOARD, key, Value::Bool(on));
        let summary = format!("{name} turned {}", if on { "on" } else { "off" });
        let body = match by {
            Cause::ShiftFiveTimes => "Shift was pressed five times in a row.",
            Cause::ShiftHeld => "Shift was held down for eight seconds.",
            Cause::TwoKeys => "Two keys were pressed at once.",
        };
        self.notify(&summary, body);
    }
}
