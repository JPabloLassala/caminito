//! Night Light (ADR 0023): Caminito warms every output toward a lower colour temperature by
//! writing its gamma table. `state` works out each output's temperature from the settings,
//! fading every change over about 2 s; the refresh pass hands them to the backend, and while
//! a fade runs `deadlines` wakes the loop for each step (176). The tty backend writes the
//! tables; the nested run and headless keep the state and tint nothing.
//!
//! The sun schedule (138) is worked out about once a minute from the sun's elevation at the
//! location, by `sun`, `curve` and `zone`, and at once on a key change, a wake from sleep, or a jump
//! of the clock. geoclue is 139's.
//!
//! A pause (140) holds the target neutral until `night-light-paused-until`, which is checked
//! at each working-out and at the end itself. When it ends, or Night Light is turned on or
//! off, Caminito writes 0 to the key.
//!
//! The sun's position is worked out with it, whether or not Night Light is on, and published
//! for a dynamic wallpaper (215): the real sun at the location, or the fixed times' with
//! none.
//!
//! A gamma client (141) wins its own output: while its table is on it, Night Light steps
//! back from that output alone, and fades back in when the client lets go.

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

use self::curve::Temperatures;
pub(crate) use self::ramp::{NEUTRAL, ramp};
use self::state::NightLightState;
use self::sun::{Location, Position};
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

/// How long a pause lasts at most (140), in seconds: a day.
const LONGEST_PAUSE: f64 = 86_400.0;

/// Night Light's temperatures, and the sun.
pub(crate) struct NightLight {
    state: NightLightState<OutputId>,
    /// Where the time zone is read from.
    zone_files: ZoneFiles,
    /// The coordinates of each zone `localtime` has named, as `zone.tab` gave them.
    zones: HashMap<String, Option<Location>>,
    /// The sun as last worked out.
    sun: Sun,
    /// Caminito's clock and the wall clock, in Unix seconds, when the sun was last worked
    /// out, to tell a jump of the one from the other. The next working-out is a minute
    /// after it.
    worked_out: Option<(Duration, f64)>,
    /// The location source last logged, so that each fallback is logged once.
    logged: Option<Source>,
    /// The end of the pause Caminito last wrote 0 over, which no longer holds while the key
    /// still has it. 0 once the key has come back as 0, or a new pause is written.
    written_over: i64,
    /// The end of a pause Caminito wrote that the settings thread hasn't given back yet.
    /// Meanwhile no 0 is written, so that an old pause's end doesn't erase the new one.
    pause_written: Option<i64>,
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
    /// Where the sun is, or the fixed times' sun with no location, for a dynamic wallpaper.
    pub(crate) position: Position,
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

/// A working-out of the sun, and of the pause with it.
struct WorkedOut {
    /// The sun schedule's temperature, in kelvin.
    temperature: u32,
    /// Whether a pause holds.
    paused: bool,
    /// Whether the key holds a pause whose time has passed, which is to be cleared.
    ended: bool,
    now: Duration,
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

/// `org.mate.Caminito`'s `Sun` (215), as its callers last heard it or are to.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PublishedSun {
    pub(crate) position: Position,
    /// Where the location comes from, as `Published` has it.
    pub(crate) location_source: &'static str,
}

impl NightLight {
    pub(crate) fn new(settings: NightLightSettings) -> Self {
        NightLight {
            state: NightLightState::new(settings),
            zone_files: ZoneFiles::default(),
            zones: HashMap::new(),
            sun: Sun {
                source: Source::None,
                location: None,
                elevation: None,
                highest: None,
                position: Position {
                    elevation: 0.0,
                    azimuth: 0.0,
                },
                next_sunrise: 0,
                temperature: NEUTRAL,
            },
            worked_out: None,
            logged: None,
            written_over: 0,
            pause_written: None,
        }
    }

    /// The sun as last worked out.
    pub(crate) fn sun(&self) -> &Sun {
        &self.sun
    }

