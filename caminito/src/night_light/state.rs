//! Night Light's temperatures, without the backend: the target the settings give, and each
//! output's on its way to it. Time is an argument, so the fades are tested without a clock.

use std::collections::BTreeMap;
use std::time::Duration;

use super::ramp::NEUTRAL;
use crate::settings::translate::{NightLightSettings, Schedule};

/// How long a change takes to fade in.
const FADE: Duration = Duration::from_secs(2);

/// The target and each output's temperature, by `K`, the output's id.
#[derive(Debug)]
pub(crate) struct NightLightState<K> {
    settings: NightLightSettings,
    outputs: BTreeMap<K, Light>,
}

/// One output's temperature: steady, or fading from one to another. A fade is in mireds,
/// 10⁶ / K, in which equal steps look equal.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Light {
    from: f64,
    to: f64,
    start: Duration,
}

impl Light {
    fn steady(kelvin: u32) -> Self {
        let mireds = mireds(kelvin);
        Light {
            from: mireds,
            to: mireds,
            start: Duration::ZERO,
        }
    }

    fn mireds_at(&self, now: Duration) -> f64 {
        let progress = now.saturating_sub(self.start).as_secs_f64() / FADE.as_secs_f64();
        self.from + (self.to - self.from) * progress.min(1.0)
    }

    fn at(&self, now: Duration) -> u32 {
        (1e6 / self.mireds_at(now)).round() as u32
    }

    /// Fades from where it is now to `kelvin`.
    fn fade_to(&mut self, kelvin: u32, now: Duration) {
        let to = mireds(kelvin);
        if to == self.to {
            return;
        }
        *self = Light {
            from: self.mireds_at(now),
            to,
            start: now,
        };
    }

    fn is_fading(&self, now: Duration) -> bool {
        self.from != self.to && now < self.start + FADE
    }
}

fn mireds(kelvin: u32) -> f64 {
    1e6 / f64::from(kelvin)
}

impl<K: Ord + Copy> NightLightState<K> {
    pub(crate) fn new(settings: NightLightSettings) -> Self {
        NightLightState {
            settings,
            outputs: BTreeMap::new(),
        }
    }

    pub(crate) fn settings(&self) -> &NightLightSettings {
        &self.settings
    }

    /// The temperature the outputs are to have, in kelvin: neutral while Night Light is
    /// off. The sun schedule is neutral until it's worked out (138).
    pub(crate) fn target(&self) -> u32 {
        match self.settings {
            NightLightSettings { enabled: false, .. } => NEUTRAL,
            NightLightSettings {
                schedule: Schedule::Constant,
                constant_temperature,
                ..
            } => constant_temperature,
            NightLightSettings {
                schedule: Schedule::Sun,
                ..
            } => NEUTRAL,
        }
    }

    /// New settings. Every output fades to a new target from where it is.
    pub(crate) fn settings_changed(&mut self, settings: NightLightSettings, now: Duration) {
        self.settings = settings;
        let target = self.target();
        for light in self.outputs.values_mut() {
            light.fade_to(target, now);
        }
    }

    /// A new output has the target at once, with no fade from neutral, as every output has
    /// at the Session's start.
    pub(crate) fn add_output(&mut self, id: K) {
        self.outputs.insert(id, Light::steady(self.target()));
    }

    pub(crate) fn remove_output(&mut self, id: K) {
        self.outputs.remove(&id);
    }

    /// The temperature of output `id` at `now`, in kelvin. One Night Light doesn't know of is
    /// at the target.
    pub(crate) fn temperature(&self, id: K, now: Duration) -> u32 {
        self.outputs
            .get(&id)
            .map_or_else(|| self.target(), |light| light.at(now))
    }

