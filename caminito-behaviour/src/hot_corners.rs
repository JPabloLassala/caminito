//! The Hot corners page's keys (CMN-184): `org.mate.Caminito`'s four `hot-corner-*` keys
//! (CMN-183) and the Activation delay's (CMN-191), which Caminito follows as they change,
//! so a corner set here fires with no restart.

use gio::prelude::*;

#[cfg(test)]
mod tests;

/// The schema the keys are in.
pub const SCHEMA: &str = "org.mate.Caminito";

/// The Activation delay's key, in milliseconds.
const ACTIVATION_DELAY: &str = "hot-corner-activation-delay";

/// The longest Activation delay, in milliseconds: the top of its key's range.
pub const MAX_ACTIVATION_DELAY: u32 = 1000;

/// A corner of every output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    /// `hot-corner-top-left`'s.
    TopLeft,
    /// `hot-corner-top-right`'s.
    TopRight,
    /// `hot-corner-bottom-left`'s.
    BottomLeft,
    /// `hot-corner-bottom-right`'s.
    BottomRight,
}

impl Corner {
    /// The four, in reading order.
    pub const ALL: [Corner; 4] = [
        Corner::TopLeft,
        Corner::TopRight,
        Corner::BottomLeft,
        Corner::BottomRight,
    ];

    /// Its key.
    pub fn key(self) -> &'static str {
        match self {
            Corner::TopLeft => "hot-corner-top-left",
            Corner::TopRight => "hot-corner-top-right",
            Corner::BottomLeft => "hot-corner-bottom-left",
            Corner::BottomRight => "hot-corner-bottom-right",
        }
    }
}

/// What a corner does when the pointer rests in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The corner is off.
    None,
    /// Opens the Overview on the corner's output, or closes it.
    Overview,
    /// Opens the Application overview on the corner's output, or closes it (CMN-185).
    ApplicationOverview,
    /// Shows the desktop, or stops.
    ShowDesktop,
    /// Locks the session.
    Lock,
}

impl Action {
    /// The five, in the drop-downs' order.
    pub const ALL: [Action; 5] = [
        Action::None,
        Action::Overview,
        Action::ApplicationOverview,
        Action::ShowDesktop,
        Action::Lock,
    ];

    /// What the user reads for it.
    pub fn label(self) -> &'static str {
        match self {
            Action::None => "None",
            Action::Overview => "Overview",
            Action::ApplicationOverview => "Application overview",
            Action::ShowDesktop => "Show desktop",
            Action::Lock => "Lock",
        }
    }

    /// Its nick in `org.mate.Caminito.HotCornerAction`.
    pub fn nick(self) -> &'static str {
        match self {
            Action::None => "none",
            Action::Overview => "overview",
            Action::ApplicationOverview => "application-overview",
            Action::ShowDesktop => "show-desktop",
            Action::Lock => "lock",
        }
    }

    /// The action of `nick`, if it's one.
    fn from_nick(nick: &str) -> Option<Self> {
        Action::ALL.into_iter().find(|action| action.nick() == nick)
    }
}

/// Why the corners can't be opened, or one can't be set.
#[derive(Debug)]
pub enum Error {
    /// `org.mate.Caminito` isn't installed, or isn't compiled.
    NoSchema,
    /// Its key is locked down, or the backend failed.
    Key(&'static str, gio::glib::BoolError),
    /// An Activation delay past [`MAX_ACTIVATION_DELAY`], in milliseconds.
    TooLong(u32),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoSchema => write!(f, "the schema {SCHEMA} isn't installed"),
            Error::Key(key, err) => write!(f, "can't write {key}: {err}"),
            Error::TooLong(delay) => write!(
                f,
                "an activation delay of {delay} ms is past {MAX_ACTIVATION_DELAY} ms"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// The four corners' keys, and the Activation delay's.
pub struct HotCorners {
    settings: gio::Settings,
}

impl HotCorners {
    /// The corners of `settings`, which are `org.mate.Caminito`'s.
    pub fn new(settings: gio::Settings) -> Self {
        HotCorners { settings }
    }

    /// Opens the installed `org.mate.Caminito`.
    pub fn open() -> Result<Self, Error> {
        let installed = gio::SettingsSchemaSource::default()
            .is_some_and(|source| source.lookup(SCHEMA, true).is_some());
        if !installed {
            return Err(Error::NoSchema);
        }
        Ok(HotCorners::new(gio::Settings::new(SCHEMA)))
    }

    /// What `corner` does.
    pub fn action(&self, corner: Corner) -> Action {
        read_action(&self.settings, corner)
    }

    /// Calls `f` with a corner and its action each time its key changes, as by `gsettings` or
    /// by [`set`](Self::set).
    pub fn connect_changed(&self, f: impl Fn(Corner, Action) + 'static) {
        self.settings.connect_changed(None, move |settings, key| {
            let Some(corner) = Corner::ALL.into_iter().find(|corner| corner.key() == key) else {
                return;
            };
            f(corner, read_action(settings, corner));
        });
    }

    /// Writes `action` to `corner`'s key at once: there's no Apply.
    pub fn set(&self, corner: Corner, action: Action) -> Result<(), Error> {
        self.settings
            .set_string(corner.key(), action.nick())
            .map_err(|err| Error::Key(corner.key(), err))
    }

    /// Whether a corner has an action: the Activation delay is a corner's only then.
    pub fn any_on(&self) -> bool {
        Corner::ALL
            .into_iter()
            .any(|corner| self.action(corner) != Action::None)
    }

    /// The Activation delay, in milliseconds: how long the pointer rests in a corner before
    /// it fires, for all four.
    pub fn activation_delay(&self) -> u32 {
        self.settings.uint(ACTIVATION_DELAY)
    }

    /// Calls `f` with the Activation delay each time its key changes.
    pub fn connect_activation_delay_changed(&self, f: impl Fn(u32) + 'static) {
        self.settings
            .connect_changed(Some(ACTIVATION_DELAY), move |settings, _| {
                f(settings.uint(ACTIVATION_DELAY));
            });
    }

    /// Writes the Activation delay `delay`, in milliseconds, at once. One past
    /// [`MAX_ACTIVATION_DELAY`] isn't written: GSettings would refuse it with a critical.
    pub fn set_activation_delay(&self, delay: u32) -> Result<(), Error> {
        if delay > MAX_ACTIVATION_DELAY {
            return Err(Error::TooLong(delay));
        }
        self.settings
            .set_uint(ACTIVATION_DELAY, delay)
            .map_err(|err| Error::Key(ACTIVATION_DELAY, err))
    }
}

/// What `corner` of `settings` does.
fn read_action(settings: &gio::Settings, corner: Corner) -> Action {
    let nick = settings.string(corner.key());
    Action::from_nick(&nick).expect("the schema's enum has the four actions")
}
