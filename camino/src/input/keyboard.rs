//! The seat keyboard's settings (09, 53): the repeat info clients get, and NumLock
//! restored at start-up and written back, on every backend.

use smithay::input::keyboard::KeyboardHandle;

use crate::settings::Value;
use crate::settings::translate::{KEYBOARD, Keyboard};
use crate::state::{Camino, State};

/// Restores NumLock, when it's remembered, before any client has the keyboard
/// (`msd-keyboard-manager.c:260-265`).
pub(crate) fn restore_numlock(keyboard: &KeyboardHandle<State>, settings: &Keyboard) {
    if let (true, Some(on)) = (settings.remember_numlock, settings.numlock) {
        let mut mods = keyboard.modifier_state();
        mods.num_lock = on;
        keyboard.set_modifier_state(mods);
    }
}

impl Camino {
    /// New keyboard settings: clients get the new repeat info. NumLock isn't restored
    /// again, as msd doesn't.
    pub(crate) fn keyboard_settings_changed(&mut self, settings: &Keyboard) {
        let old = &self.settings.keyboard;
        if (settings.repeat_rate, settings.repeat_delay) != (old.repeat_rate, old.repeat_delay) {
            self.seat
                .get_keyboard()
                .unwrap()
                .change_repeat_info(settings.repeat_rate, settings.repeat_delay);
        }
    }

    /// Writes NumLock's state back when it changed and it's remembered, from the refresh
    /// pass (`msd-keyboard-manager.c:165-194`).
    pub(crate) fn refresh_numlock(&mut self) {
        let on = self.seat.get_keyboard().unwrap().modifier_state().num_lock;
        if on == self.numlock_seen {
            return;
        }
        self.numlock_seen = on;
        if self.settings.keyboard.remember_numlock {
            let nick = if on { "on" } else { "off" };
            self.settings_thread
                .set(KEYBOARD, "numlock-state", Value::Str(nick.into()));
        }
    }
}
