//! Night Light (ADR 0023): Caminito warms every output toward a lower colour temperature by
//! writing its gamma table. `state` works out each output's temperature from the settings,
//! fading every change over about 2 s; the refresh pass hands them to the backend, and while
//! a fade runs a loop timer steps it. The tty backend writes the tables; the nested run and
//! headless keep the state and tint nothing.
//!
//! The sun schedule (138) is worked out about once a minute from the sun's elevation at the
//! location, by `sun`, `curve` and `zone`, and at once on a key change, a resume, or a jump
//! of the clock. The pause is 140's, and geoclue 139's.

mod curve;
mod ramp;
mod state;
mod sun;
mod zone;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use gio::glib;
use smithay::output::Output;
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use self::curve::Temperatures;
pub(crate) use self::ramp::{NEUTRAL, ramp};
use self::state::NightLightState;
use self::sun::Location;
use crate::output::output_id;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, NightLightSettings};
use crate::state::{Caminito, State};
use caminito_policy::OutputId;

/// How often a fade steps: 40 steps over its 2 s, each too small to see.
const STEP: Duration = Duration::from_millis(50);

/// How often the sun schedule is worked out again: the steps between are too small to see.
const MINUTE: Duration = Duration::from_secs(60);

/// How far the wall clock may move from Caminito's own between two workings-out before it
/// counts as a jump, as after a sleep, which Caminito's clock doesn't count, or a change of
/// the time. A jump fades.
const JUMP: f64 = 60.0;

/// Night Light's temperatures, the sun, and the timers that step their fades and work the
/// sun out again.
pub(crate) struct NightLight {
    state: NightLightState<OutputId>,
    /// The loop timer stepping the fades, while one runs.
    stepping: Option<RegistrationToken>,
    /// Where the time zone is read from.
    zone_files: ZoneFiles,
    /// The coordinates of each zone `localtime` has named, as `zone.tab` gave them.
    zones: HashMap<String, Option<Location>>,
    /// The sun as last worked out.
    sun: Sun,
    /// Caminito's clock and the wall clock, in Unix seconds, when the sun was last worked
    /// out, to tell a jump of the one from the other.
    worked_out: Option<(Duration, f64)>,
    /// The location source last logged, so that each fallback is logged once.
    logged: Option<Source>,
}

/// Where the sun schedule's location comes from (138), as `NightLight` and the dump name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// `night-light-last-location`.
    Stored,
    /// The time zone's coordinates in `zone.tab`.
    TimeZone,
    /// None: the fixed times stand in for the sun.
    None,
}

impl Source {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Source::Stored => "stored",
            Source::TimeZone => "time-zone",
            Source::None => "none",
        }
    }
}

/// The sun schedule's inputs and what came of them, at the last working-out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sun {
    pub(crate) source: Source,
    pub(crate) location: Option<Location>,
    /// The sun's elevation, in degrees, while there's a location.
    pub(crate) elevation: Option<f64>,
    /// The day's highest elevation, in degrees, while there's a location.
    pub(crate) highest: Option<f64>,
    /// The Unix time the sun next climbs past −6°, or the fixed times' 07:00, or 0 when it
    /// doesn't in the next two days, in a polar night or under the midnight sun.
    pub(crate) next_sunrise: i64,
    /// The sun schedule's temperature, in kelvin.
    pub(crate) temperature: u32,
}

/// The files the time zone is read from: `/etc/localtime` and the zoneinfo directory.
#[derive(Debug, Clone)]
pub(crate) struct ZoneFiles {
    pub(crate) localtime: PathBuf,
    pub(crate) zoneinfo: PathBuf,
}

impl Default for ZoneFiles {
    /// The system's. The tests' are the test zoneinfo directory, and no `localtime` until a
    /// test makes one, so they never read the host's.
    fn default() -> Self {
        #[cfg(not(test))]
        let files = ZoneFiles {
            localtime: "/etc/localtime".into(),
            zoneinfo: "/usr/share/zoneinfo".into(),
        };
        #[cfg(test)]
        let files = {
            let zoneinfo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/zoneinfo"));
            ZoneFiles {
                localtime: zoneinfo.join("no-localtime"),
                zoneinfo,
            }
        };
        files
    }
}

/// `org.mate.Caminito`'s `NightLight`, as its callers last heard it or are to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Published {
    /// The target, in kelvin: 6500 while Night Light is off.
    pub(crate) temperature: u32,
    /// Where the sun schedule's location comes from: `stored`, `time-zone` or `none`.
    pub(crate) location_source: &'static str,
    /// The Unix time the sun next climbs past −6°, as `Sun` has it.
    pub(crate) next_sunrise: i64,
}

impl NightLight {
    pub(crate) fn new(settings: NightLightSettings) -> Self {
        NightLight {
            state: NightLightState::new(settings),
            stepping: None,
            zone_files: ZoneFiles::default(),
            zones: HashMap::new(),
            sun: Sun {
                source: Source::None,
                location: None,
                elevation: None,
                highest: None,
                next_sunrise: 0,
                temperature: NEUTRAL,
            },
            worked_out: None,
            logged: None,
        }
    }

    /// The sun as last worked out.
    pub(crate) fn sun(&self) -> &Sun {
        &self.sun
    }

    /// Reads the time zone from `files` from now on, for a test.
    #[cfg(test)]
    pub(crate) fn set_zone_files(&mut self, files: ZoneFiles) {
        self.zone_files = files;
    }

