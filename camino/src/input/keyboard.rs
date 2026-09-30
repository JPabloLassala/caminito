//! The seat keyboard's settings (09, 53, 54): its keymap, the repeat info clients get, and
//! NumLock restored at start-up and written back, on every backend.

use smithay::input::keyboard::{KeyboardHandle, XkbConfig};

use crate::settings::Value;
use crate::settings::translate::{KEYBOARD, Keyboard, Keymap};
use crate::state::{Camino, State};

/// The keymap's names for xkbcommon, with rules `evdev` (09). Empty options are left to
/// xkbcommon, like the other parts, rather than meaning none.
pub(crate) fn xkb_config(keymap: &Keymap) -> XkbConfig<'_> {
    XkbConfig {
        rules: "evdev",
        model: &keymap.model,
        layout: &keymap.layouts,
        variant: &keymap.variants,
        options: (!keymap.options.is_empty()).then(|| keymap.options.clone()),
    }
}

/// Restores NumLock, when it's remembered, before any client has the keyboard
/// (`msd-keyboard-manager.c:260-265`).
pub(crate) fn restore_numlock(keyboard: &KeyboardHandle<State>, settings: &Keyboard) {
    if let (true, Some(on)) = (settings.remember_numlock, settings.numlock) {
        let mut mods = keyboard.modifier_state();
        mods.num_lock = on;
        keyboard.set_modifier_state(mods);
    }
}

impl State {
    /// A new keymap for the seat's keyboard, which clients, Xwayland among them, are sent.
    /// One xkbcommon can't compile keeps the current keymap.
    pub(crate) fn keymap_changed(&mut self, keymap: &Keymap) {
        let keyboard = self.camino.seat.get_keyboard().unwrap();
        // A new keymap starts with nothing locked. Keep NumLock and CapsLock, or a layout
        // change would turn them off and the refresh pass would remember NumLock off.
        let locks = keyboard.modifier_state();
        if let Err(err) = keyboard.set_xkb_config(self, xkb_config(keymap)) {
            tracing::warn!("can't compile the keymap {keymap:?}: {err}; keeping the current one");
            return;
        }
        let mut mods = keyboard.modifier_state();
        if (mods.num_lock, mods.caps_lock) != (locks.num_lock, locks.caps_lock) {
            mods.num_lock = locks.num_lock;
            mods.caps_lock = locks.caps_lock;
            keyboard.set_modifier_state(mods);
            keyboard.advertise_modifier_state(self);
        }
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
