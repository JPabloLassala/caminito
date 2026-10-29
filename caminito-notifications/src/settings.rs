//! `org.mate.Caminito.notifications` (ADR 0051, CMN-261): the notification settings, which
//! the server reads as each notification arrives or its bubble opens, so a change applies to
//! the next bubble with no restart. The settings window writes them, and the applets read
//! them from C, so the keys' names and nicks are a contract.
//!
//! Each Notifying app may have its own settings too (CMN-262), at its path of the relocatable
//! `org.mate.Caminito.notifications.app` ([`app_path`]). [`Settings::effective`] resolves
//! them: each key the app's path sets, and the global key of the same name for the rest.

use std::time::Duration;

use gio::glib;
use gio::prelude::*;

use crate::DOMAIN;

/// The schema of the notification settings.
pub const SCHEMA: &str = "org.mate.Caminito.notifications";
/// The relocatable schema of a Notifying app's own settings.
pub const APP_SCHEMA: &str = "org.mate.Caminito.notifications.app";
/// Where the Notifying apps' paths are.
const APPS_PATH: &str = "/org/mate/caminito/notifications/apps/";

/// The default Delivery, a [`Delivery`]'s nick.
pub const DELIVERY: &str = "delivery";
/// Whether notifications play a sound.
pub const SOUND: &str = "sound";
/// Whether bubbles show their notification's summary, body and image.
pub const PREVIEWS: &str = "previews";
/// Whether bubbles show their app's icon.
pub const APP_ICON: &str = "app-icon";
/// Whether a Notifying app's notifications are grouped, in bubbles and the Notification
/// center (CMN-265).
pub const GROUP: &str = "group";
/// Whether a Notifying app's Dock item shows its Badge (CMN-267).
pub const BADGE: &str = "badge";
/// Whether a Notifying app's notifications break through Do Not Disturb (CMN-263).
pub const BREAKS_THROUGH: &str = "breaks-through";
/// The keys a Notifying app has of its own, and the default has too, in the Apps page's order.
pub const APP_KEYS: [&str; 7] = [
    DELIVERY,
    SOUND,
    PREVIEWS,
    APP_ICON,
    GROUP,
    BADGE,
    BREAKS_THROUGH,
];
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

/// The installed notification settings, if their schemas are installed and compiled.
pub fn installed() -> Option<Settings> {
    let source = gio::SettingsSchemaSource::default()?;
    source.lookup(SCHEMA, true)?;
    let app_schema = source.lookup(APP_SCHEMA, true)?;
    Some(Settings::new(gio::Settings::new(SCHEMA), app_schema))
}

/// The global notification settings, and the Notifying apps' own over the same backend.
#[derive(Clone)]
pub struct Settings {
    global: gio::Settings,
    app_schema: gio::SettingsSchema,
}

/// What one Notifying app's notifications get, its own settings or the default's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppSettings {
    /// What the app's notifications get.
    pub delivery: Delivery,
    /// Whether they play a sound.
    pub sound: bool,
    /// Whether their bubbles show their summary, body and image.
    pub previews: bool,
    /// Whether their bubbles show the app's icon.
    pub app_icon: bool,
    /// Whether they're grouped, in bubbles and the Notification center.
    pub group: bool,
    /// Whether the app's Dock item shows its Badge.
    pub badge: bool,
    /// Whether they break through Do Not Disturb.
    pub breaks_through: bool,
}

impl AppSettings {
    /// The boolean `key` of [`APP_KEYS`], or `None` for Delivery or a key that isn't one.
    pub fn flag(&self, key: &str) -> Option<bool> {
        match key {
            SOUND => Some(self.sound),
            PREVIEWS => Some(self.previews),
            APP_ICON => Some(self.app_icon),
            GROUP => Some(self.group),
            BADGE => Some(self.badge),
            BREAKS_THROUGH => Some(self.breaks_through),
            _ => None,
        }
    }
}

impl Settings {
    /// `global`, `org.mate.Caminito.notifications`'s settings, with `app_schema`,
    /// [`APP_SCHEMA`], for the apps' own, over `global`'s backend.
    pub fn new(global: gio::Settings, app_schema: gio::SettingsSchema) -> Settings {
        Settings { global, app_schema }
    }

    /// `org.mate.Caminito.notifications`'s settings, which hold the default.
    pub fn global(&self) -> &gio::Settings {
        &self.global
    }

    /// The Notifying app `app`'s own settings, at its [`app_path`].
    pub fn app(&self, app: &str) -> gio::Settings {
        gio::Settings::new_full(
            &self.app_schema,
            self.global.backend().as_ref(),
            Some(&app_path(app)),
        )
    }

    /// What `app`'s notifications get: each key its path sets, and the default's for the rest.
    pub fn effective(&self, app: &str) -> AppSettings {
        self.resolve(Some(&self.app(app)))
    }

    /// What an app without its own settings gets.
    pub fn defaults(&self) -> AppSettings {
        self.resolve(None)
    }