    /// The location and where it comes from, under `settings`: the stored one, else the
    /// time zone's.
    fn locate(&mut self, settings: &NightLightSettings) -> (Source, Option<Location>) {
        if let Some((latitude, longitude)) = settings.last_location {
            let location = Location {
                latitude,
                longitude,
            };
            return (Source::Stored, Some(location));
        }
        let Some(name) = zone::zone_name(&self.zone_files.localtime) else {
            return (Source::None, None);
        };
        let zoneinfo = &self.zone_files.zoneinfo;
        let location = *self
            .zones
            .entry(name)
            .or_insert_with_key(|name| zone::coordinates(zoneinfo, name));
        match location {
            Some(location) => (Source::TimeZone, Some(location)),
            None => (Source::None, None),
        }
    }

    /// Works the sun out at `unix`, under `settings`.
    fn work_out(&mut self, settings: &NightLightSettings, unix: f64) -> Sun {
        let (source, location) = self.locate(settings);
        let temperatures = Temperatures {
            noon: settings.noon_temperature,
            night: settings.night_temperature,
        };
        let Some(location) = location else {
            let offset = self.utc_offset(unix);
            return Sun {
                source,
                location: None,
                elevation: None,
                highest: None,
                next_sunrise: curve::fixed_times_next_rise(unix, offset).round() as i64,
                temperature: curve::fixed_times_temperature(temperatures, unix + offset),
            };
        };
        let elevation = sun::elevation(location, unix);
        let highest = sun::highest(location, unix);
        Sun {
            source,
            location: Some(location),
            elevation: Some(elevation),
            highest: Some(highest),
            next_sunrise: sun::next_rise(location, unix).map_or(0, |rise| rise.round() as i64),
            temperature: curve::sun_temperature(temperatures, elevation, highest),
        }
    }

    /// How far local time is ahead of UTC at `unix`, in seconds, by the zone `localtime` is,
    /// for the fixed times. UTC when it can't be read.
    fn utc_offset(&self, unix: f64) -> f64 {
        let Some(path) = self.zone_files.localtime.to_str() else {
            return 0.0;
        };
        // GLib reads the zone file at a path, and is UTC when it can't.
        let zone = glib::TimeZone::new(Some(path));
        let interval = zone.find_interval(glib::TimeType::Universal, unix as i64);
        if interval < 0 {
            return 0.0;
        }
        f64::from(zone.offset(interval))
    }

    /// Logs the source the location fell to, once each time it changes.
    fn log_source(&mut self, sun: &Sun) {
        if self.logged == Some(sun.source) {
            return;
        }
        self.logged = Some(sun.source);
        match sun.source {
            Source::Stored => tracing::info!("night light follows the sun at the stored location"),
            Source::TimeZone => {
                tracing::info!("night light follows the sun at the time zone's coordinates");
            }
            Source::None => tracing::warn!(
                "night light has no location, so the sun schedule uses fixed times, 07:00 to 19:00"
            ),
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

    pub(crate) fn published(&self) -> Published {
        Published {
            temperature: self.target(),
            location_source: self.sun.source.name(),
            next_sunrise: self.sun.next_sunrise,
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
        let (sun, now) = self.work_out_sun_under(&settings);
        self.night_light.state.settings_changed(settings, sun, now);
        self.step_night_light();
    }

    /// Works the sun out at once, with no fade, and then about once a minute.
    pub(crate) fn start_night_light(&mut self) {
        self.work_out_sun(false);
        let timer = self
            .loop_handle
            .insert_source(Timer::from_duration(MINUTE), |_, _, state| {
                state.caminito.night_light_tick();
                TimeoutAction::ToDuration(MINUTE)
            });
        if let Err(err) = timer {
            tracing::warn!("can't set Night Light's minute timer: {}", err.error);
        }
    }

    /// The minute's working-out of the sun, as the timer does it and a test does once it has
    /// moved Caminito's clock on. A jump of the wall clock from Caminito's fades.
    pub(crate) fn night_light_tick(&mut self) {
        let jumped = self
            .night_light
            .worked_out
            .is_some_and(|(then, unix_then)| {
                let elapsed = self.now().saturating_sub(then).as_secs_f64();
                ((self.unix_now() - unix_then) - elapsed).abs() > JUMP
            });
        self.work_out_sun(jumped);
    }

    /// The machine has resumed from a sleep: the sun has moved on, and fades to where it is.
    pub(crate) fn night_light_resumed(&mut self) {
        self.work_out_sun(true);
    }

    /// Works the sun out again, now, and moves the target with it.
    fn work_out_sun(&mut self, fade: bool) {
        let settings = *self.night_light.settings();
        let (sun, now) = self.work_out_sun_under(&settings);
        self.night_light.state.sun_moved(sun, fade, now);
        self.step_night_light();
    }

    /// Works the sun out now under `settings`, and keeps it. Returns the sun schedule's
    /// temperature, and `now`.
    fn work_out_sun_under(&mut self, settings: &NightLightSettings) -> (u32, Duration) {
        let now = self.now();
        let unix = self.unix_now();
        let night_light = &mut self.night_light;
        let sun = night_light.work_out(settings, unix);
        night_light.log_source(&sun);
        night_light.sun = sun;
        night_light.worked_out = Some((now, unix));
        (sun.temperature, now)
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
