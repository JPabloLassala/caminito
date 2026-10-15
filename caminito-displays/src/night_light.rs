//! The Night Light tab's state (142), with no toolkit and no bus in it: `org.mate.Caminito`'s
//! `night-light-*` keys as the tab has them, the location source of Caminito's `NightLight`
//! property, what the tab shows of them, and the key each change of the user's writes.
//!
//! The keys are written as they change, with no Apply and no countdown: nothing on the tab
//! can leave the screen unusable. A change from elsewhere, Caminito's own writes of
//! `night-light-paused-until` among them, comes back through `refresh`.

#[cfg(test)]
mod tests;

/// The warmest temperature the keys take, in kelvin.
pub const MIN_TEMPERATURE: u32 = 1000;
/// The most neutral temperature the keys take, in kelvin: daylight, no tint.
pub const MAX_TEMPERATURE: u32 = 6500;
/// The step of the sliders, in kelvin.
pub const STEP: u32 = 100;

/// `night-light-schedule`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    /// "Follow the sun": the noon and night temperatures, by the sun's elevation.
    Sun,
    /// The constant temperature all the time.
    Constant,
}

impl Schedule {
    /// The key's nick for it.
    pub fn nick(self) -> &'static str {
        match self {
            Schedule::Sun => "sun",
            Schedule::Constant => "constant",
        }
    }

    /// The schedule a nick of the key's names.
    pub fn from_nick(nick: &str) -> Option<Schedule> {
        match nick {
            "sun" => Some(Schedule::Sun),
            "constant" => Some(Schedule::Constant),
            _ => None,
        }
    }

    /// As the tab shows it.
    pub fn label(self) -> &'static str {
        match self {
            Schedule::Sun => "Follow the sun",
            Schedule::Constant => "Constant",
        }
    }
}

/// One of the three temperatures a schedule uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Temperature {
    /// The sun schedule's at the day's highest sun.
    Noon,
    /// The sun schedule's while the sun is below civil twilight.
    Night,
    /// The constant schedule's.
    Constant,
}

impl Temperature {
    /// Its key.
    pub fn key(self) -> &'static str {
        match self {
            Temperature::Noon => "night-light-noon-temperature",
            Temperature::Night => "night-light-night-temperature",
            Temperature::Constant => "night-light-constant-temperature",
        }
    }

    /// The name of its slider.
    pub fn label(self) -> &'static str {
        match self {
            Temperature::Noon => "At noon",
            Temperature::Night => "At night",
            Temperature::Constant => "Temperature",
        }
    }
}

/// The `night-light-*` keys the tab reads. `night-light-last-location` is Caminito's alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// `night-light-enabled`.
    pub enabled: bool,
    /// `night-light-schedule`.
    pub schedule: Schedule,
    /// `night-light-noon-temperature`, in kelvin.
    pub noon: u32,
    /// `night-light-night-temperature`, in kelvin.
    pub night: u32,
    /// `night-light-constant-temperature`, in kelvin.
    pub constant: u32,
    /// `night-light-paused-until`: the Unix time a pause ends, or 0.
    pub paused_until: i64,
}

impl Default for Settings {
    /// The schema's defaults.
    fn default() -> Self {
        Settings {
            enabled: false,
            schedule: Schedule::Sun,
            noon: 5500,
            night: 3400,
            constant: 4000,
            paused_until: 0,
        }
    }
}

impl Settings {
    /// One of its temperatures.
    pub fn temperature(&self, which: Temperature) -> u32 {
        match which {
            Temperature::Noon => self.noon,
            Temperature::Night => self.night,
            Temperature::Constant => self.constant,
        }
    }

    fn temperature_mut(&mut self, which: Temperature) -> &mut u32 {
        match which {
            Temperature::Noon => &mut self.noon,
            Temperature::Night => &mut self.night,
            Temperature::Constant => &mut self.constant,
        }
    }
}

/// A key to write, with its new value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Write {
    /// `night-light-enabled`.
    Enabled(bool),
    /// `night-light-schedule`, as its nick.
    Schedule(Schedule),
    /// One of the temperatures, in kelvin.
    Temperature(Temperature, u32),
}

impl Write {
    /// The key it writes.
    pub fn key(self) -> &'static str {
        match self {
            Write::Enabled(_) => "night-light-enabled",
            Write::Schedule(_) => "night-light-schedule",
            Write::Temperature(which, _) => which.key(),
        }
    }
}

