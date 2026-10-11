//! The AccessX filter in the keyboard path (15): the keys it lets through go on to stage 3,
//! its beeps ring the bell event (105), a feature it turned on or off is written to its key
//! and told in a notification, and `deadlines` wakes it when it asked to be (174).

use std::time::Duration;

use smithay::backend::input::{InputTime, KeyState, Keycode};

use super::{Cause, Effect, Feature, Key, Role, Settings};
use crate::bell::{BellFor, BellSource};
use crate::settings::Value;
use crate::settings::translate::A11Y_KEYBOARD;
use crate::state::{Caminito, State};

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

    /// Runs what the filter has due at `now`, for `deadlines`.
    pub(crate) fn accessx_elapse(&mut self, now: Duration) {
        let effects = self.caminito.accessx.elapse(now);
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

    /// Does what the filter asked.
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
