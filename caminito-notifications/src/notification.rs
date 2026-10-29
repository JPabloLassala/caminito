//! A notification as `Notify` gives it: what its bubble shows, its actions, its urgency, its
//! sound and how long it stays, read from the call's arguments and hints (the Desktop
//! Notifications spec 1.3, "Hints").

use std::path::PathBuf;
use std::time::Duration;

use gio::glib::{self, Variant, VariantDict};
use gio::prelude::*;

use crate::markup;
use crate::server::{Action, Bubble, Icon, Image, Pixels, Sound};

/// The action a click on the bubble invokes, which has no button (the spec's "default").
pub(crate) const DEFAULT_ACTION: &str = "default";

/// The theme's sound for a notification that names none.
const DEFAULT_SOUND: &str = "message-new-instant";

/// The spec's `urgency` byte for a critical notification.
const CRITICAL: u8 = 2;

/// What the server keeps of a `Notify`.
pub(crate) struct Notification {
    /// The Notifying app's key: the `desktop-entry` hint, or else `app_name`.
    pub(crate) app: String,
    pub(crate) bubble: Bubble,
    /// Whether it has the default action, which a click invokes.
    pub(crate) has_default: bool,
    /// Whether it stays after one of its actions is used (`resident`).
    pub(crate) resident: bool,
    /// Whether it never enters the Notification history (`transient`).
    pub(crate) transient: bool,
    /// Whether it has a reply field (KDE's `x-kde-reply-placeholder-text`).
    pub(crate) reply: bool,
    /// Whether its urgency is critical.
    pub(crate) critical: bool,
    /// What plays as it arrives, if anything.
    pub(crate) sound: Option<Sound>,
    /// How long its bubble stays, or `None` for as long as the user leaves it.
    pub(crate) timeout: Option<Duration>,
}

/// `Notify`'s arguments other than `replaces_id`.
pub(crate) struct Arguments<'a> {
    pub(crate) app_name: &'a str,
    pub(crate) app_icon: &'a str,
    pub(crate) summary: &'a str,
    pub(crate) body: &'a str,
    pub(crate) actions: Vec<String>,
    pub(crate) hints: VariantDict,
    pub(crate) expire_timeout: i32,
}

impl Notification {
    /// The notification `arguments` make, with `default_timeout` for one that leaves its
    /// timeout to the server.
    pub(crate) fn new(arguments: Arguments<'_>, default_timeout: Duration) -> Notification {
        let hints = &arguments.hints;
        let critical = hint::<u8>(hints, "urgency") == Some(CRITICAL);
        let entry_id = hint::<String>(hints, "desktop-entry")
            .map(|id| id.trim_end_matches(".desktop").to_owned())
            .filter(|id| !id.is_empty());
        let entry = entry_id.as_deref().and_then(desktop_entry);
        let app = entry_id.unwrap_or_else(|| arguments.app_name.to_owned());
        let app_name = match (arguments.app_name, &entry) {
            ("", Some(entry)) => entry.name.clone(),
            (name, _) => name.to_owned(),
        };
        let app_icon = icon(arguments.app_icon)
            .or_else(|| entry.as_ref().and_then(|entry| entry.icon.clone()));

        // The spec's actions are a flat list of keys and labels.
        let mut has_default = false;
        let mut actions = Vec::new();
        for [key, label] in arguments.actions.as_chunks::<2>().0 {
            if key == DEFAULT_ACTION {
                has_default = true;
            } else {
                actions.push(Action {
                    key: key.clone(),
                    label: label.clone(),
                });
            }
        }

        let bubble = Bubble {
            app_name,
            app_icon,
            summary: arguments.summary.to_owned(),
            body: markup::sanitize(arguments.body),
            image: image(hints),
            actions,
        };
        Notification {
            app,
            bubble,
            has_default,
            resident: hint::<bool>(hints, "resident").unwrap_or(false),
            transient: hint::<bool>(hints, "transient").unwrap_or(false),
            reply: hint::<String>(hints, "x-kde-reply-placeholder-text").is_some(),
            critical,
            sound: sound(hints),
            timeout: timeout(arguments.expire_timeout, critical, default_timeout),
        }
    }
}

/// How long a bubble stays: never for a critical notification, and otherwise the app's
/// `expire_timeout`, where 0 is never and -1 (or any negative) is the server's default.
fn timeout(expire_timeout: i32, critical: bool, default: Duration) -> Option<Duration> {
    match expire_timeout {
        _ if critical => None,
        0 => None,
        milliseconds if milliseconds < 0 => Some(default),
        milliseconds => Some(Duration::from_millis(milliseconds.unsigned_abs().into())),
    }
}