    /// When `deadlines` is next to wake the loop (176), at `now`: for the minute's
    /// working-out of the sun, or sooner while a fade runs, for its next step. The step has
    /// nothing to run: the refresh pass after it hands the outputs' temperatures on.
    pub(crate) fn deadline(&self, now: Duration) -> Option<Duration> {
        let step = self.state.is_fading(now).then(|| now + STEP);
        self.next_due().into_iter().chain(step).min()
    }

    /// When the sun is next to be worked out: at its minute, or sooner when a pause ends.
    fn next_due(&self) -> Option<Duration> {
        self.next_working_out()
            .into_iter()
            .chain(self.pause_deadline())
            .min()
    }

    /// When the pause that holds ends, by Caminito's clock, as the wall clock read at the
    /// last working-out. A jump of the wall clock moves it, and the next working-out sees
    /// that.
    fn pause_deadline(&self) -> Option<Duration> {
        if !self.state.is_paused() {
            return None;
        }
        let (then, unix_then) = self.worked_out?;
        // A millisecond past, so that the working-out then finds it over.
        let left = self.settings().paused_until as f64 - unix_then + 0.001;
        Some(then + Duration::from_secs_f64(left.max(0.0)))
    }

    /// Whether a pause holds at `unix` under `settings` (140): Night Light is on, and the
    /// end is ahead and isn't one Caminito has written 0 over.
    fn is_paused_at(&self, settings: &NightLightSettings, unix: f64) -> bool {
        let until = settings.paused_until;
        settings.enabled && until != 0 && until != self.written_over && until as f64 > unix
    }

    /// Whether a pause holds (140).
    pub(crate) fn is_paused(&self) -> bool {
        self.state.is_paused()
    }

    /// When the sun is next worked out: a minute after the last time, whatever asked for
    /// that one.
    fn next_working_out(&self) -> Option<Duration> {
        self.worked_out.map(|(then, _)| then + MINUTE)
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
                position: curve::fixed_times_position(unix + offset),
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
            position: Position {
                elevation,
                azimuth: sun::azimuth(location, unix),
            },
            next_sunrise: sun::next_rise(location, unix).map_or(0, |rise| rise.round() as i64),
            temperature: curve::sun_temperature(temperatures, elevation, highest),
        }
    }

