//! The Screen locking page's keys (CMN-337): the Blank delay, `org.mate.session idle-delay`
//! with `org.mate.screensaver idle-activation-enabled` for Never; the Lock delay,
//! `lock-delay` with `lock-enabled` for Never; and the Idle view's clock formats. Caminito
//! and caminito-locker follow them as they change. mate-power-manager's `sleep-display`
//! times, which Power sets, the page only reads.

use gio::prelude::*;

#[cfg(test)]
mod tests;

/// `org.mate.session`'s key of the Blank delay, in minutes.
const IDLE_DELAY: &str = "idle-delay";
/// `org.mate.screensaver`'s key that is off for a Blank delay of Never.
const IDLE_ACTIVATION: &str = "idle-activation-enabled";
/// `org.mate.screensaver`'s key of the Lock delay, in minutes after the Blank starts.
const LOCK_DELAY: &str = "lock-delay";
/// `org.mate.screensaver`'s key that is off for a Lock delay of Never.
const LOCK_ENABLED: &str = "lock-enabled";
/// `org.mate.power-manager`'s keys of when the outputs are powered off, in seconds, zero for
/// never: on mains power and on battery.
const SLEEP_DISPLAY: [&str; 2] = ["sleep-display-ac", "sleep-display-battery"];

/// A delay the user sets in minutes, or Never.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delay {
    /// After this many minutes.
    Minutes(u32),
    /// Not at all.
    Never,
}

impl Delay {
    /// What the user reads for it as a Blank delay.
    pub fn blank_label(self) -> String {
        match self {
            Delay::Minutes(minutes) => minutes_label(minutes),
            Delay::Never => "Never".into(),
        }
    }

    /// What the user reads for it as a Lock delay.
    pub fn lock_label(self) -> String {
        match self {
            Delay::Minutes(0) => "When it blanks".into(),
            Delay::Minutes(minutes) => format!("{} after it blanks", minutes_label(minutes)),
            Delay::Never => "Never".into(),
        }
    }
}

/// The Blank delays the page offers.
const BLANK_MINUTES: [u32; 8] = [1, 2, 3, 5, 10, 15, 30, 60];
/// The Lock delays the page offers.
const LOCK_MINUTES: [u32; 6] = [0, 1, 2, 5, 15, 30];

/// The Blank delays the drop-down offers while the key holds `current`, in its order.
pub fn blank_choices(current: Delay) -> Vec<Delay> {
    choices(&BLANK_MINUTES, current)
}

/// The Lock delays the drop-down offers while the key holds `current`, in its order.
pub fn lock_choices(current: Delay) -> Vec<Delay> {
    choices(&LOCK_MINUTES, current)
}

/// `offered` in minutes then Never, with `current` in its place among them if it isn't one:
/// a value set with `gsettings` shows as itself, and isn't overwritten until the user picks
/// another.
fn choices(offered: &[u32], current: Delay) -> Vec<Delay> {
    let mut minutes = offered.to_vec();
    if let Delay::Minutes(current) = current
        && let Err(place) = minutes.binary_search(&current)
    {
        minutes.insert(place, current);
    }
    minutes
        .into_iter()
        .map(Delay::Minutes)
        .chain([Delay::Never])
        .collect()
}

/// The clocks' format that caminito-locker writes for the locale, as no GDateTime format
/// can.
pub const LOCALE: &str = "locale";

/// A format the clock drop-down offers by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    /// What the user reads for it.
    pub label: &'static str,
    /// The key's value: `locale`, which caminito-locker formats for the locale, or a
    /// GDateTime format.
    pub format: &'static str,
}

/// One of the Idle view's two clocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    /// The time, `lock-dialog-time-format`.
    Time,
    /// The date, `lock-dialog-date-format`.
    Date,
}

impl Clock {
    /// The two, in the page's order.
    pub const ALL: [Clock; 2] = [Clock::Time, Clock::Date];