/// What the pause button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pause {
    /// Pauses until sunrise, through `PauseNightLight()`.
    Pause,
    /// Ends the pause, which lasts until `until`, through `ResumeNightLight()`.
    Resume {
        /// The Unix time the pause ends.
        until: i64,
    },
}

impl Pause {
    /// The button's label.
    pub fn label(self) -> &'static str {
        match self {
            Pause::Pause => "Pause until sunrise",
            Pause::Resume { .. } => "Resume",
        }
    }

    /// What the tab says beside the button: until when it's paused, as `local` gives the
    /// hour and the minute of a Unix time.
    pub fn text(self, local: impl Fn(i64) -> (u32, u32)) -> Option<String> {
        match self {
            Pause::Pause => None,
            Pause::Resume { until } => {
                let (hour, minute) = local(until);
                Some(format!("Paused until {hour:02}:{minute:02}"))
            }
        }
    }
}

/// A temperature as the tab shows it.
pub fn kelvin(temperature: u32) -> String {
    format!("{temperature} K")
}

/// The tab: the keys, and where Caminito has its location from.
#[derive(Debug, Clone)]
pub struct Tab {
    settings: Settings,
    /// `NightLight`'s `location-source`, while Caminito publishes it.
    location_source: Option<String>,
}

impl Tab {
    /// The tab over `settings`, with Caminito's `location_source` if it's known.
    pub fn new(settings: Settings, location_source: Option<String>) -> Tab {
        Tab {
            settings,
            location_source,
        }
    }

    /// The keys as the tab has them.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The keys have `settings` now. Returns whether that's a change.
    pub fn refresh(&mut self, settings: Settings) -> bool {
        let changed = self.settings != settings;
        self.settings = settings;
        changed
    }

    /// Caminito has its location from `source` now, or publishes none. Returns whether
    /// that's a change.
    pub fn set_location_source(&mut self, source: Option<String>) -> bool {
        let changed = self.location_source != source;
        self.location_source = source;
        changed
    }

    /// The sliders the schedule uses, in order, with their temperatures.
    pub fn sliders(&self) -> Vec<(Temperature, u32)> {
        let shown: &[Temperature] = match self.settings.schedule {
            Schedule::Sun => &[Temperature::Noon, Temperature::Night],
            Schedule::Constant => &[Temperature::Constant],
        };
        shown
            .iter()
            .map(|&which| (which, self.settings.temperature(which)))
            .collect()
    }

    /// The switch was turned. The write, unless the key has it already.
    pub fn set_enabled(&mut self, enabled: bool) -> Option<Write> {
        (self.settings.enabled != enabled).then(|| {
            self.settings.enabled = enabled;
            Write::Enabled(enabled)
        })
    }

    /// A schedule was chosen. The write, unless the key has it already.
    pub fn set_schedule(&mut self, schedule: Schedule) -> Option<Write> {
        (self.settings.schedule != schedule).then(|| {
            self.settings.schedule = schedule;
            Write::Schedule(schedule)
        })
    }

    /// A slider was moved to `kelvin`, which is kept to the keys' range in the sliders'
    /// steps. The write, unless the key has it already.
    pub fn set_temperature(&mut self, which: Temperature, kelvin: u32) -> Option<Write> {
        let kelvin = ((kelvin + STEP / 2) / STEP * STEP).clamp(MIN_TEMPERATURE, MAX_TEMPERATURE);
        let temperature = self.settings.temperature_mut(which);
        (*temperature != kelvin).then(|| {
            *temperature = kelvin;
            Write::Temperature(which, kelvin)
        })
    }

    /// Whether there's anything to pause or resume: Night Light is on.
    pub fn can_pause(&self) -> bool {
        self.settings.enabled
    }

    /// What the pause button does at the Unix time `now`. A pause whose time has passed is
    /// over, though Caminito may not have written 0 yet.
    pub fn pause(&self, now: i64) -> Pause {
        let until = self.settings.paused_until;
        if self.settings.enabled && until > now {
            Pause::Resume { until }
        } else {
            Pause::Pause
        }
    }

    /// Why the sun schedule's times may be off, when they may.
    pub fn note(&self) -> Option<&'static str> {
        (self.location_source.as_deref() == Some("none"))
            .then_some("Location unknown, using fixed times")
    }
}