    /// Each key `own` sets, and the default's for the rest.
    fn resolve(&self, own: Option<&gio::Settings>) -> AppSettings {
        let value = |key: &str| {
            own.and_then(|own| own.user_value(key))
                .unwrap_or_else(|| self.global.value(key))
        };
        let boolean = |key: &str| value(key).get::<bool>().unwrap_or_default();
        AppSettings {
            delivery: value(DELIVERY)
                .str()
                .and_then(Delivery::of_nick)
                .unwrap_or(Delivery::Bubble),
            sound: boolean(SOUND),
            previews: boolean(PREVIEWS),
            app_icon: boolean(APP_ICON),
            group: boolean(GROUP),
            badge: boolean(BADGE),
            breaks_through: boolean(BREAKS_THROUGH),
        }
    }

    /// Whether `app` has settings of its own, rather than following the default.
    pub fn has_own(&self, app: &str) -> bool {
        let own = self.app(app);
        APP_KEYS.iter().any(|key| own.user_value(key).is_some())
    }

    /// Gives `app` settings of its own, the ones it gets now, so that a change of one leaves
    /// the rest as they were when the default changes.
    pub fn make_own(&self, app: &str) {
        let own = self.app(app);
        own.delay();
        for key in APP_KEYS {
            if own.user_value(key).is_none()
                && let Err(err) = own.set_value(key, &self.global.value(key))
            {
                glib::g_warning!(DOMAIN, "can't set {app}'s {key}: {err}");
            }
        }
        own.apply();
    }

    /// Takes `app`'s own settings away, so it follows the default.
    pub fn use_default(&self, app: &str) {
        let own = self.app(app);
        own.delay();
        for key in APP_KEYS {
            own.reset(key);
        }
        own.apply();
    }

    /// The Notifying apps' keys the daemon knows, in the order it met them.
    pub fn known_apps(&self) -> Vec<String> {
        self.global
            .strv(KNOWN_APPS)
            .iter()
            .map(|app| app.to_string())
            .collect()
    }

    /// Adds `app` to the known apps, unless it's there.
    pub(crate) fn remember(&self, app: &str) {
        let mut known = self.known_apps();
        if known.iter().any(|known| known == app) {
            return;
        }
        known.push(app.to_owned());
        if let Err(err) = self.global.set_strv(KNOWN_APPS, known) {
            glib::g_warning!(DOMAIN, "can't add {app} to the known apps: {err}");
        }
    }

    /// Whether critical notifications break through Do Not Disturb.
    pub(crate) fn critical_breaks_through(&self) -> bool {
        self.global.boolean(CRITICAL_BREAKS_THROUGH)
    }

    /// How long a bubble stays when its app leaves that to the daemon.
    pub(crate) fn timeout(&self) -> Duration {
        Duration::from_secs(self.global.uint(TIMEOUT).into())
    }
}

/// The path of the Notifying app `app`'s own settings: under [`APPS_PATH`], its key with each
/// byte other than an ASCII letter, a digit, `-` or `.` written as `_` and two lowercase hex
/// digits, so that every key has a path of its own. The empty key, which a path can't hold,
/// is a lone `_`, which no other key's is.
fn app_path(app: &str) -> String {
    let mut path = APPS_PATH.to_owned();
    if app.is_empty() {
        path.push('_');
    }
    for byte in app.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.' {
            path.push(char::from(byte));
        } else {
            path.push_str(&format!("_{byte:02x}"));
        }
    }
    path.push('/');
    path
}

/// Whether `key` of [`APP_KEYS`] does anything under `delivery`: without a bubble only
/// grouping in the Notification center and the Badge do, and with Off nothing does.
pub fn applies(delivery: Delivery, key: &str) -> bool {
    match delivery {
        Delivery::Bubble => true,
        Delivery::History => key == GROUP || key == BADGE,
        Delivery::Off => false,
    }
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

    /// The Delivery whose nick is `nick`, if it's one.
    pub fn of_nick(nick: &str) -> Option<Delivery> {
        Delivery::ALL
            .into_iter()
            .find(|delivery| delivery.nick() == nick)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_apps_path_is_its_key_with_other_bytes_than_letters_digits_dashes_and_dots_escaped() {
        assert_eq!(
            app_path("org.mozilla.firefox"),
            "/org/mate/caminito/notifications/apps/org.mozilla.firefox/"
        );
        assert_eq!(
            app_path("Mail Beta"),
            "/org/mate/caminito/notifications/apps/Mail_20Beta/"
        );
        assert_eq!(
            app_path("a_b/é"),
            "/org/mate/caminito/notifications/apps/a_5fb_2f_c3_a9/"
        );
        assert_eq!(app_path(""), "/org/mate/caminito/notifications/apps/_/");
    }

    #[test]
    fn without_a_bubble_only_group_and_badge_apply_and_with_off_nothing_does() {
        let applying = |delivery| {
            APP_KEYS[1..]
                .iter()
                .filter(|key| applies(delivery, key))
                .copied()
                .collect::<Vec<_>>()
        };
        assert_eq!(applying(Delivery::Bubble), APP_KEYS[1..]);
        assert_eq!(applying(Delivery::History), [GROUP, BADGE]);
        assert!(applying(Delivery::Off).is_empty());
    }
}