    /// Its key in `org.mate.screensaver`.
    pub fn key(self) -> &'static str {
        match self {
            Clock::Time => "lock-dialog-time-format",
            Clock::Date => "lock-dialog-date-format",
        }
    }

    /// What the user reads for it.
    pub fn label(self) -> &'static str {
        match self {
            Clock::Time => "Clock time",
            Clock::Date => "Clock date",
        }
    }

    /// The formats offered by name, the locale's first. The locale's date is caminito-locker's
    /// `%A, %B %e` translated, so the Long date is the same in English only.
    pub fn presets(self) -> &'static [Preset] {
        const LOCALE_PRESET: Preset = Preset {
            label: "Locale",
            format: LOCALE,
        };
        match self {
            Clock::Time => &[
                LOCALE_PRESET,
                Preset {
                    label: "24-hour",
                    format: "%R",
                },
                Preset {
                    label: "12-hour",
                    format: "%I:%M %p",
                },
            ],
            Clock::Date => &[
                LOCALE_PRESET,
                Preset {
                    label: "ISO 8601",
                    format: "%F",
                },
                Preset {
                    label: "Long",
                    format: "%A, %B %e",
                },
            ],
        }
    }

    /// The place among [`presets`](Self::presets) of `format`, or `None` for a Custom one.
    pub fn preset(self, format: &str) -> Option<usize> {
        self.presets()
            .iter()
            .position(|preset| preset.format == format)
    }
}

/// `moment` written in the GDateTime format `format`, as caminito-locker writes a format
/// other than `locale`, or `None` where GLib refuses the format.
pub fn preview(format: &str, moment: &gio::glib::DateTime) -> Option<String> {
    moment
        .format(format)
        .ok()
        .map(|text| text.trim().to_owned())
}

/// `minutes` as the user reads them.
fn minutes_label(minutes: u32) -> String {
    match minutes {
        1 => "1 minute".into(),
        minutes => format!("{minutes} minutes"),
    }
}

