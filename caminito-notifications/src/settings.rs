//! `org.mate.Caminito.notifications` (ADR 0051, CMN-261): the notification settings, which
//! the server reads as each notification arrives or its bubble opens, so a change applies to
//! the next bubble with no restart. The settings window writes them, and the applets read
//! them from C, so the keys' names and nicks are a contract.

use std::time::Duration;

use gio::prelude::*;

/// The schema of the notification settings.
pub const SCHEMA: &str = "org.mate.Caminito.notifications";

/// The default Delivery, a [`Delivery`]'s nick.
pub const DELIVERY: &str = "delivery";
/// Whether notifications play a sound.
pub const SOUND: &str = "sound";
/// Whether bubbles show their notification's summary, body and image.
pub const PREVIEWS: &str = "previews";
/// Whether bubbles show their app's icon.
pub const APP_ICON: &str = "app-icon";
/// The bubbles' corner, a [`Corner`]'s nick.
pub const CORNER: &str = "corner";
/// How long a bubble stays when its app leaves that to the daemon, in seconds.
pub const TIMEOUT: &str = "timeout";
/// Whether critical notifications break through Do Not Disturb (CMN-263).
pub const CRITICAL_BREAKS_THROUGH: &str = "critical-breaks-through";
/// Whether Do Not Disturb has its daily schedule (CMN-264).
pub const DO_NOT_DISTURB_SCHEDULE: &str = "do-not-disturb-schedule";
/// When the schedule starts, in minutes from midnight.
pub const DO_NOT_DISTURB_FROM: &str = "do-not-disturb-from";
/// When the schedule ends, in minutes from midnight.
pub const DO_NOT_DISTURB_TO: &str = "do-not-disturb-to";
/// Whether Do Not Disturb is on while a window is fullscreen (CMN-264).
pub const DO_NOT_DISTURB_WHILE_FULLSCREEN: &str = "do-not-disturb-while-fullscreen";
/// Whether Do Not Disturb is on while a screencast runs (CMN-264).
pub const DO_NOT_DISTURB_WHILE_SCREENCASTING: &str = "do-not-disturb-while-screencasting";
/// The Notifying apps' keys (CMN-262).
pub const KNOWN_APPS: &str = "known-apps";

/// The text a bubble shows in place of its notification while Previews are off.
pub(crate) const NO_PREVIEW: &str = "Notification";

/// The installed notification settings, if their schema is installed and compiled.
pub fn installed() -> Option<gio::Settings> {
    let source = gio::SettingsSchemaSource::default()?;
    source.lookup(SCHEMA, true)?;
    Some(gio::Settings::new(SCHEMA))
}

/// Whether a new notification plays its sound.
pub(crate) fn sound(settings: &gio::Settings) -> bool {
    settings.boolean(SOUND)
}

/// Whether a bubble shows its notification's summary, body and image.
pub(crate) fn previews(settings: &gio::Settings) -> bool {
    settings.boolean(PREVIEWS)
}

/// Whether a bubble shows its app's icon.
pub(crate) fn app_icon(settings: &gio::Settings) -> bool {
    settings.boolean(APP_ICON)
}

/// How long a bubble stays when its app leaves that to the daemon.
pub(crate) fn timeout(settings: &gio::Settings) -> Duration {
    Duration::from_secs(settings.uint(TIMEOUT).into())
}

/// What a Notifying app's notifications get.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// A bubble and a place in the Notification history.
    Bubble,
    /// A place in the history only.
    History,
    /// Nothing at all.
    Off,
}

impl Delivery {
    /// The three, in the settings window's order.
    pub const ALL: [Delivery; 3] = [Delivery::Bubble, Delivery::History, Delivery::Off];

    /// What the user reads for it.
    pub fn label(self) -> &'static str {
        match self {
            Delivery::Bubble => "Bubble and history",
            Delivery::History => "History only",
            Delivery::Off => "Off",
        }
    }

    /// Its nick in `org.mate.Caminito.NotificationDelivery`.
    pub fn nick(self) -> &'static str {
        match self {
            Delivery::Bubble => "bubble",
            Delivery::History => "history",
            Delivery::Off => "off",
        }
    }
}

/// The corner of the output where the bubbles show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    /// The default, under the panel.
    TopRight,
    /// Top left.
    TopLeft,
    /// Bottom right.
    BottomRight,
    /// Bottom left.
    BottomLeft,
}

impl Corner {
    /// The four, in the settings window's order.
    pub const ALL: [Corner; 4] = [
        Corner::TopRight,
        Corner::TopLeft,
        Corner::BottomRight,
        Corner::BottomLeft,
    ];

    /// What the user reads for it.
    pub fn label(self) -> &'static str {
        match self {
            Corner::TopRight => "Top right",
            Corner::TopLeft => "Top left",
            Corner::BottomRight => "Bottom right",
            Corner::BottomLeft => "Bottom left",
        }
    }

    /// Its nick in `org.mate.Caminito.NotificationCorner`.
    pub fn nick(self) -> &'static str {
        match self {
            Corner::TopRight => "top-right",
            Corner::TopLeft => "top-left",
            Corner::BottomRight => "bottom-right",
            Corner::BottomLeft => "bottom-left",
        }
    }

    /// The corner `settings` name, or the default for a nick that isn't one.
    pub fn of(settings: &gio::Settings) -> Corner {
        let nick = settings.string(CORNER);
        Corner::ALL
            .into_iter()
            .find(|corner| corner.nick() == nick)
            .unwrap_or(Corner::TopRight)
    }

    /// Whether it's at the top of the output.
    pub fn is_top(self) -> bool {
        matches!(self, Corner::TopRight | Corner::TopLeft)
    }

    /// Whether it's at the right of the output.
    pub fn is_right(self) -> bool {
        matches!(self, Corner::TopRight | Corner::BottomRight)
    }
}