    /// Whether any output is still fading at `now`.
    pub(crate) fn is_fading(&self, now: Duration) -> bool {
        self.outputs.values().any(|light| light.is_fading(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(enabled: bool, schedule: Schedule, constant: u32) -> NightLightSettings {
        NightLightSettings {
            enabled,
            schedule,
            noon_temperature: 5500,
            night_temperature: 3400,
            constant_temperature: constant,
            paused_until: 0,
            last_location: None,
        }
    }

    fn off() -> NightLightSettings {
        settings(false, Schedule::Constant, 4000)
    }

    fn constant(kelvin: u32) -> NightLightSettings {
        settings(true, Schedule::Constant, kelvin)
    }

    fn secs(secs: f64) -> Duration {
        Duration::from_secs_f64(secs)
    }

    /// One output, 1, with Night Light set as `start` at the Session's start, at 10 s.
    fn one_output(start: NightLightSettings) -> NightLightState<u32> {
        let mut state = NightLightState::new(start);
        state.add_output(1);
        state
    }

    #[test]
    fn off_and_the_sun_schedule_are_neutral_and_the_constant_schedule_is_its_temperature() {
        assert_eq!(one_output(off()).target(), 6500);
        assert_eq!(
            one_output(settings(true, Schedule::Sun, 4000)).target(),
            6500
        );
        assert_eq!(one_output(constant(4000)).target(), 4000);
    }

    #[test]
    fn at_the_sessions_start_the_target_is_applied_at_once() {
        let state = one_output(constant(4000));
        assert_eq!(state.temperature(1, secs(10.0)), 4000);
        assert!(!state.is_fading(secs(10.0)));
    }

    #[test]
    fn an_output_plugged_in_later_has_the_target_at_once() {
        let mut state = one_output(constant(4000));
        state.add_output(2);
        assert_eq!(state.temperature(2, secs(20.0)), 4000);
        assert!(!state.is_fading(secs(20.0)));
    }

    #[test]
    fn turning_it_on_fades_to_the_target_in_two_seconds() {
        let mut state = one_output(off());
        state.settings_changed(constant(4000), secs(10.0));

        assert!(state.is_fading(secs(10.0)));
        assert_eq!(state.temperature(1, secs(10.0)), 6500);
        let halfway = state.temperature(1, secs(11.0));
        assert!((4000..6500).contains(&halfway), "{halfway}");
        // Halfway in mireds, between 153.8 and 250.
        assert_eq!(halfway, 4952);
        assert!(state.is_fading(secs(11.9)));
        assert_eq!(state.temperature(1, secs(12.0)), 4000);
        assert!(!state.is_fading(secs(12.0)));
        assert_eq!(state.temperature(1, secs(60.0)), 4000);
    }

    #[test]
    fn turning_it_off_fades_back_to_neutral() {
        let mut state = one_output(constant(4000));
        state.settings_changed(off(), secs(10.0));

        assert_eq!(state.temperature(1, secs(11.0)), 4952);
        assert_eq!(state.temperature(1, secs(12.0)), 6500);
        assert!(!state.is_fading(secs(12.0)));
    }

    #[test]
    fn a_change_mid_fade_starts_from_where_the_fade_is() {
        let mut state = one_output(off());
        state.settings_changed(constant(4000), secs(10.0));
        let there = state.temperature(1, secs(11.0));

        state.settings_changed(off(), secs(11.0));

        assert_eq!(state.temperature(1, secs(11.0)), there);
        let back = state.temperature(1, secs(12.0));
        assert!((there..6500).contains(&back), "{back}");
        // The new fade lasts its own two seconds.
        assert!(state.is_fading(secs(12.5)));
        assert_eq!(state.temperature(1, secs(13.0)), 6500);
    }

    #[test]
    fn a_change_that_keeps_the_target_doesnt_restart_the_fade() {
        let mut state = one_output(off());
        state.settings_changed(constant(4000), secs(10.0));
        let mut noon = constant(4000);
        noon.noon_temperature = 6000;

        state.settings_changed(noon, secs(11.0));

        assert_eq!(state.temperature(1, secs(12.0)), 4000);
    }

    #[test]
    fn every_output_fades_together() {
        let mut state = one_output(off());
        state.add_output(2);
        state.settings_changed(constant(3000), secs(10.0));
        assert_eq!(
            state.temperature(1, secs(11.0)),
            state.temperature(2, secs(11.0))
        );
    }

    #[test]
    fn an_output_unplugged_is_forgotten() {
        let mut state = one_output(off());
        state.settings_changed(constant(3000), secs(10.0));
        state.remove_output(1);
        assert!(!state.is_fading(secs(10.5)));
    }
}