/// Why the keys can't be opened, or one can't be set.
#[derive(Debug)]
pub enum Error {
    /// This schema isn't installed, or isn't compiled.
    NoSchema(&'static str),
    /// Its key is locked down, or the backend failed.
    Key(&'static str, gio::glib::BoolError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoSchema(id) => write!(f, "the schema {id} isn't installed"),
            Error::Key(key, err) => write!(f, "can't write {key}: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// The page's keys: `org.mate.session`'s and `org.mate.screensaver`'s, and
/// `org.mate.power-manager`'s if it's installed.
pub struct ScreenLocking {
    session: gio::Settings,
    screensaver: gio::Settings,
    power: Option<gio::Settings>,
}

impl ScreenLocking {
    /// The keys of `session`, `screensaver` and `power`, which are `org.mate.session`,
    /// `org.mate.screensaver` and `org.mate.power-manager`.
    pub fn new(
        session: gio::Settings,
        screensaver: gio::Settings,
        power: Option<gio::Settings>,
    ) -> Self {
        ScreenLocking {
            session,
            screensaver,
            power,
        }
    }

    /// Opens the installed schemas. Without `org.mate.power-manager` there's no Power line.
    pub fn open() -> Result<Self, Error> {
        let open = |id| crate::installed_schema(id).ok_or(Error::NoSchema(id));
        Ok(ScreenLocking::new(
            open("org.mate.session")?,
            open("org.mate.screensaver")?,
            crate::installed_schema("org.mate.power-manager"),
        ))
    }

    /// The Blank delay.
    pub fn blank_delay(&self) -> Delay {
        read_blank_delay(&self.session, &self.screensaver)
    }

    /// Writes the Blank delay at once: there's no Apply. Never leaves `idle-delay` as it
    /// was, so that the minutes come back with blanking.
    pub fn set_blank_delay(&self, delay: Delay) -> Result<(), Error> {
        if let Delay::Minutes(minutes) = delay {
            set_int(&self.session, IDLE_DELAY, minutes)?;
        }
        set_boolean(&self.screensaver, IDLE_ACTIVATION, delay != Delay::Never)
    }

    /// The Lock delay: zero locks with the Blank.
    pub fn lock_delay(&self) -> Delay {
        read_lock_delay(&self.screensaver)
    }

    /// Writes the Lock delay at once. Never leaves `lock-delay` as it was.
    pub fn set_lock_delay(&self, delay: Delay) -> Result<(), Error> {
        if let Delay::Minutes(minutes) = delay {
            set_int(&self.screensaver, LOCK_DELAY, minutes)?;
        }
        set_boolean(&self.screensaver, LOCK_ENABLED, delay != Delay::Never)
    }

    /// `clock`'s format.
    pub fn format(&self, clock: Clock) -> String {
        self.screensaver.string(clock.key()).into()
    }

    /// Writes `clock`'s format at once.
    pub fn set_format(&self, clock: Clock, format: &str) -> Result<(), Error> {
        self.screensaver
            .set_string(clock.key(), format)
            .map_err(|err| Error::Key(clock.key(), err))
    }

    /// The Power line: when mate-power-manager has the outputs powered off, or `None` if
    /// its schema isn't installed.
    pub fn power_line(&self) -> Option<String> {
        self.power.as_ref().map(power_line)
    }

    /// Calls `f` with the Blank delay each time one of its two keys changes.
    pub fn connect_blank_delay_changed(&self, f: impl Fn(Delay) + 'static) {
        let f = std::rc::Rc::new(f);
        let (session, screensaver) = (self.session.clone(), self.screensaver.clone());
        let read = move || read_blank_delay(&session, &screensaver);
        let read = std::rc::Rc::new(read);
        for (settings, key) in [
            (&self.session, IDLE_DELAY),
            (&self.screensaver, IDLE_ACTIVATION),
        ] {
            let (f, read) = (f.clone(), read.clone());
            settings.connect_changed(Some(key), move |_, _| f(read()));
        }
    }

    /// Calls `f` with the Lock delay each time one of its two keys changes.
    pub fn connect_lock_delay_changed(&self, f: impl Fn(Delay) + 'static) {
        self.screensaver
            .connect_changed(None, move |settings, key| {
                if key == LOCK_DELAY || key == LOCK_ENABLED {
                    f(read_lock_delay(settings));
                }
            });
    }

    /// Calls `f` with a clock and its format each time its key changes.
    pub fn connect_format_changed(&self, f: impl Fn(Clock, String) + 'static) {
        self.screensaver
            .connect_changed(None, move |settings, key| {
                let Some(clock) = Clock::ALL.into_iter().find(|clock| clock.key() == key) else {
                    return;
                };
                f(clock, settings.string(key).into());
            });
    }

    /// Calls `f` with the Power line each time one of its keys changes.
    pub fn connect_power_line_changed(&self, f: impl Fn(String) + 'static) {
        let Some(power) = &self.power else {
            return;
        };
        power.connect_changed(None, move |settings, key| {
            if SLEEP_DISPLAY.contains(&key) {
                f(power_line(settings));
            }
        });
    }
}

/// The Blank delay of `session` and `screensaver`.
fn read_blank_delay(session: &gio::Settings, screensaver: &gio::Settings) -> Delay {
    if !screensaver.boolean(IDLE_ACTIVATION) {
        return Delay::Never;
    }
    Delay::Minutes(count(session, IDLE_DELAY))
}

/// The Lock delay of `screensaver`.
fn read_lock_delay(screensaver: &gio::Settings) -> Delay {
    if !screensaver.boolean(LOCK_ENABLED) {
        return Delay::Never;
    }
    Delay::Minutes(count(screensaver, LOCK_DELAY))
}

/// The Power line of `power`, `org.mate.power-manager`.
fn power_line(power: &gio::Settings) -> String {
    let [ac, battery] = SLEEP_DISPLAY.map(|key| count(power, key));
    match (ac, battery) {
        (0, 0) => "The screens never turn off.".into(),
        (ac, battery) if ac == battery => {
            format!("The screens turn off after {}.", seconds_label(ac))
        }
        (0, battery) => format!(
            "The screens turn off after {} on battery, and never otherwise.",
            seconds_label(battery)
        ),
        (ac, 0) => format!(
            "The screens turn off after {}, and never on battery.",
            seconds_label(ac)
        ),
        (ac, battery) => format!(
            "The screens turn off after {}, or {} on battery.",
            seconds_label(ac),
            seconds_label(battery)
        ),
    }
}

/// `seconds` as the user reads them: in hours or minutes where they're whole.
fn seconds_label(seconds: u32) -> String {
    match seconds {
        3600 => "1 hour".into(),
        seconds if seconds % 3600 == 0 => format!("{} hours", seconds / 3600),
        seconds if seconds % 60 == 0 => minutes_label(seconds / 60),
        seconds => format!("{seconds} seconds"),
    }
}

/// `settings`' `key`, a count of minutes or seconds, with a negative one read as zero.
fn count(settings: &gio::Settings, key: &str) -> u32 {
    settings.int(key).max(0) as u32
}

/// Writes `value` to `settings`' `key`, as the `i` GSettings has it.
fn set_int(settings: &gio::Settings, key: &'static str, value: u32) -> Result<(), Error> {
    let value = i32::try_from(value).unwrap_or(i32::MAX);
    settings
        .set_int(key, value)
        .map_err(|err| Error::Key(key, err))
}

/// Writes `value` to `settings`' `key`.
fn set_boolean(settings: &gio::Settings, key: &'static str, value: bool) -> Result<(), Error> {
    settings
        .set_boolean(key, value)
        .map_err(|err| Error::Key(key, err))
}
