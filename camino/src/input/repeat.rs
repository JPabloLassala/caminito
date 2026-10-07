//! The repeats of a held grabbed accelerator (99): `AcceleratorRepeated` on
//! `org.mate.Camino`, at the keyboard's delay and rate, as a Wayland client repeats a key
//! it holds. msd's volume keys go on changing the volume while they're held.

use std::time::Duration;

use smithay::input::keyboard::Keycode;
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::state::Camino;

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
    timer: RegistrationToken,
}

impl Camino {
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
        let timer = Timer::from_duration(Duration::from_millis(delay.into()));
        let timer = self.loop_handle.insert_source(timer, |_, _, state| {
            match state.camino.signal_accelerator_repeat() {
                Some(period) => TimeoutAction::ToDuration(period),
                None => TimeoutAction::Drop,
            }
        });
        match timer {
            Ok(timer) => {
                self.accelerator_repeat = Some(Repeat {
                    keycode,
                    action,
                    owner: owner.to_owned(),
                    // The first repeat adds a period.
                    time: time.wrapping_add(delay).wrapping_sub(period),
                    period,
                    timer,
                });
            }
            Err(err) => tracing::warn!("can't repeat the accelerator {action}: {}", err.error),
        }
    }

    /// Ends the repeats, as another key's press does.
    pub(super) fn stop_accelerator_repeat(&mut self) {
        if let Some(repeat) = self.accelerator_repeat.take() {
            self.loop_handle.remove(repeat.timer);
        }
    }

    /// `keycode` was released: its repeats end.
    pub(super) fn stop_accelerator_repeat_of(&mut self, keycode: Keycode) {
        if (self.accelerator_repeat.as_ref()).is_some_and(|repeat| repeat.keycode == keycode) {
            self.stop_accelerator_repeat();
        }
    }

    /// The repeat's timer fired: signals the repeat, and returns the time until the next.
    /// It returns `None` when the grab was released since, which ends the repeats.
    fn signal_accelerator_repeat(&mut self) -> Option<Duration> {
        let repeat = self.accelerator_repeat.as_mut()?;
        if self.accelerator_grabs.owner(repeat.action) != Some(repeat.owner.as_str()) {
            self.accelerator_repeat = None;
            return None;
        }
        repeat.time = repeat.time.wrapping_add(repeat.period);
        let period = Duration::from_millis(repeat.period.into());
        let repeat = self.accelerator_repeat.as_ref()?;
        self.signal_accelerator(&repeat.owner, repeat.action, repeat.time, true);
        Some(period)
    }
}
