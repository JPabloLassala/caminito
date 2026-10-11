//! The repeats of a held grabbed accelerator (99): `AcceleratorRepeated` on
//! `org.mate.Caminito`, at the keyboard's delay and rate, as a Wayland client repeats a key
//! it holds. msd's volume keys go on changing the volume while they're held. `deadlines`
//! signals each when it's due (176).

use std::time::Duration;

use smithay::input::keyboard::Keycode;

use crate::state::Caminito;

/// The grabbed accelerator whose key is held, and repeating.
pub(crate) struct Repeat {
    keycode: Keycode,
    /// The grab's action id.
    action: u32,
    /// The grabber's unique name on the bus.
    owner: String,
    /// The time of the last signal, in the key events' milliseconds.
    time: u32,
    /// The time between two repeats, in milliseconds.
    period: u32,
    /// When the next repeat is signalled, on Caminito's clock.
    due: Duration,
}

impl Repeat {
    /// When the next repeat is due: `deadlines` signals it then (176).
    pub(crate) fn deadline(&self) -> Duration {
        self.due
    }
}

impl Caminito {
    /// `keycode`'s press at `time` was signalled as `owner`'s grab `action`: it repeats
    /// from now on, unless the keyboard's repeat is off.
    pub(super) fn start_accelerator_repeat(
        &mut self,
        keycode: Keycode,
        owner: &str,
        action: u32,
        time: u32,
    ) {
        self.stop_accelerator_repeat();
        let keyboard = &self.settings.keyboard;
        let (Ok(rate @ 1..), Ok(delay)) = (
            u32::try_from(keyboard.repeat_rate),
            u32::try_from(keyboard.repeat_delay),
        ) else {
            return;
        };
        let period = (1000 / rate).max(1);
        self.accelerator_repeat = Some(Repeat {
            keycode,
            action,
            owner: owner.to_owned(),
            // The first repeat adds a period.
            time: time.wrapping_add(delay).wrapping_sub(period),
            period,
            due: self.now() + Duration::from_millis(delay.into()),
        });
    }

    /// Ends the repeats, as another key's press does.
    pub(crate) fn stop_accelerator_repeat(&mut self) {
        self.accelerator_repeat = None;
    }

    /// `keycode` was released: its repeats end.
    pub(super) fn stop_accelerator_repeat_of(&mut self, keycode: Keycode) {
        if (self.accelerator_repeat.as_ref()).is_some_and(|repeat| repeat.keycode == keycode) {
            self.stop_accelerator_repeat();
        }
    }

    /// Signals the repeat if it's due at `now`, for `deadlines`, and ends the repeats if
    /// the grab was released since. A repeat that's late isn't made up for: the next is a
    /// period after `now`, so a resume from a suspend brings one repeat and not a burst.
    pub(crate) fn accelerator_repeat_elapse(&mut self, now: Duration) {
        let Some(repeat) = self.accelerator_repeat.as_mut() else {
            return;
        };
        if repeat.due > now {
            return;
        }
        if self.accelerator_grabs.owner(repeat.action) != Some(repeat.owner.as_str()) {
            self.accelerator_repeat = None;
            return;
        }
        repeat.time = repeat.time.wrapping_add(repeat.period);
        repeat.due = now + Duration::from_millis(repeat.period.into());
        let Some(repeat) = self.accelerator_repeat.as_ref() else {
            return;
        };
        self.signal_accelerator(&repeat.owner, repeat.action, repeat.time, true);
    }
}