    /// When a pause made at `unix` ends (140): the next sunrise, the sun past −6° or the
    /// fixed times' 07:00, under either schedule. It's at most a day away, in a polar night
    /// or under the midnight sun too.
    fn pause_end(&mut self, unix: f64) -> i64 {
        let settings = *self.settings();
        let cap = unix + LONGEST_PAUSE;
        let sun = self.work_out(&settings, unix);
        let rise = sun.next_sunrise as f64;
        if sun.next_sunrise > 0 && rise < cap {
            sun.next_sunrise
        } else {
            cap.round() as i64
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

    /// `output`'s temperature at `now`, in kelvin, partway through a fade or not, or `None`
    /// while a gamma client's table is on it.
    pub(crate) fn applied(&self, output: &Output, now: Duration) -> Option<u32> {
        let id = output_id(output);
        (!self.state.has_stepped_back(id)).then(|| self.state.temperature(id, now))
    }

    pub(crate) fn published(&self) -> Published {
        Published {
            temperature: self.target(),
            location_source: self.sun.source.name(),
            next_sunrise: self.sun.next_sunrise,
        }
    }

    /// The sun as `Sun` publishes it, for a dynamic wallpaper.
    pub(crate) fn published_sun(&self) -> PublishedSun {
        PublishedSun {
            position: self.sun.position,
            location_source: self.sun.source.name(),
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

    /// A gamma client's table is on `output`: Night Light steps back from it.
    pub(crate) fn night_light_step_back(&mut self, output: &Output) {
        self.night_light.state.step_back(output_id(output));
    }

    /// The gamma client let go of `output`, whose table the backend made neutral: Night
    /// Light fades back in on it.
    pub(crate) fn night_light_step_in(&mut self, output: &Output) {
        let now = self.now();
        self.night_light.state.step_in(output_id(output), now);
    }

    /// New Night Light settings: the outputs fade to the new target. Turning Night Light on
    /// or off ends a pause, whoever turned it (140).
    pub(crate) fn night_light_settings_changed(&mut self, settings: NightLightSettings) {
        let old = *self.night_light.settings();
        if settings == old {
            return;
        }
        let night_light = &mut self.night_light;
        if settings.paused_until == 0 {
            night_light.written_over = 0;
        }
        if night_light.pause_written == Some(settings.paused_until) {
            night_light.pause_written = None;
        }
        // Before the working-out, so that the pause no longer holds in it.
        if settings.enabled != old.enabled {
            self.clear_pause(settings.paused_until);
        }
        let worked = self.work_out_sun_under(&settings);
        self.night_light.state.settings_changed(
            settings,
            worked.temperature,
            worked.paused,
            worked.now,
        );
        if worked.ended {
            self.clear_pause(settings.paused_until);
        }
    }

    /// Works the sun out at once, with no fade. `deadlines` works it out again about once a
    /// minute from then on.
    pub(crate) fn start_night_light(&mut self) {
        self.work_out_sun(false);
    }

    /// Works the sun out if its minute or the pause's end is due at `now`, for `deadlines`.
    /// A minute that's late isn't made up for: the next is a minute after this one.
    pub(crate) fn night_light_elapse(&mut self, now: Duration) {
        if self.night_light.next_due().is_some_and(|due| due <= now) {
            self.night_light_tick();
        }
    }

    /// The minute's working-out of the sun, which a test also does once it has made the wall
    /// clock jump. A jump of the wall clock from Caminito's fades.
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

    /// Works the sun out again, now, and moves the target with it. A pause that ends, or
    /// that a jump of the clock ended, fades, and its key is cleared; so is one read at the
    /// start whose time has passed.
    fn work_out_sun(&mut self, fade: bool) {
        let settings = *self.night_light.settings();
        let started = self.night_light.worked_out.is_some();
        let worked = self.work_out_sun_under(&settings);
        let state = &mut self.night_light.state;
        let fade = fade || (started && worked.paused != state.is_paused());
        state.sun_moved(worked.temperature, worked.paused, fade, worked.now);
        if worked.ended {
            self.clear_pause(settings.paused_until);
        }
    }

    /// Works the sun out now under `settings`, and keeps it.
    fn work_out_sun_under(&mut self, settings: &NightLightSettings) -> WorkedOut {
        let now = self.now();
        let unix = self.unix_now();
        let night_light = &mut self.night_light;
        let sun = night_light.work_out(settings, unix);
        night_light.log_source(&sun);
        night_light.sun = sun;
        night_light.worked_out = Some((now, unix));
        let until = settings.paused_until;
        WorkedOut {
            temperature: sun.temperature,
            paused: night_light.is_paused_at(settings, unix),
            ended: until != 0 && until as f64 <= unix,
            now,
        }
    }

    /// Writes 0 over a pause that ends at `until`, once, and holds it no longer meanwhile.
    /// A pause written since, which the key will have, is left alone.
    fn clear_pause(&mut self, until: i64) {
        let night_light = &mut self.night_light;
        if until == 0 || until == night_light.written_over || night_light.pause_written.is_some() {
            return;
        }
        night_light.written_over = until;
        self.write_paused_until(0);
    }

    fn write_paused_until(&self, until: i64) {
        self.settings_thread
            .set(CAMINITO, "night-light-paused-until", Value::Int64(until));
    }

    /// Pauses Night Light until the next sunrise, for `PauseNightLight()`, by writing
    /// `night-light-paused-until`. It does nothing while Night Light is off. The change
    /// comes back from the settings thread as any other.
    pub(crate) fn pause_night_light(&mut self) {
        if !self.night_light.settings().enabled {
            return;
        }
        let until = self.night_light.pause_end(self.unix_now());
        self.night_light.written_over = 0;
        self.night_light.pause_written = Some(until);
        self.write_paused_until(until);
    }

    /// Ends a pause, for `ResumeNightLight()`, by writing 0 to `night-light-paused-until`.
    pub(crate) fn resume_night_light(&self) {
        self.write_paused_until(0);
    }

    /// Flips `night-light-enabled`, for `ToggleNightLight()`. The change comes back from the
    /// settings thread as any other.
    pub(crate) fn toggle_night_light(&self) {
        let enabled = self.night_light.settings().enabled;
        self.settings_thread
            .set(CAMINITO, "night-light-enabled", Value::Bool(!enabled));
    }
}

impl State {
    /// The refresh pass's part of Night Light: each output's temperature now, to the
    /// backend, which writes a table only where it changed. Where a gamma client's table
    /// is on an output, the backend keeps it, and writes it again if a resume or a
    /// modeset may have lost it.
    pub(crate) fn refresh_night_light(&mut self) {
        let caminito = &self.caminito;
        let now = caminito.now();
        for output in &caminito.outputs {
            let kelvin = caminito
                .night_light
                .state
                .temperature(output_id(output), now);
            self.backend.set_temperature(output, kelvin);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::translate::Schedule;

    const HOUR: f64 = 3600.0;

    /// 2026-12-21 at 00:00 UTC, 21:00 the evening before in Montevideo.
    const DECEMBER: f64 = 1_797_811_200.0;

    const MONTEVIDEO: Location = Location {
        latitude: -34.9011,
        longitude: -56.1645,
    };
    const LONGYEARBYEN: Location = Location {
        latitude: 78.2232,
        longitude: 15.6267,
    };

    /// Night Light on, at `location`, or with none.
    fn at(location: Option<Location>) -> NightLight {
        NightLight::new(NightLightSettings {
            enabled: true,
            schedule: Schedule::Sun,
            noon_temperature: 5500,
            night_temperature: 3400,
            constant_temperature: 4000,
            paused_until: 0,
            last_location: location.map(|it| (it.latitude, it.longitude)),
        })
    }

    #[test]
    fn a_pause_at_ten_at_night_ends_at_tomorrows_civil_twilight() {
        // 22:00 in Montevideo, three hours behind, is 01:00 UTC on the 22nd. Its sun climbs
        // past −6° at about 07:58 UTC.
        let unix = DECEMBER + 25.0 * HOUR;
        let end = at(Some(MONTEVIDEO)).pause_end(unix);
        let after = end as f64 - unix;
        assert!((6.5 * HOUR..7.5 * HOUR).contains(&after), "{after}");
    }

    #[test]
    fn a_pause_at_ten_in_the_morning_ends_at_tomorrows_too() {
        // 10:00 in Montevideo, 13:00 UTC on the 21st: today's twilight is past.
        let unix = DECEMBER + 13.0 * HOUR;
        let end = at(Some(MONTEVIDEO)).pause_end(unix);
        let after = end as f64 - unix;
        assert!((18.5 * HOUR..19.5 * HOUR).contains(&after), "{after}");
    }

    #[test]
    fn a_pause_in_a_polar_night_ends_a_day_later() {
        // Longyearbyen, 2026-12-15 at 00:00 UTC: the sun stays below −6° for weeks.
        let unix = 1_797_292_800.0;
        assert_eq!(
            at(Some(LONGYEARBYEN)).pause_end(unix),
            (unix + LONGEST_PAUSE) as i64
        );
    }

    #[test]
    fn a_pause_with_no_location_ends_at_seven() {
        // The tests' `localtime` is missing: UTC, so 07:00 UTC.
        let unix = DECEMBER + 22.0 * HOUR;
        assert_eq!(
            at(None).pause_end(unix),
            (DECEMBER + LONGEST_PAUSE + 7.0 * HOUR) as i64
        );
    }
}
