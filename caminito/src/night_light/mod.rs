//! Night Light (ADR 0023): Caminito warms every output toward a lower colour temperature by
//! writing its gamma table. `state` works out each output's temperature from the settings,
//! fading every change over about 2 s; the refresh pass hands them to the backend, and while
//! a fade runs a loop timer steps it. The tty backend writes the tables; the nested run and
//! headless keep the state and tint nothing.
//!
//! This is the constant schedule (137). The sun schedule is neutral until 138 works it out,
//! and the pause is 140's.

mod ramp;
mod state;

use std::time::Duration;

use smithay::output::Output;
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

pub(crate) use self::ramp::{NEUTRAL, ramp};
use self::state::NightLightState;
use crate::output::output_id;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, NightLightSettings};
use crate::state::{Caminito, State};
use caminito_policy::OutputId;

/// How often a fade steps: 40 steps over its 2 s, each too small to see.
const STEP: Duration = Duration::from_millis(50);

/// Night Light's temperatures and the timer that steps their fades.
pub(crate) struct NightLight {
    state: NightLightState<OutputId>,
    /// The loop timer stepping the fades, while one runs.
    stepping: Option<RegistrationToken>,
}

/// `org.mate.Caminito`'s `NightLight`, as its callers last heard it or are to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Published {
    /// The target, in kelvin: 6500 while Night Light is off.
    pub(crate) temperature: u32,
    /// Where the sun schedule's location comes from: `none` until 138.
    pub(crate) location_source: &'static str,
    /// The Unix time of the next sunrise: 0 until 138.
    pub(crate) next_sunrise: i64,
}

impl NightLight {
    pub(crate) fn new(settings: NightLightSettings) -> Self {
        NightLight {
            state: NightLightState::new(settings),
            stepping: None,
        }
    }

    pub(crate) fn settings(&self) -> &NightLightSettings {
        self.state.settings()
    }

    /// The temperature the outputs are to have, in kelvin.
    pub(crate) fn target(&self) -> u32 {
        self.state.target()
    }

    /// `output`'s temperature at `now`, in kelvin, partway through a fade or not.
    pub(crate) fn temperature(&self, output: &Output, now: Duration) -> u32 {
        self.state.temperature(output_id(output), now)
    }

    /// Where the sun schedule's location comes from, as `NightLight` and the dump name it.
    pub(crate) fn location_source(&self) -> &'static str {
        "none"
    }

    pub(crate) fn published(&self) -> Published {
        Published {
            temperature: self.target(),
            location_source: self.location_source(),
            next_sunrise: 0,
        }
    }
}

impl Caminito {
    /// An output is shown: it has the target at once.
    pub(crate) fn night_light_output_added(&mut self, id: OutputId) {
        self.night_light.state.add_output(id);
    }

    pub(crate) fn night_light_output_removed(&mut self, id: OutputId) {
        self.night_light.state.remove_output(id);
    }

    /// New Night Light settings: the outputs fade to the new target.
    pub(crate) fn night_light_settings_changed(&mut self, settings: NightLightSettings) {
        if settings == *self.night_light.settings() {
            return;
        }
        let now = self.now();
        self.night_light.state.settings_changed(settings, now);
        self.step_night_light();
    }

    /// Flips `night-light-enabled`, for `ToggleNightLight()`. The change comes back from the
    /// settings thread as any other.
    pub(crate) fn toggle_night_light(&self) {
        let enabled = self.night_light.settings().enabled;
        self.settings_thread
            .set(CAMINITO, "night-light-enabled", Value::Bool(!enabled));
    }

    /// Sets the timer that steps the fades, if one runs and nothing steps it. Each step wakes
    /// the loop, and the refresh pass after it hands the outputs' temperatures on.
    fn step_night_light(&mut self) {
        if self.night_light.stepping.is_some() || !self.night_light.state.is_fading(self.now()) {
            return;
        }
        let timer = self
            .loop_handle
            .insert_source(Timer::from_duration(STEP), |_, _, state| {
                let caminito = &mut state.caminito;
                if caminito.night_light.state.is_fading(caminito.now()) {
                    TimeoutAction::ToDuration(STEP)
                } else {
                    // The refresh pass after this hands on where the fade ended.
                    caminito.night_light.stepping = None;
                    TimeoutAction::Drop
                }
            });
        match timer {
            Ok(timer) => self.night_light.stepping = Some(timer),
            Err(err) => tracing::warn!("can't set Night Light's timer: {}", err.error),
        }
    }
}

impl State {
    /// The refresh pass's part of Night Light: each output's temperature now, to the
    /// backend, which writes a table only where it changed.
    pub(crate) fn refresh_night_light(&mut self) {
        let caminito = &self.caminito;
        let now = caminito.now();
        for output in &caminito.outputs {
            let kelvin = caminito.night_light.temperature(output, now);
            self.backend.set_temperature(output, kelvin);
        }
    }
}