/// The sound that plays as the notification arrives: none with `suppress-sound`, the
/// `sound-file` or the `sound-name`, and otherwise the theme's for a notification.
fn sound(hints: &VariantDict) -> Option<Sound> {
    if hint::<bool>(hints, "suppress-sound") == Some(true) {
        return None;
    }
    if let Some(file) = hint::<String>(hints, "sound-file").filter(|file| !file.is_empty()) {
        return Some(Sound::File(PathBuf::from(file)));
    }
    let name = hint::<String>(hints, "sound-name").filter(|name| !name.is_empty());
    Some(Sound::Name(
        name.unwrap_or_else(|| DEFAULT_SOUND.to_owned()),
    ))
}

/// The bubble's image: the pixels of `image-data`, or else the icon or file `image-path`
/// names, with the names of the spec's older versions too.
fn image(hints: &VariantDict) -> Option<Image> {
    for key in ["image-data", "image_data", "icon_data"] {
        if let Some(pixels) = hints.lookup_value(key, None).and_then(|data| pixels(&data)) {
            return Some(Image::Pixels(pixels));
        }
    }
    ["image-path", "image_path"]
        .into_iter()
        .find_map(|key| hint::<String>(hints, key))
        .and_then(|path| icon(&path))
        .map(Image::Icon)
}

/// The pixels of an `image-data` hint, `(iiibiiay)`.
pub(crate) fn pixels(data: &Variant) -> Option<Pixels> {
    if data.type_().as_str() != "(iiibiiay)" {
        return None;
    }
    let int = |index| data.child_value(index).get::<i32>();
    let pixels = Pixels {
        width: int(0)?,
        height: int(1)?,
        rowstride: int(2)?,
        has_alpha: data.child_value(3).get::<bool>()?,
        bits_per_sample: int(4)?,
        channels: int(5)?,
        data: data.child_value(6).data_as_bytes(),
    };
    pixels.is_drawable().then_some(pixels)
}

/// An app icon or an image path: a file, as a path or a `file://` URI, or else an icon
/// theme's name. Empty is none.
pub(crate) fn icon(text: &str) -> Option<Icon> {
    if text.is_empty() {
        None
    } else if text.starts_with("file://") {
        gio::File::for_uri(text).path().map(Icon::File)
    } else if text.starts_with('/') {
        Some(Icon::File(PathBuf::from(text)))
    } else {
        Some(Icon::Name(text.to_owned()))
    }
}

/// What a desktop entry tells of its app.
struct DesktopEntry {
    name: String,
    icon: Option<Icon>,
}

/// The app of the `desktop-entry` hint, which is the entry's id without `.desktop`, from the
/// XDG data directories' `applications/`.
fn desktop_entry(id: &str) -> Option<DesktopEntry> {
    const GROUP: &str = "Desktop Entry";
    let file = glib::KeyFile::new();
    file.load_from_data_dirs(
        format!("applications/{id}.desktop"),
        glib::KeyFileFlags::NONE,
    )
    .ok()?;
    let name = file.locale_string(GROUP, "Name", None).ok()?;
    let icon = file.string(GROUP, "Icon").ok().and_then(|text| icon(&text));
    Some(DesktopEntry {
        name: name.to_string(),
        icon,
    })
}

/// The hint `key`, if it's there with the type `T`.
fn hint<T: glib::variant::FromVariant + glib::variant::StaticVariantType>(
    hints: &VariantDict,
    key: &str,
) -> Option<T> {
    hints
        .lookup_value(key, Some(&T::static_variant_type()))
        .and_then(|value| value.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: Duration = Duration::from_secs(7);

    #[test]
    fn an_apps_timeout_is_its_own_or_the_default_and_a_critical_one_never_times_out() {
        assert_eq!(timeout(-1, false, DEFAULT), Some(DEFAULT));
        assert_eq!(timeout(0, false, DEFAULT), None);
        assert_eq!(
            timeout(2500, false, DEFAULT),
            Some(Duration::from_millis(2500))
        );
        assert_eq!(timeout(2500, true, DEFAULT), None);
        assert_eq!(timeout(-1, true, DEFAULT), None);
    }

    #[test]
    fn an_icon_is_a_file_by_its_path_or_uri_and_otherwise_a_themes_name() {
        assert_eq!(
            icon("file:///tmp/a%20b.png"),
            Some(Icon::File("/tmp/a b.png".into()))
        );
        assert_eq!(icon("/tmp/a.png"), Some(Icon::File("/tmp/a.png".into())));
        assert_eq!(
            icon("dialog-information"),
            Some(Icon::Name("dialog-information".into()))
        );
        assert_eq!(icon(""), None);
    }
}
